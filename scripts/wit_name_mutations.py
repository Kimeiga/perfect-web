#!/usr/bin/env python3
"""Mutation controls for WIT's reserved names (the 2026-09-26 correction).

Each mutant undoes one piece of how a Pleris name that WIT reserves is
written into the generated WIT text: the escape itself, one keyword, or one
place a name is written. The tests, which run each query through the E8
host, must then fail.

Run from the repository root; `just e10-wit-names` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WIT = ROOT / "compiler/pw-core/src/wit.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a keyword is written bare",
        WIT,
        '        true => format!("%{id}"),',
        "        true => id.to_string(),",
    ),
    (
        "`list` is not known to be a keyword",
        WIT,
        '    "list",\n    "map",',
        '    "map",',
    ),
    (
        "a field's name is written unescaped",
        WIT,
        "                        escaped(&ident(f)),",
        "                        ident(f),",
    ),
    (
        "a case's name is written unescaped",
        WIT,
        '"        {}{},\\n", escaped(&ident(case)), payload',
        '"        {}{},\\n", ident(case), payload',
    ),
    (
        "a function's name is written unescaped",
        WIT,
        '        format!("{}: func({}){ret};", escaped(ident), params.join(", ")),',
        '        format!("{}: func({}){ret};", ident, params.join(", ")),',
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "wit_names"],
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
