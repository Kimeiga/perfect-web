#!/usr/bin/env python3
"""Whether the nightly's verify run is skipped (track ci-diagnostics,
2026-10-20).

In the seven days to 2026-10-20 the nightly `verify` (`on: schedule`) used 691
runner-hours, some 100 a night and 27% of all the CI there was, and ran every
recipe again at a head `master` had held since 2026-10-17, where a run at that
exact commit had passed already. It is skipped, recipes and browser suites
alike, where such a run has:

    python3 scripts/nightly_skip.py SHA SCHEDULE   # exit 0: skip, 1: run, 2: usage

It prints which, and why, on one line, which `verify.yml`'s plan step writes
to the log and to the run's summary.

**How it decides**: one query of the GitHub API, `GET
/repos/{repo}/actions/workflows/verify.yml/runs?head_sha=SHA&status=success`
(`gh api`, with the `GH_TOKEN` and `GITHUB_REPOSITORY` the workflow gives
it; the plan job has `actions: read`), and the nightly is skipped where one
of the runs it returns is of a **push or a schedule**: a landing train's own
run, or an earlier nightly's. That is the rule `verified_base.py` already
holds a push to `master` to (ADR-0281, amended 2026-10-13): a push's run
plans everything its track changed since it left `master`, and a nightly's
every recipe. A run of a `workflow_dispatch` is not one: it runs the recipes
it names, which may be a few. A run that failed, was cancelled or is still
going is not one either: `status=success` leaves them out, so a head whose
run is red, or was cancelled by hand, is verified in full that night.

- **`any error runs it`** (as `verified_base.py`'s plans more, never less):
  the API unreachable, a reply that is no JSON, `gh` missing.
- **The weekly run is never skipped.** The workflow has two schedules, Monday
  to Saturday and Sunday (`WEEKLY`), and `github.event.schedule` says which
  fired: Sunday's runs every recipe whatever has passed, so that what a
  verified commit cannot show, a dependency's new advisory, a test that fails
  one run in twenty, is still met within a week. A weekday's is skipped only
  by the rule above.
- **A dispatch is the override**: `workflow_dispatch` never asks this; with
  no recipes named it runs every one (verify.yml).
- A skipped nightly is itself a run of a schedule that succeeded at the
  commit, so the next night's is skipped by it as well, until `master`
  moves or Sunday comes; the run it stands on is named in each one's line.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from typing import Callable

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import verified_base  # noqa: E402 - the events a covering run is of are its

# The schedule of the weekly full run: Sunday, as `verify.yml` has it.
WEEKLY = "17 6 * * 0"

# The events whose successful run at a commit verified it: a push's, a nightly's.
COVERING = verified_base.COVERING


def runs_at(sha: str) -> list[dict]:
    """The verify runs that succeeded at `sha`, each `{"id", "event",
    "conclusion", "html_url"}`."""
    repo = os.environ["GITHUB_REPOSITORY"]
    out = subprocess.run(
        [
            "gh",
            "api",
            f"repos/{repo}/actions/workflows/verify.yml/runs?head_sha={sha}&status=success&per_page=100",
            "--jq",
            "[.workflow_runs[] | {id, event, conclusion, html_url}]",
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return json.loads(out)


def verified_by(runs: list[dict]) -> dict | None:
    """The first run that verified the commit: a push's or a schedule's that
    succeeded."""
    return next((r for r in runs if r.get("conclusion") == "success" and r.get("event") in COVERING), None)


def decide(sha: str, schedule: str, runs: Callable[[str], list[dict]] = runs_at) -> tuple[bool, str]:
    """(skip, why): whether the nightly at `sha`, fired by `schedule`, is
    skipped, and the line that says so."""
    if schedule == WEEKLY:
        return False, f"run: the weekly full run ({WEEKLY}) verifies {sha[:8]} again whatever has passed"
    try:
        found = verified_by(runs(sha))
    except Exception as error:  # noqa: BLE001 - any failure runs more, never less
        return False, f"run: could not ask which runs passed at {sha[:8]} ({error}); verifying in full"
    if found is None:
        return False, f"run: no push's or nightly's verify run has succeeded at {sha[:8]}"
    return True, (
        f"skip: {sha[:8]} was verified by the {found.get('event')}'s run {found.get('id')} "
        f"({found.get('html_url')}); Sunday's schedule verifies it in full"
    )


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print("usage: nightly_skip.py SHA [SCHEDULE]", file=sys.stderr)
        return 2
    skip, why = decide(argv[1], argv[2] if len(argv) == 3 else "")
    print(why)
    return 0 if skip else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
