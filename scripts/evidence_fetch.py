#!/usr/bin/env python3
"""Fetch a verification run's evidence into `docs/evidence/` (ADR-0245).

`just evidence-fetch <run>` is the recorded command. Every file the run's
recipes wrote is copied in. One with a `commit:` line gains a line after it
naming the run that produced it, its URL and its runner, so it says which
command produced it (its `produced by:` line, unchanged), at which commit,
and where it ran, and a reader can open the run and its logs. A recipe run
here again writes the file without it.

Refused, before anything is copied:
- a run that has not completed, or did not succeed, unless each job that
  failed is one `--known` names: a failure open in NEXT, which a merge names
  (ADR-0281). A recipe shard is never known: its evidence is the record;
- a file whose `commit:` line names another commit than the run's: it would
  be evidence of something else;
- a run of another commit than `HEAD`, unless `--any-commit` says so.

Each recipe that ran to its end (exit 0) has its seconds kept in
`scripts/ci_seconds.json`, which `ci_plan.py` deals the next runs by
(ADR-0290). `--times-only` keeps them from any completed run, one that
failed or was cancelled among them, and copies no evidence: a recipe's time
is what it took, whatever its shard's others did.

    python3 scripts/evidence_fetch.py <run-id> [--any-commit] [--known JOB]...
    python3 scripts/evidence_fetch.py <run-id> --times-only
"""

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Each recipe's seconds, as `ci_plan.SECONDS` reads them.
SECONDS = ROOT / "scripts/ci_seconds.json"


def gh(*args: str) -> str:
    return subprocess.run(
        ["gh", *args], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout


def stamp(text: str, sha: str, url: str, runner: str) -> str:
    """`text` with the run named after its `commit:` line, or as it is where
    it has none (an artifact, a recipe's raw output). Raises where it names
    another commit than the run's."""
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.startswith("commit: "):
            written = line[len("commit: "):].split()[0]
            if written != sha:
                raise ValueError(f"names commit {written}, and the run is of {sha}")
            run_line = f"ci: {url} ({runner})\n"
            if i + 1 < len(lines) and lines[i + 1].startswith("ci: "):
                lines[i + 1] = run_line
            else:
                lines.insert(i + 1, run_line)
            return "".join(lines)
    return text


def refusal(run: dict, jobs: list[dict], known: list[str]) -> str | None:
    """Why a run's evidence is not fetched, or `None`. A run that failed is
    fetched only where every job that failed is named as known, and none is
    a recipe shard, whose evidence the fetch copies."""
    if run["status"] != "completed":
        return f"is {run['status']}"
    if run["conclusion"] == "success":
        return None
    failed = [j["name"] for j in jobs if j.get("conclusion") not in ("success", "skipped")]
    shards = [n for n in failed if n.startswith("recipes")]
    if shards:
        return f"failed in {', '.join(shards)}, whose evidence this would copy"
    unknown = [n for n in failed if n not in known]
    if unknown or not failed:
        return f"is {run['conclusion']}, failed in {', '.join(unknown) or 'no job'}"
    return None


def timed(results: list[dict]) -> dict[str, int]:
    """Each recipe that ran to its end, and its seconds, from shards'
    `results.json`."""
    return {r["recipe"]: round(r["seconds"]) for r in results if r.get("status") == 0}


def remember(seconds: dict[str, int], path: pathlib.Path) -> int:
    """Keep `seconds` in `path`, over what it held for the same recipes;
    every other recipe's as it was. How many it kept."""
    try:
        kept = json.loads(path.read_text())
    except (OSError, ValueError):
        kept = {}
    kept.update(seconds)
    path.write_text(json.dumps(dict(sorted(kept.items())), indent=2) + "\n")
    return len(seconds)


def shard_dirs(tmp: pathlib.Path) -> list[pathlib.Path]:
    """Each shard's directory, or the download's own where it had one
    (`ci_summary.shards_of`)."""
    return sorted(tmp.glob("evidence-*")) or ([tmp] if (tmp / "evidence").is_dir() else [])


def results_of(shards: list[pathlib.Path]) -> list[dict]:
    out = []
    for shard in shards:
        if (shard / "results.json").exists():
            out.extend(json.loads((shard / "results.json").read_text()))
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("run")
    parser.add_argument("--any-commit", action="store_true")
    parser.add_argument("--known", action="append", default=[], metavar="JOB")
    parser.add_argument("--times-only", action="store_true")
    args = parser.parse_args()
    run = json.loads(gh("run", "view", args.run, "--json", "headSha,url,status,conclusion,workflowName,jobs"))
    if args.times_only:
        if run["status"] != "completed":
            print(f"evidence-fetch: run {args.run} is {run['status']}", file=sys.stderr)
            return 1
        with tempfile.TemporaryDirectory() as tmp:
            gh("run", "download", args.run, "--dir", tmp, "--pattern", "evidence-*")
            n = remember(timed(results_of(shard_dirs(pathlib.Path(tmp)))), SECONDS)
        print(f"evidence-fetch: {n} recipes' seconds from {run['url']} kept in {SECONDS.relative_to(ROOT)}")
        return 0
    why = refusal(run, run.get("jobs", []), args.known)
    if why is not None:
        print(f"evidence-fetch: run {args.run} {why}", file=sys.stderr)
        return 1
    if run["conclusion"] != "success":
        print(f"evidence-fetch: run {args.run} failed only in {', '.join(args.known)}, known (ADR-0281)")
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    if run["headSha"] != head and not args.any_commit:
        print(f"evidence-fetch: run {args.run} is of {run['headSha']}, and HEAD is {head}", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as tmp:
        gh("run", "download", args.run, "--dir", tmp, "--pattern", "evidence-*")
        staged: dict[pathlib.Path, bytes] = {}
        refused = []
        shards = shard_dirs(pathlib.Path(tmp))
        for shard in shards:
            runner = (shard / "runner.txt").read_text().strip() if (shard / "runner.txt").exists() else "a GitHub-hosted runner"
            for f in sorted((shard / "evidence").rglob("*")):
                if not f.is_file():
                    continue
                rel = f.relative_to(shard / "evidence")
                if rel.suffix != ".txt":
                    staged[ROOT / rel] = f.read_bytes()
                    continue
                try:
                    staged[ROOT / rel] = stamp(f.read_text(), run["headSha"], run["url"], runner).encode()
                except ValueError as e:
                    refused.append(f"{rel}: {e}")
        if refused:
            print("evidence-fetch: refused, nothing copied:\n  " + "\n  ".join(refused), file=sys.stderr)
            return 1
        for path, data in sorted(staged.items()):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            print(f"  {path.relative_to(ROOT)}")
        n = remember(timed(results_of(shards)), SECONDS)
    print(f"evidence-fetch: {len(staged)} files from {run['url']}; {n} recipes' seconds kept")
    return 0


if __name__ == "__main__":
    sys.exit(main())
