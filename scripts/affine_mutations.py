#!/usr/bin/env python3
"""Mutation controls for ADR-0045: an affine value is consumed exactly once on
every path, and a declaration that promises to release a parameter does.

A test that passes with the mechanism removed is not evidence for it. Each
mutant undoes one piece of ADR-0045, and at least one test must then fail or
not build: the generality witnesses in
`examples/generality/affine_not_consumed_once/`, the corpus, or the unit
tests.

Run from the repository root; `just e10-affine` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
AFFINE = ROOT / "compiler/pw-core/src/affine.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the end of a scope is not an exit",
        AFFINE,
        "    if flow.through.contains(&0) {",
        "    if false {",
    ),
    (
        "a failing `?` is not an exit",
        AFFINE,
        "                .then(Flow::exit(span.clone()).or(Flow::identity())),",
        "                .then(Flow::identity()),",
    ),
    (
        "a second release is not counted",
        AFFINE,
        "    (a + b).min(2)",
        "    (a + b).min(1)",
    ),
    (
        "a release in a loop counts once",
        AFFINE,
        "                    .find(|s| self.releases.contains(s)),\n            },",
        "                    .find(|s| self.releases.contains(s))\n                    .filter(|_| false),\n            },",
    ),
    (
        "a parameter the declaration promises to release is not followed",
        AFFINE,
        "        owned.extend(released_parameters(hir, sigs, id, decl, body));",
        "        let _ = released_parameters(hir, sigs, id, decl, body);",
    ),
    (
        "the body's value does not move the resource to the caller",
        AFFINE,
        "                if self.tails.contains(&id) && means(self.body, self.types, id, self.acquired) =>\n"
        "            {\n"
        "                Flow::releasing(1)",
        "                if self.tails.contains(&id) && means(self.body, self.types, id, self.acquired) =>\n"
        "            {\n"
        "                Flow::identity()",
    ),
    (
        "a use binding is held to explicit releases",
        AFFINE,
        "            } else if a.scoped {",
        "            } else if false {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "generality"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-cli"],
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
