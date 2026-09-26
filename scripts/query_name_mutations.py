#!/usr/bin/env python3
"""Mutation controls for ADR-0108: a query names a resource that exists.

Each mutant undoes one piece: treating `query` and `subscription` apart
from the rest of the keyword family, resolving the resource's name, and a
subscription. The tests in `queries_name_resources.rs`
must then fail.

Run from the repository root; `just e10-queries-name-resources` records
the output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NAMES = ROOT / "compiler/pw-core/src/names.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a query's first word is bound, as before",
        NAMES,
        '            } if matches!(keyword.as_str(), "query" | "subscription") => {',
        '            } if matches!(keyword.as_str(), "query" | "subscription") && false => {',
    ),
    (
        "the resource's name is not resolved",
        NAMES,
        "                    self.name(&r, self.body.expr_span(id));\n",
        "                    let _ = r;\n",
    ),
    (
        "a subscription's first word is bound",
        NAMES,
        '            } if matches!(keyword.as_str(), "query" | "subscription") => {',
        '            } if matches!(keyword.as_str(), "query") => {',
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "queries_name_resources"],
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
