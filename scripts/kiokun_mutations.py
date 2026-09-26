#!/usr/bin/env python3
"""Mutation controls for ADR-0041: kiokun's shard rule and ranking, in Pleris.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0041, or of the defects found building it, and
at least one test must then fail or not build.

The Pleris mutants are killed by the E10 oracle, which compiles
`examples/kiokun/` from source. `evidence_is_current` is left out of the tests
run here: it would kill every Pleris mutant by noticing only that the
committed build differs, which says nothing about what the build means.

Run from the repository root; `just e10-kiokun-mutants` records the output.
The sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
SHARDS = ROOT / "examples/kiokun/Shards.pw"
APP = ROOT / "examples/kiokun/app.pw"
HOST = ROOT / "spikes/kiokun/server/src"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "group_by never starts a new run",
        WASM,
        "            I::Call(eq),\n            I::I32Eqz,\n            I::LocalSet(differs),",
        "            I::Call(eq),\n            I::Drop,\n            I::I32Const(0),\n            I::LocalSet(differs),",
    ),
    (
        "every run group_by writes starts at the list's head",
        WASM,
        "                I::LocalGet(ptr),\n                I::LocalGet(start),\n                I::I32Const(esize as i32),",
        "                I::LocalGet(ptr),\n                I::I32Const(0),\n                I::I32Const(esize as i32),",
    ),
    (
        "an unannotated binding of a generic call keeps the callee's `T`",
        VALUES,
        "            .retain(|_, t| !t.mentions_foreign_parameter(own));",
        "            .retain(|_, _| true);",
    ),
    (
        "a statement keyword can name a value",
        GRAMMAR,
        """        if self.at(Kind::Ident)
            && (STMT_KEYWORDS.contains(&self.cur_text()) || self.cur_text() == "derived")
        {""",
        "        if false {",
    ),
    (
        "Shards.pw: two Han characters are han-3plus",
        SHARDS,
        '            if hans == 2 { "han-2char-{h % 8 + 1}" }',
        '            if hans == 3 { "han-2char-{h % 8 + 1}" }',
    ),
    (
        "Shards.pw: the hex digit ten is `:`",
        SHARDS,
        "    if digit < 10 { 48 + digit } else { 87 + digit }",
        "    if digit < 10 { 48 + digit } else { 48 + digit }",
    ),
    (
        "app.pw: Search keeps twenty-one hits",
        APP,
        "        List.map(List.take(ranked, 20), r => r.hit)",
        "        List.map(List.take(ranked, 21), r => r.hit)",
    ),
    (
        "app.pw: a common word ranks after an uncommon one",
        APP,
        "            if a.hit.common { -1 } else { 1 }",
        "            if a.hit.common { 1 } else { -1 }",
    ),
    (
        "the host takes an escaped file name for its word",
        HOST / "shard.rs",
        "    if !name.contains('_') {",
        "    if true {",
    ),
    (
        "a lookup takes a file that records another word",
        HOST / "app.rs",
        "                                .is_none_or(|k| &k == word)",
        "                                .is_none_or(|_| true)",
    ),
    (
        "a word's path may hold `/`",
        HOST / "main.rs",
        "            .filter(|w| !w.contains('/'))",
        "            .filter(|_| true)",
    ),
    (
        "Places answers the first thousand words only",
        HOST / "app.rs",
        "    for chunk in words.chunks(1000) {",
        "    for chunk in words.chunks(1000).take(1) {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "stdlib"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "oracle"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "value_relations"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib"],
    ["cargo", "test", "--quiet", "--locked", "-p", "kiokun-server"],
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
