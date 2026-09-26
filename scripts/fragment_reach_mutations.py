#!/usr/bin/env python3
"""Mutation controls for ADR-0103: a write reaches the fragments built on it.

Each mutant undoes one piece: holding a materialization at all, what it
reads itself, what a query with a staleness window reads, reaching a
fragment only by an event, an event reaching a fragment through what it
reads, an event that reaches nothing, and naming the dependency the write
reaches the fragment through. The tests in `writes_reach_fragments.rs` must
then fail.

Run from the repository root; `just e10-fragments-reached` records the
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
        "a fragment is not a reader",
        CHECK,
        "            if decl.kind != DeclKind::Materialize {",
        "            if true {",
    ),
    (
        "a fragment's own reads are not its",
        CHECK,
        "                .chain(\n"
        "                    inference\n"
        "                        .effective_effects(unit, hir, id)\n"
        "                        .iter()\n",
        "                .chain(\n"
        "                    Vec::<String>::new()\n"
        "                        .iter()\n",
    ),
    (
        "a query with a window reads nothing a fragment shows",
        CHECK,
        "            read_by.insert(path.clone(), (decl.name.clone(), reads.clone()));",
        "            if decl.policy(\"freshness\").is_none() {\n"
        "                read_by.insert(path.clone(), (decl.name.clone(), reads.clone()));\n"
        "            }",
    ),
    (
        "a fragment is reached as a query is",
        CHECK,
        "            let reached = if fragment {",
        "            let reached = if false {",
    ),
    (
        "an event reaches a fragment's listeners only",
        CHECK,
        "                        && graph.affected_by(&e.to).contains(&r.path)",
        "                        && graph.edges.iter().any(|l| {\n"
        "                            l.kind == EdgeKind::InvalidatedBy && l.from == r.path && l.to == e.to\n"
        "                        })",
    ),
    (
        "any emitted event reaches a fragment",
        CHECK,
        "                        && graph.affected_by(&e.to).contains(&r.path)",
        "                        && true",
    ),
    (
        "every dependency is named",
        CHECK,
        "                    .filter(|(_, reads)| shared.iter().any(|d| reads.contains(*d)))",
        "                    .filter(|_| true)",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "writes_reach_fragments"],
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
