#!/usr/bin/env python3
"""Run evidence recipes one after another, as a verification shard does
(ADR-0245).

For each recipe: `just <recipe>`, its exit status, how long it took, its
output, and the evidence files it wrote: those under `docs/evidence/` whose
bytes changed while it ran. Everything goes under `--out`:

    <out>/results.json          [{recipe, status, seconds, wrote}, ...]
    <out>/logs/<recipe>.log     what the recipe printed
    <out>/evidence/<path>       each file it wrote, at its own path

Exits 0 whatever the recipes did: whether a run passed is the summary's to
say, from every shard's results, and a shard that stopped at its first
failure would hide the recipes after it.

    python3 scripts/ci_recipes.py --out ci-out e14-feed e14-read-whole ...
"""

import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
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


def run(recipe: str, out: pathlib.Path) -> dict:
    before = snapshot()
    start = time.monotonic()
    with open(out / "logs" / f"{recipe}.log", "w") as log:
        status = subprocess.run(
            ["just", recipe], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT
        ).returncode
    seconds = round(time.monotonic() - start, 1)
    wrote = written(before, snapshot())
    for path in wrote:
        target = out / "evidence" / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, target)
    return {"recipe": recipe, "status": status, "seconds": seconds, "wrote": wrote}


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
        print(f"{recipe}: exit {result['status']} in {result['seconds']}s, wrote {len(result['wrote'])}")
        sys.stdout.flush()
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
