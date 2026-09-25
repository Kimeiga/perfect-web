#!/usr/bin/env python3
"""Mutation controls for ADR-0039: pure computation in the component backend.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0039, or of the two defects found building it,
and at least one test must then fail or not build. A mutant that survives is a
piece nothing holds.

Run from the repository root; `just e10-pure` records the output. The sources
are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
KOKA = ROOT / "compiler/pw-core/src/koka.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "addition wraps",
        WASM,
        "fn trap_on_add_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {",
        "fn trap_on_add_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {\n    return;",
    ),
    (
        "subtraction wraps",
        WASM,
        "fn trap_on_sub_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {",
        "fn trap_on_sub_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {\n    return;",
    ),
    (
        "multiplication wraps",
        WASM,
        "fn trap_on_mul_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {",
        "fn trap_on_mul_overflow(ops: &mut Ops, a: u32, b: u32, r: u32) {\n    return;",
    ),
    (
        "negation wraps",
        WASM,
        "fn trap_on_negation_overflow(ops: &mut Ops, a: u32) {",
        "fn trap_on_negation_overflow(ops: &mut Ops, a: u32) {\n    return;",
    ),
    (
        "division truncates",
        WASM,
        "fn euclidean_quotient(ops: &mut Ops, a: u32, b: u32, q: u32) {",
        "fn euclidean_quotient(ops: &mut Ops, a: u32, b: u32, q: u32) {\n    return;",
    ),
    (
        "the remainder takes the dividend's sign",
        WASM,
        "fn euclidean_remainder(ops: &mut Ops, b: u32, m: u32) {",
        "fn euclidean_remainder(ops: &mut Ops, b: u32, m: u32) {\n    return;",
    ),
    (
        "`&` evaluates as `|`",
        LOWER,
        "B::And => (right, self.constant(Const::Bool(false), Type::Bool)),",
        "B::And => (self.constant(Const::Bool(true), Type::Bool), right),",
    ),
    (
        "`<` on strings is `<=`",
        WASM,
        "BinaryOp::Lt => (Helper::StrCmp, Some(I::I32LtS)),",
        "BinaryOp::Lt => (Helper::StrCmp, Some(I::I32LeS)),",
    ),
    (
        "a negative number is written with `+`",
        WASM,
        "                    I::I32Const(45),",
        "                    I::I32Const(43),",
    ),
    (
        "a literal is placed one byte off",
        WASM,
        ".insert(s.to_string(), DATA_BASE as u32 + out.bytes.len() as u32);",
        ".insert(s.to_string(), DATA_BASE as u32 + out.bytes.len() as u32 + 1);",
    ),
    (
        "the region starts on the literals",
        WASM,
        "        let end = DATA_BASE as u32 + self.bytes.len() as u32;",
        "        let end = DATA_BASE as u32;",
    ),
    (
        "every record field is stored at offset 0",
        WASM,
        "            let offset = offset.size_wasm32() as u64;",
        "            let offset = offset.size_wasm32() as u64 * 0;",
    ),
    (
        "imports are read from the top of the body only",
        WASM,
        "    for i in all_instrs(&entry.instrs) {",
        "    for i in entry.instrs.iter() {",
    ),
    (
        "a program that does not parse is compiled",
        LOWER,
        "            units.iter().flat_map(syntax_errors).collect();",
        "            Vec::new();",
    ),
    (
        "an operator after a name starts a policy",
        GRAMMAR,
        "                | Kind::Plus\n                | Kind::Minus\n                | Kind::Star\n                | Kind::Slash\n                | Kind::Percent\n                | Kind::Cmp\n",
        "",
    ),
    (
        "Koka negates with `-`",
        KOKA,
        'UnOp::Neg => "~",',
        'UnOp::Neg => "-",',
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "computation"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "koka_backend"],
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
