#!/usr/bin/env python3
"""Where a push to `master` plans its verification from (ADR-0281, amended
2026-10-13): the newest commit of what it pushed that a verify run passed
at, or the commit before the push where none did.

A train is verified by one run at its head (ADR-0281's train amendment),
and `master` then moves to it with the train's evidence on top. Planned
from the commit before the push, master's run re-ran every recipe the
train's had, its code the train's, until the integrator cancelled it by
hand. Planned from the train's head, it runs what changed since: the
evidence and the documents, which no recipe reads.

- Only a commit on the push's first-parent line counts. A merge made on
  `master` is not the head its track's run passed at, and is verified as
  before.
- Only a run of a push or of the nightly counts. A push's run plans
  everything its track changed since it left `master` (verify.yml), and the
  nightly's every recipe; a dispatch's runs only the recipes it names.
- Any error answers the commit before the push: an error here plans more,
  never less.

    python3 scripts/verified_base.py BEFORE HEAD   # prints the base
    python3 scripts/verified_base.py --browsers BASE HEAD   # prints true or false

**And the browser suites** (amended the same day, its first use): master's
verify after train-3 planned no recipe from the train's head, and its three
browser jobs ran the suites the train's run had, on the same code. Where the
base is a run's commit, not the push's before, and what changed since it is
documents and the seconds kept alone, they are not run again.

`gh` reads the runs, with `GH_TOKEN` and `GITHUB_REPOSITORY` as a workflow
gives them.
"""

import json
import os
import pathlib
import subprocess
import sys
from typing import Callable

ROOT = pathlib.Path(__file__).resolve().parent.parent

# The events whose runs plan every recipe the commit's changes reach.
COVERING = ("push", "schedule")


def first_parents(before: str, head: str, root: pathlib.Path = ROOT) -> list[str]:
    """The commits `head` adds to `before` on its first-parent line, newest
    first, `head` among them."""
    out = subprocess.run(
        ["git", "rev-list", "--first-parent", f"{before}..{head}"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return out.split()


def runs_at(sha: str) -> list[dict]:
    """The verify runs that passed at `sha`, each `{"event", "conclusion"}`."""
    repo = os.environ["GITHUB_REPOSITORY"]
    out = subprocess.run(
        [
            "gh",
            "api",
            f"repos/{repo}/actions/workflows/verify.yml/runs"
            f"?head_sha={sha}&status=success&per_page=100",
            "--jq",
            "[.workflow_runs[] | {event, conclusion}]",
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return json.loads(out)


def passed(runs: list[dict]) -> bool:
    return any(r.get("conclusion") == "success" and r.get("event") in COVERING for r in runs)


def base(
    before: str,
    head: str,
    runs: Callable[[str], list[dict]] = runs_at,
    root: pathlib.Path = ROOT,
) -> str:
    try:
        for sha in first_parents(before, head, root):
            if passed(runs(sha)):
                return sha
    except Exception as error:  # noqa: BLE001 - any failure plans more, never less
        print(f"verified_base: {error}; planning from {before}", file=sys.stderr)
    return before


# What changes nothing a browser suite reads: the documents, the evidence
# among them, and the seconds each recipe last took.
RECORDS = ("docs/", "scripts/ci_seconds.json")


def browsers(verified: str, head: str, root: pathlib.Path = ROOT) -> bool:
    """Whether the browser suites run for `head`, where `verified` is the
    commit a run passed at: not where what changed since is records alone.
    Any error runs them."""
    try:
        names = subprocess.run(
            ["git", "diff", "--name-only", f"{verified}..{head}"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.split()
    except Exception as error:  # noqa: BLE001 - any failure runs more, never less
        print(f"verified_base: {error}; the browser suites run", file=sys.stderr)
        return True
    return not all(n.startswith(RECORDS) for n in names)


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "--browsers":
        print("true" if browsers(sys.argv[2], sys.argv[3]) else "false")
        return 0
    if len(sys.argv) != 3:
        print("usage: verified_base.py BEFORE HEAD | --browsers BASE HEAD", file=sys.stderr)
        return 2
    print(base(sys.argv[1], sys.argv[2]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
