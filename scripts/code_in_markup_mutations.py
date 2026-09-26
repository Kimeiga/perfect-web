#!/usr/bin/env python3
"""Mutation controls for ADR-0094: a template writes no code.

Each mutant undoes one piece: running the rule, each of the places it
refuses (a script, a stylesheet a value writes into, an inline handler, a
script URL or a scheme a value writes, a `srcdoc`), reading a URL as a
browser does, and keeping the `on:` directive apart from an inline handler.
The tests in `code_in_markup.rs` must then fail.

Run from the repository root; `just e10-code-in-markup` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the rule does not run",
        CHECK,
        "        per_unit.extend(code_in_markup(&u.hir));\n",
        "",
    ),
    (
        "a script element is written",
        CHECK,
        "            if tag == \"script\" {",
        "            if false && tag == \"script\" {",
    ),
    (
        "a stylesheet holds a value",
        CHECK,
        "            if tag == \"style\"\n",
        "            if false && tag == \"style\"\n",
    ),
    (
        "an inline handler is written",
        CHECK,
        "                if let Some(event) = name.strip_prefix(\"on\")\n",
        "                if let Some(event) = name.strip_prefix(\"on\").filter(|_| false)\n",
    ),
    (
        "an on: directive is an inline handler",
        CHECK,
        "                    && event.chars().all(|c| c.is_ascii_alphabetic())",
        "                    && event.chars().all(|c| c.is_ascii_alphabetic() || c == ':')",
    ),
    (
        "a script URL is written",
        CHECK,
        "    [\"javascript\", \"vbscript\"]\n",
        "    [\"no-scheme\"]\n",
    ),
    (
        "a value may write a URL's scheme",
        CHECK,
        "    if scheme.contains(HOLE) {",
        "    if false && scheme.contains(HOLE) {",
    ),
    (
        "whitespace hides a scheme",
        CHECK,
        "        .filter(|c| *c == HOLE || *c > ' ')",
        "        .filter(|_| true)",
    ),
    (
        "a URL with holes is not read",
        CHECK,
        "                        Expr::Interpolated { text, .. } => Some(text.as_str()),",
        "                        Expr::Interpolated { .. } => None,",
    ),
    (
        "a document is written inline",
        CHECK,
        "                if name == \"srcdoc\" {",
        "                if false && name == \"srcdoc\" {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "code_in_markup"],
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
