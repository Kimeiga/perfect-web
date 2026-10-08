#!/usr/bin/env python3
"""Mutation controls for ADR-0230: a condition is a `Bool`, or a `List` or a
`String` tested non-empty (ruling 0071-a).

Each mutant undoes one piece: a number with no truth, a record with none, a
string and a list tested non-empty, the number's own message and repair, and
"an" before a vowel. The template-truth tests, ADR-0071's, the corpus's
R-059 and `operand_type`'s witnesses must then fail.

Run from the repository root; `just e14-condition-truth` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a number has a truth",
        VALUES,
        "            Ty::Primitive(Primitive::Bool | Primitive::Str)\n",
        "            Ty::Primitive(Primitive::Bool | Primitive::Str | Primitive::Int | Primitive::Float)\n",
    ),
    (
        "a record has a truth",
        VALUES,
        "            other => Outcome::Disagree {\n"
        '                expected: "no truth".to_string(),\n'
        "                actual: self.display(other),\n"
        "            },\n",
        "            _ => Outcome::Agree,\n",
    ),
    (
        "a string is not tested non-empty",
        VALUES,
        "            Ty::Primitive(Primitive::Bool | Primitive::Str)\n",
        "            Ty::Primitive(Primitive::Bool)\n",
    ),
    (
        "a list is not tested non-empty",
        VALUES,
        "            | Ty::Builtin(Builtin::List, _)\n",
        "",
    ),
    (
        "a number is said as a case",
        VALUES,
        '                "a number" => Diagnostic::error(\n',
        '                "a number, said" => Diagnostic::error(\n',
    ),
    (
        "a vowel takes \"a\"",
        VALUES,
        "        Some('a' | 'e' | 'i' | 'o' | 'u') => \"an\",\n",
        "        Some('a' | 'e' | 'i' | 'o' | 'u') => \"a\",\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "template_truth", "--test", "template_operands",
        "--test", "checking_source", "--test", "generality",
    ],
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
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
