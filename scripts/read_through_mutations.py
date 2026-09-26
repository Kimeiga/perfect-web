#!/usr/bin/env python3
"""Mutation controls for ADR-0102: an event reaches what reads what it
invalidates.

Each mutant undoes one piece: the drain asking what an event reaches rather
than who listens, following a read past the first, the key a read supplies,
a position a read leaves unfilled, an unknown position matching any value,
ending a cycle of reads, and following a node again on another path. The
tests in pw-materialize's `reads.rs` must then fail.

Run from the repository root; `just e10-read-through` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAPH = ROOT / "runtime/pw-materialize/src/graph.rs"
MATERIALIZE = ROOT / "runtime/pw-materialize/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an event reaches its listeners only",
        MATERIALIZE,
        "                if !g.reaches(&key.fragment, &c.event.name, &c.event.args, &key.key) {",
        "                if !g.listens(&key.fragment, &c.event.name, &c.event.args, &key.key) {",
    ),
    (
        "a read is followed one step",
        GRAPH,
        "                self.reaches_from(&e.to, event, values, &supplied, path)",
        "                self.binds(&e.to, event, values, &supplied)",
    ),
    (
        "a read's key is not carried",
        GRAPH,
        "                            .and_then(|j| key.get(j).copied().flatten())",
        "                            .and_then(|_: usize| None)",
    ),
    (
        "a position the read leaves unfilled is not read",
        GRAPH,
        "                    .map_or(e.key.len(), |n| n.params.len().max(e.key.len()));",
        "                    .map_or(e.key.len(), |_| e.key.len());",
    ),
    (
        "an unknown position matches no value",
        GRAPH,
        "                        Some(j) => key.get(j).is_some_and(|k| k.is_none_or(|k| k == value)),",
        "                        Some(j) => key.get(j).is_some_and(|k| k.is_some_and(|k| k == value)),",
    ),
    (
        "a cycle of reads does not end",
        GRAPH,
        "        if path.iter().any(|p| p == node) {",
        "        if path.iter().any(|p| p == node) && false {",
    ),
    (
        "a node followed once is not followed again",
        GRAPH,
        "        path.pop();\n",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-materialize", "--test", "reads"],
]


def run_tests():
    """(built, crashed, passed, failed) over every test command.

    A test binary that dies of a stack overflow prints no result line: the
    mutant built, and its tests did not finish.
    """
    built, crashed, passed, failed = True, False, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            if "overflowed its stack" in out:
                crashed = True
            else:
                built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
    return built, crashed, passed, failed


def main():
    built, crashed, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or crashed or failed or not passed:
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
            built, crashed, passed, failed = run_tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif crashed:
            verdict = "KILLED (the tests overflow the stack)"
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
