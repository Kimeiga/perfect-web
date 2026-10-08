#!/usr/bin/env python3
"""Mutation controls for ADR-0276: `pw fmt` changes no program's meaning.

`pw fmt` wrote the feed's `max_bytes 5_000_000` as `5 _000_000`, which `pw
check` refuses, and `-1` in a clause as `- 1`; it put a lambda's `else`
branch a level deeper than its `if` branch, and a clause's continued value
flush with the clauses. Each mutant undoes one piece: a `_` between two
digits grouping nothing, or grouping wherever it stands; the HIR keeping a
number's `_`; a policy's value spaced as an expression, and a block policy's
body kept as written; the check that a formatting kept what a program says,
by its tokens and by a value's gaps; a line that begins by closing indenting
from itself; and a policy's value continuing flush with its clauses. The
tests of each must then fail.

Run from the repository root; `just e14-fmt-meaning` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEXER = ROOT / "compiler/pw-syntax/src/lexer.rs"
FMT = ROOT / "compiler/pw-syntax/src/fmt.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a `_` between two digits groups nothing",
        LEXER,
        "                    && (b[*i].is_ascii_digit()\n"
        "                        || (b[*i] == b'_' && *i + 1 < b.len() && b[*i + 1].is_ascii_digit()))\n",
        "                    && (b[*i].is_ascii_digit())\n",
    ),
    (
        "a `_` groups digits wherever it stands",
        LEXER,
        "                        || (b[*i] == b'_' && *i + 1 < b.len() && b[*i + 1].is_ascii_digit()))\n",
        "                        || b[*i] == b'_')\n",
    ),
    (
        "the HIR keeps a number's `_`",
        LOWER,
        "                    Some((K::Int, s)) => Literal::Int(s.replace('_', \"\")),\n",
        "                    Some((K::Int, s)) => Literal::Int(s),\n",
    ),
    (
        "a policy's value is spaced as an expression",
        FMT,
        "        if let Some(apart) = written_apart_in_a_policy_value(t) {\n",
        "        if let Some(apart) = written_apart_in_a_policy_value(t).filter(|_| false) {\n",
    ),
    (
        "a block policy's body keeps its gaps as written",
        FMT,
        "        .filter(|p| !has_block(p))\n",
        "",
    ),
    (
        "a value's gaps are not compared",
        FMT,
        "        if a != b || apart(&was, i) == apart(&is, i) {\n",
        "        if true || a != b || apart(&was, i) == apart(&is, i) {\n",
    ),
    (
        "a token's text is not compared",
        FMT,
        "        if w.kind() != n.kind() || w.text() != n.text() {\n",
        "        if w.kind() != n.kind() {\n",
    ),
    (
        "a line that begins by closing indents from itself",
        FMT,
        "        if !first.starts_with(['}', ')', ']']) {\n",
        "        if true || !first.starts_with(['}', ')', ']']) {\n",
    ),
    (
        "a policy's value continues flush with its clauses",
        FMT,
        "            scope_since_last_decl = !is_decl_here && a.kind() != K::Policy;\n",
        "            scope_since_last_decl = !is_decl_here;\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-syntax"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "backend_lowering"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
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
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
