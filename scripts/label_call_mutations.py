#!/usr/bin/env python3
"""Mutation controls for ADR-0064: a label carried through a call.

Each mutant undoes one piece of how a call's result is labelled by what it is
given: a declared call whose result mentions one of its type parameters, the
argument, piped value or receiver each parameter is given, a function
labelled by what it computes, and an undeclared call's receiver. The tests of
labels through calls must then fail.

Run from the repository root; `just e10-labels` records the output.
The source is restored after every mutant, whatever happens.
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
        "a declared call's result keeps its contract alone",
        LABELS,
        "                        if carried.is_empty() {\n                            return l;\n                        }",
        "                        if carried.is_empty() || l.is_public() {\n                            return l;\n                        }",
    ),
    (
        "a result's type parameters are not found",
        LABELS,
        "        walk(r, sig.definition, &mut out);",
        "        let _ = r;",
    ),
    (
        "an argument a parameter brings in is not joined",
        LABELS,
        "                                l = l.join(&self.label(body, value));",
        "                                let _ = value;",
    ),
    (
        "a piped value is not its call's first argument",
        LABELS,
        "                        let first = self.piped.get(&id).copied().or(match body.expr(*callee) {",
        "                        let first = self.piped.get(&id).copied().filter(|_| false).or(match body.expr(*callee) {",
    ),
    (
        "a method call's receiver is not its first argument",
        LABELS,
        "                            {\n                                Some(*base)\n                            }",
        "                            {\n                                let _ = base;\n                                None\n                            }",
    ),
    (
        "a function is labelled public",
        LABELS,
        "            Expr::Lambda { body: inner, .. } => self.label(body, *inner),\n",
        "",
    ),
    (
        "an undeclared call leaves out its receiver",
        LABELS,
        "                            .chain(receiver)\n",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "labels_through_calls"],
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
