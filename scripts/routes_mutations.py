#!/usr/bin/env python3
"""Mutation controls for ADR-0160: a page's route.

Each mutant undoes one piece: the plan's route, each part of PW0340 (a
malformed route, a name that is no parameter, a name given twice, a
parameter left out), PW0621 (any type, an opaque type over text), and
PW0341 (two pages at one route, told apart by their parameters' names).
`routes.rs` must then fail.

Run from the repository root; `just e14-routes` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROUTES = ROOT / "compiler/pw-core/src/routes.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the plan carries no route",
        PLAN,
        "            route: crate::routes::declared_route(hir, decl),\n",
        "            route: None,\n",
    ),
    (
        "a malformed route passes",
        ROUTES,
        "        if malformed {\n",
        "        if malformed && false {\n",
    ),
    (
        "a name that is no parameter passes",
        ROUTES,
        "            if !params.contains(name) {\n",
        "            if !params.contains(name) && false {\n",
    ),
    (
        "a name given twice passes",
        ROUTES,
        "            } else if named[..i].contains(name) {\n",
        "            } else if named[..i].contains(name) && false {\n",
    ),
    (
        "a parameter the route leaves out passes",
        ROUTES,
        "            if !named.contains(p) {\n",
        "            if !named.contains(p) && false {\n",
    ),
    (
        "a parameter of any type passes",
        ROUTES,
        "            if !named.contains(&p.name.as_str()) || is_text(sigs, ty) {\n",
        "            if !named.contains(&p.name.as_str()) || true {\n",
    ),
    (
        "an opaque type over text is not text",
        ROUTES,
        "        .is_some_and(|r| r.as_primitive() == Some(Primitive::Str))\n",
        "        .is_some_and(|_| false)\n",
    ),
    (
        "two pages at one route pass",
        ROUTES,
        "                Some((_, other)) if u == unit => {\n",
        "                Some((_, other)) if u == unit && false => {\n",
    ),
    (
        "parameters' names tell two routes apart",
        ROUTES,
        "            .map(|s| match s.starts_with('{') && s.ends_with('}') {\n",
        "            .map(|s| match s.starts_with('{') && s.ends_with('}') && false {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "routes"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout + r.stderr)
        if not found:
            built = False
            continue
        for p, f in found:
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
