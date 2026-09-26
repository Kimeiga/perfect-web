#!/usr/bin/env python3
"""Mutation controls for ADR-0072: an element named with a capital letter is
a view.

Each mutant undoes one piece of how such an element is read: that it is read
at all, how its name resolves, and which declarations are views. The
view-element tests must then fail.

Run from the repository root; `just e10-view-elements` records the output.
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
        "the rule is not run",
        CHECK,
        "        per_unit.extend(view_elements(&workspace, &hirs, i, &u.hir));\n",
        "",
    ),
    (
        "an element named with a capital letter is not read",
        CHECK,
        "            if !tag.starts_with(|c: char| c.is_ascii_uppercase()) {",
        "            if true || !tag.starts_with(|c: char| c.is_ascii_uppercase()) {",
    ),
    (
        "an element named in lowercase is read as a view",
        CHECK,
        "            if !tag.starts_with(|c: char| c.is_ascii_uppercase()) {",
        "            if !tag.starts_with(|c: char| c.is_ascii_alphabetic()) {",
    ),
    (
        "a tag's name resolves to nothing",
        CHECK,
        "                    crate::resolve::declaration(hirs, def).map(|d| d.kind)",
        "                    {\n                        let _ = (hirs, def);\n                        None\n                    }",
    ),
    (
        "an imported view is not resolved",
        CHECK,
        "                Resolution::Local(def) | Resolution::Imported { def, .. } => {\n"
        "                    crate::resolve::declaration(hirs, def).map(|d| d.kind)\n"
        "                }\n"
        "                Resolution::Unresolved | Resolution::Ambiguous(_) => None,",
        "                Resolution::Local(def) => {\n"
        "                    crate::resolve::declaration(hirs, def).map(|d| d.kind)\n"
        "                }\n"
        "                Resolution::Imported { .. } | Resolution::Unresolved | Resolution::Ambiguous(_) => None,",
    ),
    (
        "a view is not told from another declaration",
        CHECK,
        "                Some(DeclKind::View | DeclKind::Component | DeclKind::Page) => (",
        "                Some(DeclKind::Type) => (",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "view_elements"],
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
