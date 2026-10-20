#!/usr/bin/env python3
"""Mutation controls for `scripts/skipped_specs.py` (the integrator's ruling
of 2026-10-19, train-11): every spec an optional build can leave out runs in
at least one recipe that makes that build.

Each mutant undoes one part of the check: a recipe that runs the spec need
not make its build; one that makes the build need not run the spec; a suite
that names another spec taken for one that runs it; an annotated spec's
roles not read; a build with no directory taken as optional.

A mutant must fail `scripts/tests/test_skipped_specs.py` by a failed
assertion: a run that errors, or says no count, is no kill, and stops the
run.

Run from the repository root; `just e14-skipped-specs` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "scripts/skipped_specs.py"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a recipe that runs the spec need not make its build",
        CHECK,
        "                makes(body, build[\"prerequisite\"]) and runs(body, spec, root)\n",
        "                makes(body, build[\"prerequisite\"]) or runs(body, spec, root)\n",
    ),
    (
        "a recipe that makes the build need not run the spec",
        CHECK,
        "            return True\n    return False\n\n\ndef makes(",
        "            return True\n    return True\n\n\ndef makes(",
    ),
    (
        "a suite that names another spec runs this one",
        CHECK,
        "        if directory == SUITE and (spec in command or \".spec.mjs\" not in command):\n",
        "        if directory == SUITE:\n",
    ),
    (
        "an annotated spec's roles are not read",
        CHECK,
        "            roles = ANNOTATION.findall((root / SUITE / \"e2e\" / spec).read_text())\n",
        "            roles = []\n",
    ),
    (
        "a build with no directory is optional",
        CHECK,
        "        if name != required and build[\"dist\"] is not None\n",
        "        if name != required\n",
    ),
]

TESTS = [sys.executable, "-m", "unittest", "-v", "scripts/tests/test_skipped_specs.py"]


def run_tests():
    """(valid, passed, failed): valid where unittest ran and counted, and no
    test errored (an error is not an assertion's failure)."""
    r = subprocess.run(TESTS, cwd=ROOT, capture_output=True, text=True, timeout=600)
    out = r.stdout + r.stderr
    ran = re.search(r"^Ran (\d+) tests? in ", out, re.M)
    if ran is None or re.search(r"^ERROR: ", out, re.M):
        return False, 0, 0
    if re.search(r"^OK\b", out, re.M) and r.returncode == 0:
        return True, int(ran.group(1)), 0
    bad = re.search(r"^FAILED \(failures=(\d+)(?:, skipped=\d+)?\)$", out, re.M)
    if bad is None or r.returncode == 0:
        return False, 0, 0
    failed = int(bad.group(1))
    return True, int(ran.group(1)) - failed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    valid, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not valid or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1

    survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            valid, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not valid:
            print(f"{what}: REFUSED (the tests errored or said no count; no kill)", flush=True)
            mutation_baseline.explain()
            return 1
        if failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
