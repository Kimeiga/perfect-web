#!/usr/bin/env python3
"""Mutation controls for ADR-0067: a record is built with each of its fields
once, and an `if` without `else` is no value.

Each mutant undoes one piece of the fields relation, or of the type an `if`
without `else` has. The record-fields tests must then fail.

Run from the repository root; `just e10-record-fields` records the output.
The source is restored after every mutant, whatever happens.
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
        "a field its type does not declare is skipped",
        VALUES,
        "                relations.push(fields_relation(\n"
        "                    init.span.clone(),\n"
        "                    FIELD_UNDECLARED,\n"
        "                    &init.name,\n"
        "                ));\n"
        "                whole = false;\n"
        "                continue;",
        "                continue;",
    ),
    (
        "a field given twice is not noticed",
        VALUES,
        "            if !given.insert(init.name.as_str()) {",
        "            if !given.insert(init.name.as_str()) && false {",
    ),
    (
        "a field left out is not noticed",
        VALUES,
        "            if !given.contains(name.as_str()) {",
        "            if false && !given.contains(name.as_str()) {",
    ),
    (
        "a shorthand field is not counted as given",
        VALUES,
        "            if !given.insert(init.name.as_str()) {",
        "            if init.value.is_some() && !given.insert(init.name.as_str()) {",
    ),
    (
        "an `if` without `else` has no stated type",
        VALUES,
        "            Expr::If { els: None, .. } => Ty::Primitive(Primitive::Unit),",
        "            Expr::If { els: None, .. } => Ty::Unknown,",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "record_fields"],
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
