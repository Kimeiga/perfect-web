#!/usr/bin/env python3
"""Mutation controls for ADR-0101: a command invalidates what it writes.

Each mutant undoes one piece: checking a command's write at all, what counts
as reaching a reader (naming it, or an event it and not another reader
listens for, declared by this command), which readers are held (a staleness
window, any module, the domain written), setting aside a clause another rule
refuses, the operation a database effect names, and the underline. The
tests in `writes_invalidated.rs` and `effects.rs`'s unit test must then fail.

Run from the repository root; `just e10-writes-invalidated` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
GRAPH = ROOT / "compiler/pw-core/src/graph.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a write is not checked",
        CHECK,
        "        if writes.is_empty() {",
        "        if true {",
    ),
    (
        "any emitted event reaches the reader",
        GRAPH,
        "                        l.kind == EdgeKind::InvalidatedBy && l.from == resource && l.to == e.to",
        "                        true",
    ),
    (
        "another reader's listening counts",
        GRAPH,
        "                        l.kind == EdgeKind::InvalidatedBy && l.from == resource && l.to == e.to",
        "                        l.kind == EdgeKind::InvalidatedBy && l.to == e.to",
    ),
    (
        "naming the entry does not invalidate it",
        GRAPH,
        "                    EdgeKind::Invalidates => e.to == resource,",
        "                    EdgeKind::Invalidates => false,",
    ),
    (
        "any command's clause counts",
        GRAPH,
        "            e.from == command\n",
        "            (e.from == command || e.from != command)\n",
    ),
    (
        "a staleness window is not the reader's declaration",
        CHECK,
        ".is_some_and(|f| crate::policy::duration(&f.value).is_some_and(|ms| ms > 0))",
        ".is_some_and(|_| false)",
    ),
    (
        "a zero window is a window",
        CHECK,
        ".is_some_and(|f| crate::policy::duration(&f.value).is_some_and(|ms| ms > 0))",
        ".is_some_and(|f| crate::policy::duration(&f.value).is_some())",
    ),
    (
        "readers of the command's module only",
        CHECK,
        "        for r in readers {",
        "        for r in readers.iter().filter(|r| r.unit == unit) {",
    ),
    (
        "any domain is the one written",
        CHECK,
        "writes.intersection(&r.reads)",
        "writes.union(&r.reads)",
    ),
    (
        "a clause naming nothing is not set aside",
        CHECK,
        "        if clauses_refused(graph, &path) {",
        "        if false {",
    ),
    (
        "a clause naming another kind is not set aside",
        CHECK,
        "        || graph.edges.iter().any(|e| {\n            e.from == command\n",
        "        || false && graph.edges.iter().any(|e| {\n            e.from == command\n",
    ),
    (
        "a read is a write",
        EFFECTS,
        '(head.trim().strip_prefix("database.") == Some(operation) && !domain.is_empty())',
        '(head.trim().starts_with("database.") && !domain.is_empty())',
    ),
    (
        "the write is not underlined",
        CHECK,
        '.find(|s| crate::effects::database_domain(&s.effect, "write") == Some(shared[0]))',
        ".find(|_| false)",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "writes_invalidated"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--lib", "a_database_effect_names_its_domain"],
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
