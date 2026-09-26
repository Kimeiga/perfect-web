#!/usr/bin/env python3
"""Mutation controls for ADR-0092: a dependency-graph clause belongs to a
declaration that can mean it.

Each mutant undoes one piece: running the rule, asking the declaration's
kind, and each clause's kinds in the table. The tests in `clause_places.rs`
must then fail.

Run from the repository root; `just e10-clause-places` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the rule does not run",
        CHECK,
        "        per_unit.extend(clauses_in_place(&u.hir));\n",
        "",
    ),
    (
        "a clause is in place on any declaration",
        CHECK,
        "            if kinds.contains(&decl.kind) {",
        "            if true || kinds.contains(&decl.kind) {",
    ),
    (
        "a query or a function may emit",
        POLICY,
        "        \"emits\" | \"invalidates\" => (&[K::Command], \"a command\"),",
        "        \"emits\" | \"invalidates\" => (&[K::Command, K::Query, K::Fn], \"a command\"),",
    ),
    (
        "a command may listen",
        POLICY,
        "            &[K::Query, K::Subscription, K::Resource, K::Materialize],",
        "            &[K::Query, K::Subscription, K::Resource, K::Materialize, K::Command],",
    ),
    (
        "a command may depend",
        POLICY,
        "        \"depends_on\" => (&[K::Materialize], \"a materialization\"),",
        "        \"depends_on\" => (&[K::Materialize, K::Command], \"a materialization\"),",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "clause_places"],
]

def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
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
