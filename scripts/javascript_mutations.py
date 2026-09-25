#!/usr/bin/env python3
"""Mutation controls for ADR-0044: pure computation compiled to JavaScript.

The differential test agreed on its first run, which is exactly what a test
that cannot fail also does. Each mutant here puts back one place where
JavaScript's own semantics differ from Pleris's, and the test must then find
the component and the module disagreeing.

Run from the repository root; `just e10-javascript` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a String is ordered by UTF-16 unit, as JavaScript's `<` is",
        JS,
        'format!("{}({a}, {b}) {} 0", self.uses("compare"), symbol(*op))',
        'format!("{a} {} {b}", symbol(*op))',
    ),
    (
        "String.trim is ECMAScript's trim",
        JS,
        'Intrinsic::StrTrim => format!("{}({})", self.uses("trim"), arg(0)),',
        'Intrinsic::StrTrim => format!("{}.trim()", arg(0)),',
    ),
    (
        "String.length counts UTF-16 units",
        JS,
        'format!("BigInt(Array.from({}).length)", arg(0))',
        'format!("BigInt({}.length)", arg(0))',
    ),
    (
        "an Int does not trap on overflow",
        JS,
        r'if (v < -(2n ** 63n) || v > 2n ** 63n - 1n) trap(\"Int overflow\");',
        r'if (false) trap(\"Int overflow\");',
    ),
    (
        "`/` truncates toward zero, as BigInt's does",
        JS,
        "if (a % b < 0n) q = b > 0n ? q - 1n : q + 1n;",
        "/* truncated */",
    ),
    (
        "`%` takes the dividend's sign, as BigInt's does",
        JS,
        "return r < 0n ? (b > 0n ? r + b : r - b) : r;",
        "return r;",
    ),
    (
        "from_codepoints accepts surrogates",
        JS,
        " || (p >= 0xD800n && p <= 0xDFFFn)",
        "",
    ),
    (
        "sort_by puts the left element first on a positive comparison",
        JS,
        "sort((a, b) => {sign}(f(a, b)))",
        "sort((a, b) => -{sign}(f(a, b)))",
    ),
    (
        "List.get reads one past the end",
        JS,
        "{i} >= 0n && {i} < BigInt({xs}.length)",
        "{i} >= 0n && {i} <= BigInt({xs}.length)",
    ),
    (
        "List.take reads a negative count from the end",
        JS,
        "{n} < 0n ? 0 : ",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript"],
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
