#!/usr/bin/env python3
"""Mutation controls for ADR-0153 and ADR-0154: two forms gate item 5 found
open, closed.

Each mutant undoes one piece:
- an `{#if}` condition is not read for a case test;
- an `{:else if}` condition is not;
- a case on the left of `==` is not;
- a command a page's handler calls is not found;
- an `idempotent_by` is not asked of it.

A mutant must fail `case_and_delivery.rs`.

Run from the repository root; `just e14-case-and-delivery` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
RESUME = ROOT / "compiler/pw-core/src/resume.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "an {#if} that tests a case is not refused",
        CHECK,
        "                        case_test(body, sigs, &types, sigs.unit_of(hir.module_of(id)), c)\n",
        "                        case_test(body, sigs, &types, sigs.unit_of(hir.module_of(id)), c).filter(|_| false)\n",
    ),
    (
        "an {:else if} condition is not read",
        CHECK,
        "                        Node::Branch { condition, .. } => *condition,\n",
        "                        Node::Branch { .. } => None,\n",
    ),
    (
        "a case written on the left is not seen",
        CHECK,
        "    for (value, other) in [(*lhs, *rhs), (*rhs, *lhs)] {\n",
        "    for (value, other) in [(*lhs, *rhs)] {\n",
    ),
    (
        "a qualified case's qualifier is not checked",
        CHECK,
        "                        if Some(d) == x.def_id()\n",
        "                        if Some(d) == x.def_id() || true\n",
    ),
    (
        "a command a handler calls is not found",
        RESUME,
        "                    if sigs.kind_of(def) == Some(crate::hir::DeclKind::Command) {\n",
        "                    if sigs.kind_of(def) == Some(crate::hir::DeclKind::Command) && false {\n",
    ),
    (
        "a sent command is not asked for idempotent_by",
        CHECK,
        "        if decl.kind != hir::DeclKind::Command || decl.policy(\"idempotent_by\").is_some() {\n",
        "        if decl.kind != hir::DeclKind::Command || decl.policy(\"idempotent_by\").is_none() {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "case_and_delivery"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
        if not found:
            built = False
            continue
        for p, f in found:
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
