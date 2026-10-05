#!/usr/bin/env python3
"""Mutation controls for ADR-0211: a path that leaves a loop's body leaves
the function, and owes its releases (the owner's ruling 0045-a).

Each mutant undoes one piece: a pass's exits, a release accepted on a path
that leaves before the pass ends, and a release refused on a path that goes
round again. The affine tests must then fail.

Run from the repository root; `just e14-affine-loops` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
AFFINE = ROOT / "compiler/pw-core/src/affine.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a loop's body has no exits",
        AFFINE,
        "                    exits: pass.exits,\n",
        "                    exits: Vec::new(),\n",
    ),
    (
        "a release before an exit inside a loop is refused",
        AFFINE,
        "                        .any(|c| *c > 0)\n",
        "                        .any(|_| true)\n",
    ),
    (
        "a release that goes round again is accepted",
        AFFINE,
        "                        .any(|c| *c > 0)\n",
        "                        .any(|c| *c > 1)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "affine_loops", "--test", "affine_bindings"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
