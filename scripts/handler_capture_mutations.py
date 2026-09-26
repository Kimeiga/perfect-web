#!/usr/bin/env python3
"""Mutation controls for ADR-0110: a resumable handler reads what it
captures.

Each mutant undoes one piece: running the rule, a capture read as one, what
the handler binds itself, a declaration's name not being a binding, a
record's shorthand field as a read, and reporting each name once. The
tests in `handler_captures.rs` must then fail.

Run from the repository root; `just e10-handler-captures` records the
output. The source is restored after every mutant, whatever happens.
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
        "        per_unit.extend(handlers_read_their_captures(&sigs, i, &u.hir));\n",
        "",
    ),
    (
        "a capture is not read as one",
        CHECK,
        "                    if !bound || captured.contains(n) || own.contains(n) || !reported.insert(n) {",
        "                    if !bound || own.contains(n) || !reported.insert(n) {",
    ),
    (
        "what the handler binds is not its own",
        CHECK,
        "                    if !bound || captured.contains(n) || own.contains(n) || !reported.insert(n) {",
        "                    if !bound || captured.contains(n) || !reported.insert(n) {",
    ),
    (
        "a declaration's name is a binding",
        CHECK,
        "vec![(n.as_str(), body.expr_span(e), lexical.binder(e).is_some())]",
        "vec![(n.as_str(), body.expr_span(e), true)]",
    ),
    (
        "a shorthand field is not a read",
        CHECK,
        "                        .filter(|(_, f)| f.value.is_none())\n",
        "                        .filter(|(_, f)| f.value.is_none() && false)\n",
    ),
    (
        "each read is reported",
        CHECK,
        "                    if !bound || captured.contains(n) || own.contains(n) || !reported.insert(n) {",
        "                    if !bound || captured.contains(n) || own.contains(n) {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handler_captures"],
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
