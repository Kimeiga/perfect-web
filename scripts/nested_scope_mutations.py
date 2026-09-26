#!/usr/bin/env python3
"""Mutation controls for ADR-0066: a nested declaration sees the bindings
around it.

Each mutant undoes one piece of how a declaration nested in another, a
stream's part, a release clause, a lambda's one parenthesised parameter, and
a call to a function value are resolved to the binding they mean, or how
that binding is typed or labelled. The nested-scope tests must then fail.

Run from the repository root; `just e10-nested` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"
HIR = ROOT / "compiler/pw-core/src/hir.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a nested declaration sees nothing around it",
        LEXICAL,
        "        let outer: Vec<(String, DeclId, Binder)> = match enclosing(hir, id) {",
        "        let outer: Vec<(String, DeclId, Binder)> = match enclosing(hir, id).filter(|_| false) {",
    ),
    (
        "a nested declaration sees what the body's end does, wherever it is written",
        LEXICAL,
        "                            if *written < at && !self.out.nested.contains_key(child) {",
        "                            if false && *written < at && !self.out.nested.contains_key(child) {",
    ),
    (
        "a binding around a nested declaration is untyped in the value relations",
        VALUES,
        "                typed\n                    .get(&d)",
        "                typed\n                    .get(&d)\n                    .filter(|_| false)",
    ),
    (
        "a binding around a nested declaration is untyped in the declared-type environment",
        INFER,
        "            .map(|(_, b)| parent.as_ref().and_then(|t| t.bindings.get(&b).cloned()))",
        "            .map(|(_, b)| parent.as_ref().and_then(|t| t.bindings.get(&b).cloned()).filter(|_| false))",
    ),
    (
        "a binding around a nested declaration is unlabelled",
        LABELS,
        "                outer.insert(Binder::Outer(i as u32), found.clone());",
        "                let _ = (i, found);",
    ),
    (
        "a nested declaration has no module",
        HIR,
        "        self.module_of(parent)\n",
        "        let _ = parent;\n        None\n",
    ),
    (
        "a stream's part binds nothing",
        LEXICAL,
        "                        scope.push((name, Binder::Stream(*n)));",
        "                        let _ = (name, n);",
    ),
    (
        "a stream's part is untyped",
        VALUES,
        "            added |= self.bind(Binder::Stream(node), t);",
        "            let _ = (node, t);",
    ),
    (
        "a release clause binds nothing",
        LEXICAL,
        "                            scope.push((name, Binder::Clause(s, k)));",
        "                            let _ = (name, s, k);",
    ),
    (
        "a release clause's name is not what its resource acquires",
        VALUES,
        "            added |= self.bind(Binder::Clause(clause, 0), t);",
        "            let _ = (clause, t);",
    ),
    (
        "one parenthesised parameter is a descriptor",
        LOWER,
        "        K::NameExpr | K::ParamList | K::TupleExpr | K::ListExpr | K::ParenExpr",
        "        K::NameExpr | K::ParamList | K::TupleExpr | K::ListExpr",
    ),
    (
        "a call's callee is local wherever the body binds its name",
        CHECK,
        "                    Some(lx) if matches!(body.expr(*callee), Expr::Name(_)) => {",
        "                    Some(lx) if false && matches!(body.expr(*callee), Expr::Name(_)) => {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "nested_scope"],
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
