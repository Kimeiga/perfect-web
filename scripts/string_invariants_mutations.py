#!/usr/bin/env python3
"""Mutation controls for ADR-0225: a `String`'s length is an invariant.

Each mutant undoes one piece: the length read from the predicate, matched to
its representation, and never below nothing; a literal's length in code
points, a test of it that narrows, a bounded value's, and the construction
held to it; the contract's measure; the host's count; and the feed's
`PostText` stating its bounds. The tests of each must then fail.

The browser suite's `e2e/feed.spec.mjs` sends 280 emoji and not 281
characters, in three engines.

Run from the repository root; `just e14-string-invariants` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
FEED = ROOT / "examples/feed/app.pw"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a length is not read from the predicate",
        LOWER,
        '                    "String.length(value)" => Some(crate::hir::Measure::Length),\n',
        '                    "String.length(value)" => None,\n',
    ),
    (
        "a value's bound and a length's are read together",
        LOWER,
        "                Some(m) if *m != measure => {\n",
        "                Some(m) if false && *m != measure => {\n",
    ),
    (
        "a length is read of an `Int`",
        RULES,
        '        crate::hir::Measure::Length => ("String", "`String.length(value)`"),\n',
        '        crate::hir::Measure::Length => ("Int", "`String.length(value)`"),\n',
    ),
    (
        "a length may be below nothing",
        RULES,
        "        crate::hir::Measure::Length => Some(inv.at_least.unwrap_or(0).max(0)),\n",
        "        crate::hir::Measure::Length => inv.at_least,\n",
    ),
    (
        "a construction is held to a length as to a value",
        VALUES,
        "                    let found = self.length_at(*value);\n",
        "                    let found = self.interval_at(*value);\n",
    ),
    (
        "a test of a length narrows nothing",
        VALUES,
        "                let (path, op, k) = match (self.measured_path(*lhs), self.integer(*rhs)) {\n",
        "                let (path, op, k) = match (self.path_of(*lhs), self.integer(*rhs)) {\n",
    ),
    (
        "`String.length` is not known for what it is",
        VALUES,
        '            Named::Target(Target::Callable(sig)) if sig.path == "String.length" => Some(text.value),\n',
        '            Named::Target(Target::Callable(sig)) if false && sig.path == "String.length" => Some(text.value),\n',
    ),
    (
        "a literal's length is its bytes",
        VALUES,
        "                .map_or(any, |s| Interval::exactly(s.chars().count() as i128)),\n",
        "                .map_or(any, |s| Interval::exactly(s.len() as i128)),\n",
    ),
    (
        "a bounded value's length is unknown",
        VALUES,
        "                    .filter(|i| i.unread.is_empty() && i.measure == crate::hir::Measure::Length)\n",
        "                    .filter(|i| i.unread.is_empty() && i.measure == crate::hir::Measure::Value)\n",
    ),
    (
        "the contract bounds a length as a value",
        CONTRACT,
        "                        crate::hir::Measure::Length => Measure::Length,\n",
        "                        crate::hir::Measure::Length => Measure::Value,\n",
    ),
    (
        "the host counts a length in bytes",
        HOST,
        "                    let n = s.chars().count();\n",
        "                    let n = s.len();\n",
    ),
    (
        "a post's text states nothing",
        FEED,
        "opaque type PostText = String where String.length(value) >= 1 & String.length(value) <= 280\n",
        "opaque type PostText = String\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "string_invariants", "--test", "invariants"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine", "--test", "bounded"],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_posts_text_is_held_to_its_length_where_it_arrives",
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
