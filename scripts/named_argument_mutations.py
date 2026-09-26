#!/usr/bin/env python3
"""Mutation controls for ADR-0081: a named argument is given to the
parameter of its name.

Each mutant undoes one piece of how a named argument reaches its parameter:
in the checker's relations, in its refusals, in the labels' carried
parameters, and in the backend, where it decides what a program computes.
The named-argument tests, and the conformance test that runs the call
through the E8 host, must then fail.

Run from the repository root; `just e10-named-arguments` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
SIGNATURES = ROOT / "compiler/pw-core/src/signatures.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a named argument is related to the parameter in its place",
        VALUES,
        "                Ok(order) if arity_ok => {",
        "                Ok(order) if false && arity_ok => {",
    ),
    (
        "a named argument that names no parameter is not refused",
        VALUES,
        "                Err(fault) => named_fault = Some(fault),",
        "                Err(_) => {}",
    ),
    (
        "a positional argument may follow a named one",
        SIGNATURES,
        "            None if named => return Err(ArgFault::AfterNamed),\n",
        "",
    ),
    (
        "a function value's arguments may be named",
        VALUES,
        "        if let Some(n) = &named {",
        "        if let Some(n) = named.as_ref().filter(|_| false) {",
    ),
    (
        "a label is carried from the argument in the parameter's place",
        LABELS,
        "                        for (a, i) in args.iter().zip(order) {",
        "                        for (a, i) in args.iter().zip(leading..) {",
    ),
    (
        "the backend passes arguments in written order",
        LOWER,
        "        for (a, i) in args.iter().zip(order) {",
        "        for (a, i) in args.iter().zip(offset..) {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "named_arguments"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "named_arguments"],
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
