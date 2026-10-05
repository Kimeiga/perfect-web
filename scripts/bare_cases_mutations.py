#!/usr/bin/env python3
"""Mutation controls for ADR-0198: a bare case with a payload resolves as a
case without one does (ADR-0195, ruling 5).

Each mutant undoes one piece: the name check accepting one type's case and
naming several, the typer relating its payload, and the backend building it.

Every mutant must fail the tests of the rule, or the compiled programs'.

Run from the repository root; `just e14-bare-cases` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a bare case with a payload resolves to nothing",
        CHECK,
        "                        1 => {}\n",
        "                        1 => out.push(unresolved_bare_call(hir, decl, body, id, &path, &owners)),\n",
    ),
    (
        "one of several types' cases is taken",
        CHECK,
        "                        _ => out.push(crate::names::ambiguous_case(\n",
        "                        _ => drop(crate::names::ambiguous_case(\n",
    ),
    (
        "its payload is related to nothing",
        VALUES,
        "                        Some((def, index)) => Named::Target(Target::Case(def, index)),\n                        None => Named::Nothing,\n",
        "                        Some(_) => Named::Nothing,\n                        None => Named::Nothing,\n",
    ),
    (
        "the backend does not build it",
        BACKEND,
        "            if !path.contains('.')\n                && let Some((def, index)) =\n",
        "            if false\n                && let Some((def, index)) =\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "sum_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "sum_types"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests. A test process that aborts,
    as a stack overflow does, fails."""
    passed = failed = 0
    for command in TESTS:
        p = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "could not compile" in out or "error[E" in out:
                return False, passed, failed
            # It built, and the process ended before it reported.
            failed += 1
            continue
        passed += sum(int(n) for n, _ in found)
        failed += sum(int(f) for _, f in found)
    return True, passed, failed


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
            built, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
