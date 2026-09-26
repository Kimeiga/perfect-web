#!/usr/bin/env python3
"""Mutation controls for ADR-0105: a command invalidates the entry it
speculates on.

Each mutant undoes one piece: checking a speculation at all, reaching the
entry through an event it hears, the command's own clauses, setting aside a
clause another rule refuses, and leaving a speculated reader to PW5107
alone. The tests in `speculation_reconciled.rs` must then fail.

Run from the repository root; `just e10-speculation-reconciled` records the
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
        "a speculation is not checked",
        CHECK,
        "        per_unit.extend(speculation_reconciled(&graph, &speculated, &u.hir));\n",
        "",
    ),
    (
        "only `invalidates` reconciles",
        CHECK,
        "        if clauses_refused(graph, &s.command) || graph.invalidates(&s.command, &s.target) {",
        "        if clauses_refused(graph, &s.command)\n"
        "            || graph.edges.iter().any(|e| {\n"
        "                e.from == s.command && e.kind == EdgeKind::Invalidates && e.to == s.target\n"
        "            })\n"
        "        {",
    ),
    (
        "any command's clause reconciles",
        CHECK,
        "        if clauses_refused(graph, &s.command) || graph.invalidates(&s.command, &s.target) {",
        "        if clauses_refused(graph, &s.command)\n"
        "            || graph.edges.iter().any(|e| {\n"
        "                e.kind == EdgeKind::Invalidates && e.to == s.target\n"
        "            })\n"
        "        {",
    ),
    (
        "a refused clause is reported again",
        CHECK,
        "        if clauses_refused(graph, &s.command) || graph.invalidates(&s.command, &s.target) {",
        "        if graph.invalidates(&s.command, &s.target) {",
    ),
    (
        "PW5106 reports a speculated reader too",
        CHECK,
        "                .any(|s| s.command == path && s.target == r.path)",
        "                .any(|s| s.command == path && s.target == r.path && false)",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "speculation_reconciled"],
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
