#!/usr/bin/env python3
"""Mutation controls for ADR-0100: a query reads.

Each mutant undoes one piece: refusing a write inside a query, a
subscription's too, and counting a transaction as the write it holds. The
tests in `query_reads.rs` and `effects.rs`'s unit test must then fail.

Run from the repository root; `just e10-query-reads` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a query may write",
        EFFECTS,
        "        (Query | Subscription, \"database\") if writes(effect) => Some(",
        "        (Query | Subscription, \"no-family\") if writes(effect) => Some(",
    ),
    (
        "a subscription may write",
        EFFECTS,
        "        (Query | Subscription, \"database\") if writes(effect) => Some(",
        "        (Query, \"database\") if writes(effect) => Some(",
    ),
    (
        "a transaction is not a write",
        EFFECTS,
        "        \"database.write\" | \"database.transaction\"",
        "        \"database.write\"",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "query_reads"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "a_query_may_read_and_not_write"],
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
