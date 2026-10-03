#!/usr/bin/env python3
"""Mutation controls for ADR-0159: a handler handles what its command
answers.

Each mutant undoes one piece:
- in the checker: a handler's last value, a value it returns, and the
  failure its `?` returns, each counted as used again; and a lambda inside a
  handler taken for the handler;
- in the backend: a block ending in a binding refused again, so the discard
  by name the checker accepts cannot be built;
- in naming: a handler that discards its call's answer named nothing.

A mutant must fail `results_handled.rs` or `handlers.rs`, all of them run.

Run from the repository root; `just e14-handler-failures` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ANNOTATIONS = ROOT / "compiler/pw-core/src/annotations.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a handler's last value is used",
        ANNOTATIONS,
        "            true => unused(body, *b, Fate::ToTheRuntime, true, handlers, out),\n",
        "            true => unused(body, *b, Fate::Used, true, handlers, out),\n",
    ),
    (
        "a value a handler returns is used",
        ANNOTATIONS,
        "        true => Fate::ToTheRuntime,\n        false => Fate::Used,\n    };\n",
        "        true => Fate::Used,\n        false => Fate::Used,\n    };\n",
    ),
    (
        "a handler's `?` passes its failure on",
        ANNOTATIONS,
        "        Expr::Try { value } if handler => {\n",
        "        Expr::Try { value } if handler && false => {\n",
    ),
    (
        "a lambda inside a handler is taken for the handler",
        ANNOTATIONS,
        "            false => unused(body, *b, Fate::Used, false, handlers, out),\n",
        "            false => unused(body, *b, Fate::Used, handler, handlers, out),\n",
    ),
    (
        "a block ending in a binding is refused",
        LOWER,
        "                        if tail && !matches!(expected, None | Some(Type::Unit)) {\n",
        "                        if tail {\n",
    ),
    (
        "a handler that discards its call's answer is named nothing",
        TEMPLATE,
        "        Expr::Let { init: Some(i), .. } => named_call(body, *i),\n",
        "        Expr::Let { .. } => None,\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "results_handled"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handlers"],
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
