#!/usr/bin/env python3
"""Mutation controls for ADR-0040: the standard library's lists and strings.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0040, or of the defect found building it, and
at least one test must then fail or not build.

Run from the repository root; `just e10-stdlib` records the output. The
sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/mod.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "map writes every element to the first slot",
        WASM,
        "                let dst = self.element_address(out, i, usz);",
        "                let dst = out;",
    ),
    (
        "filter keeps every element",
        WASM,
        "                self.ops.extend([I::LocalGet(keep), I::If(Empty)]);",
        "                self.ops.extend([I::I32Const(1), I::If(Empty)]);",
    ),
    (
        "fold keeps its seed",
        WASM,
        "                match self.move_into(body.value, &holder) {",
        "                match Encoding::Encoded(()) {",
    ),
    (
        "find does not stop at the first match",
        WASM,
        "                self.ops.extend([I::Br(2), I::End]);",
        "                self.ops.extend([I::Nop, I::End]);",
    ),
    (
        "sort_by takes the right element on a tie",
        WASM,
        "            I::LocalGet(order),\n            I::I64Const(0),\n            I::I64LeS,",
        "            I::LocalGet(order),\n            I::I64Const(0),\n            I::I64LtS,",
    ),
    (
        "get refuses index 0",
        WASM,
        "                    I::LocalGet(index),\n                    I::I64Const(0),\n                    I::I64GeS,",
        "                    I::LocalGet(index),\n                    I::I64Const(0),\n                    I::I64GtS,",
    ),
    (
        "take does not clamp a negative count",
        WASM,
        "            I::LocalGet(n),\n            I::I64Const(0),\n            I::I64LtS,",
        "            I::I32Const(0),",
    ),
    (
        "concat writes the second list over the first",
        WASM,
        "                let second = self.element_address(out, la, esize);",
        "                let second = out;",
    ),
    (
        "length counts some bytes twice",
        WASM,
        "                    I::LocalGet(3),\n                    I::I64Const(1),\n                    I::I64Add,",
        "                    I::LocalGet(3),\n                    I::I64Const(2),\n                    I::I64Add,",
    ),
    (
        "from_codepoints accepts surrogates",
        WASM,
        "                I::I64Const(0xD800),",
        "                I::I64Const(0x110000),",
    ),
    (
        "trim keeps U+3000",
        WASM,
        "        0x20, 0x85, 0xA0, 0x1680, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000,",
        "        0x20, 0x85, 0xA0, 0x1680, 0x2028, 0x2029, 0x202F, 0x205F,",
    ),
    (
        "to_lower_ascii leaves `Z`",
        WASM,
        "                    I::I32Const(26),",
        "                    I::I32Const(25),",
    ),
    (
        "contains looks only at the start",
        WASM,
        "                I::LocalGet(1),\n                I::LocalGet(3),\n                I::I32Sub,\n                I::LocalSet(6),",
        "                I::I32Const(0),\n                I::LocalSet(6),",
    ),
    (
        "join writes no separators",
        WASM,
        "            let mut copy = vec![\n                I::LocalGet(4),\n                I::If(Empty),",
        "            let mut copy = vec![\n                I::I32Const(0),\n                I::If(Empty),",
    ),
    (
        "an intrinsic is not read from its declaration",
        BACKEND,
        'let p = decl.policy("intrinsic")?;',
        'let p = decl.policy("intrinsic_")?;',
    ),
    (
        "a parenthesised list is not a lambda's parameters",
        GRAMMAR,
        "                    .start_at(cp, if tuple { K::TupleExpr } else { K::ParenExpr });",
        "                    .start_at(cp, K::ParenExpr);",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "stdlib"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib"],
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
