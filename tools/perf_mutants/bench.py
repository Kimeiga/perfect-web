#!/usr/bin/env python3
"""Compare full Pleris mutant-program checks with different worker counts.

Never weakens or alters scripts/mutant_programs.py, never touches main CI.
All candidates must judge the exact same set of source mutants and programs.
"""
import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import mutant_programs as checker  # noqa: E402

SUMMARY = re.compile(
    r"mutant programs: (?P<mutants>\d+) mutants of a program's source, "
    r"(?P<judged>\d+) judged, in (?P<programs>\d+) programs "
    r"\((?P<roots>.*?)\); (?P<checks>\d+) checks; "
    r"(?P<named>\d+) refused by the rule their script names, "
    r"(?P<failures>\d+) failing"
)


def parse_summary(value):
    found = SUMMARY.findall(value)
    if len(found) != 1:
        raise ValueError("expected exactly one complete mutation result summary")
    n, judged, programs, roots, checks, named, failures = found[0]
    return {
        "mutants": int(n), "judged": int(judged),
        "programs": int(programs), "roots": sorted(roots.split(", ")),
        "checks": int(checks), "named": int(named), "failures": int(failures),
    }


def validate(summary, expected_mutants):
    if (not summary["mutants"] or summary["mutants"] != expected_mutants
        or summary["judged"] != expected_mutants
        or not summary["programs"] or len(summary["roots"]) != summary["programs"]
        or not summary["checks"] or summary["failures"]):
        raise ValueError("untrustworthy or incomplete mutation census: " + str(summary))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--workers", type=int, choices=[2, 4, 6], required=True)
    parser.add_argument("--pw", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.pw.is_file():
        parser.error("compiled Pleris pw binary does not exist")
    args.output.mkdir(parents=True, exist_ok=True)
    expected = len(checker.mutants())
    if expected <= 0:
        parser.error("Pleris mutant census empty: refusing vacuous experiment")
    checker.WORKERS = args.workers
    previous = sys.argv
    sys.argv = [str(ROOT / "scripts/mutant_programs.py"), str(args.pw.resolve())]
    result = io.StringIO()
    begin = time.monotonic()
    rc = 99
    try:
        with contextlib.redirect_stdout(result):
            rc = checker.main()
    finally:
        sys.argv = previous
    elapsed = round(time.monotonic()-begin, 3)
    output = result.getvalue()
    print(output, end="", flush=True)
    (args.output / "gate.log").write_text(output)
    summary = parse_summary(output)
    validate(summary, expected)
    if rc != 0:
        raise RuntimeError("mutation checker refused its candidates: code " + str(rc))
    record = {
        "sha": os.environ.get("GITHUB_SHA"),
        "arch": os.uname().machine,
        "cpu_count": os.cpu_count(),
        "workers": args.workers,
        "wall_seconds": elapsed,
        "check_exit": rc,
        "summary": summary,
    }
    (args.output / "results.json").write_text(json.dumps(record, indent=2) + "\n")
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as file:
            file.write(
                f"### Pleris mutant programs: {args.workers} workers\n"
                f"Full checks: {summary['checks']}; mutants: {summary['mutants']}; "
                f"wall: {elapsed}s; 0 failures.\n"
            )
    print(f"MUTANT BENCH WORKERS={args.workers} SECONDS={elapsed} CHECKS={summary['checks']}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
