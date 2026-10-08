#!/usr/bin/env python3
"""Mutation controls for ADR-0239: every code the compiler writes is
registered, once.

Each mutant undoes one piece: a declaration rule's code registered, the
parser's `for` without `in` given its own code, a reader's value in a shared
cache reported once, and the registry's tests reading what the compiler
writes, without its tests. The tests of each must then fail.

Run from the repository root; `just e14-registered-codes` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CODES = ROOT / "compiler/pw-core/src/codes.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "read-your-writes's code is not registered",
        CODES,
        "    READ_YOUR_WRITES_NEEDS_A_SESSION = \"PW0101\"",
        "    READ_YOUR_WRITES_NEEDS_A_SESSION = \"PW0119\"",
    ),
    (
        "a stale session read's code is not registered",
        CODES,
        "    SESSION_STATE_IS_FRESH = \"PW0102\"",
        "    SESSION_STATE_IS_FRESH = \"PW0118\"",
    ),
    (
        "the parser's `for` without `in` is PW0102 again",
        GRAMMAR,
        "            self.error(\"PW0018\", \"expected `in` after the loop binding\");\n",
        "            self.error(\"PW0102\", \"expected `in` after the loop binding\");\n",
    ),
    (
        "a reader's value in a shared cache is two errors again",
        RULES,
        "    // --- PW0101: read_your_writes on a public read --------------------------\n",
        "    if let Some(cache) = policy(policies, \"cache\")\n"
        "        && cache.value.trim() == \"shared\"\n"
        "        && matches!(visibility, \"session\" | \"private\")\n"
        "    {\n"
        "        out.push(err(\n"
        "            \"PW5001\",\n"
        "            \"a shared cache may contain only Public values\",\n"
        "            format!(\"cannot materialize `{name}` in a shared public cache\"),\n"
        "            cache.span.clone(),\n"
        "        ));\n"
        "    }\n"
        "    // --- PW0101: read_your_writes on a public read --------------------------\n",
    ),
    (
        "the registry's tests read the compiler's tests too",
        CODES,
        "                let own = src.split(\"#[cfg(test)]\").next().unwrap_or(\"\");\n",
        "                let own = src.as_str();\n",
    ),
    (
        "the registry's tests read no parser",
        CODES,
        "[\"pw-syntax/src\", \"pw-core/src\", \"pw-cli/src\"]",
        "[\"pw-core/src\", \"pw-cli/src\"]",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "registered_codes"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "codes::"],
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
