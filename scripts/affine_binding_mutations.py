#!/usr/bin/env python3
"""Mutation controls for ADR-0080: the affine rule follows bindings, not
names.

Each mutant undoes one piece of how a use of an affine value is found: by
the binding a name means, through a local bound to a declaration, and never
through a binding that may be reassigned; and the refusal of a value given
to any other function value. The affine binding tests must then fail.

Run from the repository root; `just e10-affine-bindings` records the output.
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
        "a use is any name spelled like the value",
        AFFINE,
        "    matches!(body.expr(e), Expr::Name(_)) && types.lexical().binder(e) == Some(a.binder)",
        "    matches!(body.expr(e), Expr::Name(n) if *n == a.name)",
    ),
    (
        "a local bound to a release counts nothing",
        AFFINE,
        "    let Binder::Pattern(p) = types.lexical().binder(name)? else {",
        "    let Some(Binder::Pattern(p)) = None::<Binder> else {",
    ),
    (
        "a binding that may be reassigned is read as its first value",
        AFFINE,
        "    if !matches!(\n"
        "        body.pat(p),\n"
        "        crate::hir::Pattern::Bind { mutable: false, .. }\n"
        "    ) {",
        "    if false {",
    ),
    (
        "a value given to a function value is not refused",
        AFFINE,
        "            } else if let Some((span, to)) = given.into_iter().next() {",
        "            } else if let Some((span, to)) = given.into_iter().next().filter(|_| false) {",
    ),
    (
        "a binding holding a function is not a function value",
        AFFINE,
        "        Expr::Name(_) => types.lexical().binder(callee).is_some(),",
        "        Expr::Name(_) => false,",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "affine_bindings"],
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
