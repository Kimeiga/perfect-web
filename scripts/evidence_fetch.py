#!/usr/bin/env python3
"""Fetch a verification run's evidence into `docs/evidence/` (ADR-0245).

`just evidence-fetch <run>` is the recorded command. Every file the run's
recipes wrote is copied in. One with a `commit:` line gains a line after it
naming the run that produced it, its URL and its runner, so it says which
command produced it (its `produced by:` line, unchanged), at which commit,
and where it ran, and a reader can open the run and its logs. A recipe run
here again writes the file without it.

Refused, before anything is copied:
- a run that has not completed, or did not succeed;
- a file whose `commit:` line names another commit than the run's: it would
  be evidence of something else;
- a run of another commit than `HEAD`, unless `--any-commit` says so.

    python3 scripts/evidence_fetch.py <run-id> [--any-commit]
"""

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("run")
    parser.add_argument("--any-commit", action="store_true")
    args = parser.parse_args()
    run = json.loads(gh("run", "view", args.run, "--json", "headSha,url,status,conclusion,workflowName"))
    if run["status"] != "completed" or run["conclusion"] != "success":
        print(f"evidence-fetch: run {args.run} is {run['status']}, {run['conclusion']}", file=sys.stderr)
        return 1
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    if run["headSha"] != head and not args.any_commit:
        print(f"evidence-fetch: run {args.run} is of {run['headSha']}, and HEAD is {head}", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as tmp:
        gh("run", "download", args.run, "--dir", tmp, "--pattern", "evidence-*")
        staged: dict[pathlib.Path, bytes] = {}
        refused = []
        for shard in sorted(pathlib.Path(tmp).glob("evidence-*")):
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
    print(f"evidence-fetch: {len(staged)} files from {run['url']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
