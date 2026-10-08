#!/usr/bin/env python3
"""Mutation controls for ADR-0252: a value returned early carries its label
to the caller.

Each mutant undoes one piece: each `return`'s value in a body's label; a
`return` below the body's top, in a branch or a loop; a function value's
`return`, which is its own; and the conditions an early return runs under,
a `return`'s and a `?`'s. The tests of each must then fail.

Run from the repository root; `just e14-returned-labels` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LABELS = ROOT / "compiler/pw-core/src/labels.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a `return`'s value is left out",
        LABELS,
        "        returns(body, body.root, &mut early);\n",
        "        let _ = returns;\n",
    ),
    (
        "only a `return` at the body's top counts",
        LABELS,
        "    for c in body.children(id) {\n        returns(body, c, out);\n    }\n",
        "    for c in body.children(id).into_iter().filter(|_| false) {\n        returns(body, c, out);\n    }\n",
    ),
    (
        "a function value's `return` is the body's",
        LABELS,
        "        Expr::Lambda { .. } => return,\n        Expr::Block { stmts } => {\n            for w in stmts.windows(2) {\n                if matches!(body.expr(w[0]), Expr::Name(n) if n == \"return\") {\n                    out.push((w[0], w[1]));\n",
        "        Expr::Lambda { .. } => {}\n        Expr::Block { stmts } => {\n            for w in stmts.windows(2) {\n                if matches!(body.expr(w[0]), Expr::Name(n) if n == \"return\") {\n                    out.push((w[0], w[1]));\n",
    ),
    (
        "an early return's conditions are left out",
        LABELS,
        "                    Some((c, _)) => l.join(c),\n",
        "                    Some((_, _)) => l,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "returned_labels"],
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
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
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
