#!/usr/bin/env python3
"""Mutation controls for ADR-0216: every clause belongs to a declaration
that reads it, and a code body admits none (the owner's rulings 0092-b and
0047-a's interim).

Run from the repository root; `just e14-clause-heads` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a head with no place fails open",
        POLICY,
        "        \"on_key_change\" => (&[K::Query, K::Resource], \"a query or a resource\"),\n",
        "",
    ),
    (
        "a cached read's heads belong to a command too",
        POLICY,
        "            (READS, \"a query, a subscription or a resource\")\n",
        "            (&[K::Query, K::Subscription, K::Resource, K::Command], \"a query, a subscription or a resource\")\n",
    ),
    (
        "`retry` belongs to a function too",
        POLICY,
        "            &[K::Query, K::Resource, K::Command],\n            \"a query, a resource or a command\",\n",
        "            &[K::Query, K::Resource, K::Command, K::Fn],\n            \"a query, a resource or a command\",\n",
    ),
    (
        "a code body admits clauses",
        NAMES,
        "            crate::hir::DeclKind::Fn\n                | crate::hir::DeclKind::Query\n",
        "            crate::hir::DeclKind::Prelude\n                | crate::hir::DeclKind::Query\n",
    ),
    (
        "no feasible placement speaks over the declared placement's refusal",
        CHECK,
        "            .any(|e| crate::rules::placement_conflict(target, &e.path).is_some())\n",
        "            .any(|e| crate::rules::placement_conflict(target, &e.path).is_some() && false)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "clause_places", "--test", "checking_source"],
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
