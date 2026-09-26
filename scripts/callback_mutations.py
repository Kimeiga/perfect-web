#!/usr/bin/env python3
"""Mutation controls for ADR-0053: a lambda's parameters take the types its
use declares.

Each mutant undoes one piece of how a lambda's parameters are typed, in
`infer.rs` (read without solving the call) or in `values.rs` (from the call,
the annotation or the result, solved), and the callback tests must then fail.

Run from the repository root; `just e10-callbacks` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
INFER = ROOT / "compiler/pw-core/src/infer.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "any type parameter takes the list's element",
        INFER,
        "Some(p) => elements.get(&p).cloned(),",
        "Some(_) => elements.values().next().cloned(),",
    ),
    (
        "a declared parameter type is not read",
        INFER,
        "None if !mentions_parameter(w) => Some(w.clone()),",
        "None if false => Some(w.clone()),",
    ),
    (
        "a list's element binds no type parameter",
        INFER,
        "elements.entry(param).or_insert(e);",
        "elements.entry(param).or_insert(e);\n                    elements.clear();",
    ),
    (
        "a piped value is not the first argument",
        INFER,
        """            let values: Vec<(usize, ExprId)> = piped
                .get(&id)
                .copied()
                .into_iter()""",
        """            let values: Vec<(usize, ExprId)> = None::<ExprId>
                .into_iter()""",
    ),
    (
        "a solved lambda's parameters are forgotten",
        VALUES,
        "let mut added = typer.solve_lambdas();",
        "let mut added = false;",
    ),
    (
        "a parameter with a hole in its type is kept",
        VALUES,
        "if ty.is_closed() && !ty.mentions_foreign_parameter(own) {",
        "if !ty.mentions_foreign_parameter(own) {",
    ),
    (
        "a lambda bound with a written type is not typed by it",
        VALUES,
        "} if is_lambda(*init) => {",
        "} if false => {",
    ),
    (
        "a returned lambda is not typed by the declared result",
        VALUES,
        "            for site in self.result_sites() {",
        "            for site in Vec::<ExprId>::new() {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "callback_parameters"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "causal_evidence"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "value_relations"],
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
