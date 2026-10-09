#!/usr/bin/env python3
"""Run evidence recipes one after another, as a verification shard does
(ADR-0245).

For each recipe: `just <recipe>`, its exit status, how long it took, its
output, and the evidence files it wrote: those under `docs/evidence/` whose
bytes changed while it ran. Everything goes under `--out`:

    <out>/results.json          [{recipe, status, seconds, wrote, free}, ...]
    <out>/logs/<recipe>.log     what the recipe printed
    <out>/evidence/<path>       each file it wrote, at its own path

Between recipes it frees what their builds leave behind (`prune`): a
runner's disk is about 14 GB, and each mutant a recipe plants builds its
tests again under a new hash. `free` is the disk left after.

Exits 0 whatever the recipes did: whether a run passed is the summary's to
say, from every shard's results, and a shard that stopped at its first
failure would hide the recipes after it.

    python3 scripts/ci_recipes.py --out ci-out e14-feed e14-read-whole ...
"""

import argparse
import collections
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import threading
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "docs" / "evidence"


def snapshot() -> dict[str, str]:
    """Every evidence file, by path, with the digest of its bytes."""
    out = {}
    for p in sorted(EVIDENCE.rglob("*")):
        if p.is_file():
            out[str(p.relative_to(ROOT))] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


def written(before: dict[str, str], after: dict[str, str]) -> list[str]:
    """The paths whose bytes changed, or that appeared, between two snapshots."""
    return sorted(p for p, digest in after.items() if before.get(p) != digest)


def prune(target: pathlib.Path) -> None:
    """Free what builds leave in `target`: the codegen units' object files,
    the incremental cache, and each test binary but the newest of its name.
    A later build rebuilds what it needs."""
    deps = target / "debug" / "deps"
    if deps.is_dir():
        newest: dict[str, list[pathlib.Path]] = collections.defaultdict(list)
        for p in deps.iterdir():
            m = re.match(r"^(.+)-[0-9a-f]{16}$", p.name)
            if m and p.is_file() and os.access(p, os.X_OK):
                newest[m.group(1)].append(p)
        for paths in newest.values():
            paths.sort(key=lambda p: p.stat().st_mtime, reverse=True)
            for old in paths[1:]:
                old.unlink(missing_ok=True)
        for p in deps.glob("*.rcgu.o"):
            p.unlink(missing_ok=True)
    shutil.rmtree(target / "debug" / "incremental", ignore_errors=True)


def free() -> str:
    """The disk left, as `df` says it."""
    usage = shutil.disk_usage(ROOT)
    return f"{usage.free / 2**30:.1f} GiB"


def available() -> str:
    """The memory left, as the kernel says it, where it says it."""
    meminfo = pathlib.Path("/proc/meminfo")
    if not meminfo.exists():
        return "memory unknown"
    for line in meminfo.read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return f"{int(line.split()[1]) / 2**20:.1f} GiB available"
    return "memory unknown"


def largest(n: int = 3) -> str:
    """The `n` processes holding the most memory, by name and size."""
    ps = subprocess.run(["ps", "-eo", "rss=,comm="], capture_output=True, text=True).stdout
    rows = sorted((line.split(None, 1) for line in ps.splitlines() if line.strip()),
                  key=lambda r: -int(r[0]))[:n]
    return ", ".join(f"{name.strip()} {int(rss) / 2**20:.1f} GiB" for rss, name in rows)


def newest_line(since: float) -> str:
    """The last line of the evidence file written most recently, since
    `since`: how far a recipe has got, in its own words."""
    files = [p for p in (ROOT / "docs" / "evidence").rglob("*.txt") if p.stat().st_mtime >= since]
    if not files:
        return "no evidence written yet"
    path = max(files, key=lambda p: p.stat().st_mtime)
    lines = path.read_text(errors="replace").strip().splitlines()
    return f"{path.relative_to(ROOT)}: {lines[-1][:160] if lines else ''}"


def heartbeat(recipe: str, stop: threading.Event, every: float = 30.0) -> None:
    """**What a recipe is doing, every `every` seconds, into the job's log**:
    its memory, the largest processes and its evidence's last line. A recipe
    writes its record to a file the runner uploads at the end, so a runner
    that dies mid-recipe left nothing to say where: `e14-graphs-on-the-wire`
    took three runners down a few minutes in (2026-10-08 and -09)."""
    start, wall = time.monotonic(), time.time()
    while not stop.wait(every):
        print(
            f"  [{recipe}, {time.monotonic() - start:.0f}s] {available()}; "
            f"{largest()}; {newest_line(wall)}",
            flush=True,
        )


def run(recipe: str, out: pathlib.Path) -> dict:
    before = snapshot()
    start = time.monotonic()
    stop = threading.Event()
    beat = threading.Thread(target=heartbeat, args=(recipe, stop), daemon=True)
    beat.start()
    try:
        with open(out / "logs" / f"{recipe}.log", "w") as log:
            status = subprocess.run(
                ["just", recipe], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT
            ).returncode
    finally:
        stop.set()
        beat.join()
    seconds = round(time.monotonic() - start, 1)
    wrote = written(before, snapshot())
    for path in wrote:
        target = out / "evidence" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, target)
    prune(ROOT / "target")
    return {"recipe": recipe, "status": status, "seconds": seconds, "wrote": wrote, "free": free()}


def failed_tail(recipe: str, out: pathlib.Path, wrote: list[str], lines: int = 40) -> str:
    """**What a failed recipe said last**, its log's and each evidence file's
    last `lines` lines, for the job's log: the runner uploads them only at
    the end, and `e14-contract` failed in 27 s in a nightly whose runner
    then died (2026-10-08), leaving nothing to read."""
    parts = []
    for name, path in [("log", out / "logs" / f"{recipe}.log")] + [
        (w, out / "evidence" / w) for w in wrote
    ]:
        if path.exists():
            tail = path.read_text(errors="replace").splitlines()[-lines:]
            parts.append(f"  --- {recipe}: {name}, its last {len(tail)} lines")
            parts.extend(f"  | {line}" for line in tail)
    return "\n".join(parts)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True)
    parser.add_argument("recipes", nargs="+")
    args = parser.parse_args()
    out = pathlib.Path(args.out).resolve()
    (out / "logs").mkdir(parents=True, exist_ok=True)
    results = []
    for recipe in args.recipes:
        result = run(recipe, out)
        results.append(result)
        print(
            f"{recipe}: exit {result['status']} in {result['seconds']}s, "
            f"wrote {len(result['wrote'])}, {result['free']} free"
        )
        if result["status"] != 0:
            print(failed_tail(recipe, out, result["wrote"]))
        sys.stdout.flush()
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
