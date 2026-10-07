#!/usr/bin/env python3
"""Mutation controls for ADR-0241: an optimistic transition's value is the
value typer's.

Each mutant undoes one piece: each transition related to its target's value,
the target's success type where it is a `Result`, the relation reported as
PW0331, with the target as its related span and the binder in its repair,
and `check.rs` still refusing a target that is no resource's entry. The
tests of each must then fail.

Run from the repository root; `just e14-transition-values` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a transition is related to nothing",
        VALUES,
        "        for (_, target, transition) in self.decl.optimistic_clauses() {\n"
        "            let expected = match self.of(target.root) {\n",
        "        for (_, target, transition) in self.decl.optimistic_clauses().into_iter().take(0) {\n"
        "            let expected = match self.of(target.root) {\n",
    ),
    (
        "a target's value is its whole `Result`",
        VALUES,
        "                Ty::Builtin(Builtin::Result, mut args) if args.len() == 2 => args.swap_remove(0),\n"
        "                other => other,\n"
        "            };\n"
        "            let boundary = (\n",
        "                Ty::Builtin(Builtin::Result, mut args) if args.len() == 9 => args.swap_remove(0),\n"
        "                other => other,\n"
        "            };\n"
        "            let boundary = (\n",
    ),
    (
        "the transition's value is related to itself",
        VALUES,
        "                RelationKind::Transition,\n"
        "                expected,\n"
        "                self.of(transition.root),\n",
        "                RelationKind::Transition,\n"
        "                self.of(transition.root),\n"
        "                self.of(transition.root),\n",
    ),
    (
        "the relation is reported as a result's",
        VALUES,
        "            RelationKind::Transition => Diagnostic::error(\n"
        "                crate::codes::OPTIMISTIC_TARGET_MISMATCH.id,\n",
        "            RelationKind::Transition => Diagnostic::error(\n"
        "                crate::codes::RETURN_TYPE.id,\n",
    ),
    (
        "the related span is the transition's",
        VALUES,
        "            let boundary = (\n"
        "                self.body.expr_span(target.root),\n",
        "            let boundary = (\n"
        "                self.body.expr_span(transition.root),\n",
    ),
    (
        "the repair names no binder",
        VALUES,
        "                .map_or_else(|| \"the bound value\".to_string(), |(n, _)| format!(\"`{n}`\"));\n",
        "                .map_or_else(|| \"the bound value\".to_string(), |_| \"the bound value\".to_string());\n",
    ),
    (
        "a target that is no resource's entry is not refused",
        CHECK,
        "            if resource_value_type(sigs, workspace, hirs, unit, &path).is_none() {\n",
        "            if resource_value_type(sigs, workspace, hirs, unit, &path).is_none() && false {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "transition_values"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "value_relations"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "policy_term_positions"],
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
