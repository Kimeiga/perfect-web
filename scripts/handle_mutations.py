#!/usr/bin/env python3
"""Mutation controls for ADR-0263: a session's, a user's or an
organization's handle is the platform's to make.

Each mutant undoes one piece: a construction found (PW5037), each kind of
handle, an operation outside the platform's answering one (PW5038), a
handle found inside an argument, a record and a representation, and what
the browser supplies (PW5039): a page's parameter, a module's signal and a
view's. The tests of `compiler/pw-core/tests/handles.rs` must then fail.

Run from the repository root; `just e14-handles` records the output. The
source is restored after every mutant, whatever happens.
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
        "a program may construct a handle",
        CHECK,
        "            let crate::values::Named::Target(crate::values::Target::Opaque(def)) =\n",
        "            let crate::values::Named::Target(crate::values::Target::Record(def)) =\n",
    ),
    (
        "a program may construct a user's handle",
        CHECK,
        "                Some(Q::User) => \"a user's\",\n",
        "",
    ),
    (
        "an operation outside the platform's may answer a handle",
        CHECK,
        "            && !op.interface.starts_with(\"pw:\")\n",
        "            && op.interface.starts_with(\"pw:\")\n",
    ),
    (
        "a handle in a type's argument is not found",
        CHECK,
        "    if let Some(held) = ty.args().iter().find_map(|a| handle_in(sigs, a, seen)) {\n",
        "    if let Some(held) = ty.args().iter().take(0).find_map(|a| handle_in(sigs, a, seen)) {\n",
    ),
    (
        "a handle in a record's field is not found",
        CHECK,
        # Re-anchored by ADR-0264: a type's parts are read in one place,
        # `declared_parts`, which ADR-0264's walk shares.
        "    decl.record\n        .iter()\n        .flatten()\n        .map(|(_, t)| t)\n"
        "        .chain(decl.representation.iter())\n"
        "        .chain(decl.variants.iter().flatten().flat_map(|(_, ts)| ts.iter()))\n"
        "        .filter_map(crate::resolved::TypeResolution::resolved)\n        .collect()\n",
        "    decl.representation\n        .iter()\n"
        "        .filter_map(crate::resolved::TypeResolution::resolved)\n        .collect()\n",
    ),
    (
        "an organization's handle is not found in a type",
        CHECK,
        "        Some(Q::Organization) => return Some(\"an organization's\"),\n",
        "",
    ),
    (
        "a page's parameter may hold a handle",
        CHECK,
        "        if matches!(decl.kind, DeclKind::Command | DeclKind::Page) {\n",
        "        if matches!(decl.kind, DeclKind::Command) {\n",
    ),
    (
        "a module's signal may hold a handle",
        CHECK,
        "        if decl.kind == DeclKind::Signal\n",
        "        if decl.kind == DeclKind::Signal && decl.name.is_empty()\n",
    ),
    (
        "a view's signal may hold a handle",
        CHECK,
        "        for at in &body.signals {\n",
        "        for at in body.signals.iter().take(0) {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handles"],
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
