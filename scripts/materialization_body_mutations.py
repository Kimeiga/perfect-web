#!/usr/bin/env python3
"""Mutation controls for ADR-0273: a materialization is a value its body
derives.

Each mutant undoes one piece: its body held to its declared type; a
materialization a term, read by `query`; a `depends_on` beside a body
refused; its body's reads the graph's edges; one that derives its value read
by another that derives its own, and a fragment the host renders read by
none; a materialization called by nothing; `depends_on` naming one among the
terms; and, inside a block, a clause's value ending with its line unless it
cannot have, where a header's reads on. The checker's tests must then fail.

Run from the repository root; `just e14-materialization-bodies` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
RESOLVE = ROOT / "compiler/pw-core/src/resolve.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
GRAPH = ROOT / "compiler/pw-core/src/graph.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a materialization's body is not held to its type",
        VALUES,
        "    ) || (decl.kind == DeclKind::Materialize && decl.ret.is_some())\n",
        "    )\n",
    ),
    (
        "a materialization is no term",
        RESOLVE,
        "            | DeclKind::Materialize => Namespace::Term,\n"
        "            DeclKind::View | DeclKind::Component | DeclKind::Page => Namespace::Ui,\n",
        "            => Namespace::Term,\n"
        "            DeclKind::View | DeclKind::Component | DeclKind::Page | DeclKind::Materialize => Namespace::Ui,\n",
    ),
    (
        "a `depends_on` beside a body is not refused",
        CHECK,
        "        per_unit.extend(dependencies_stated_once(&u.hir));\n",
        "",
    ),
    (
        "a body's reads are no edges",
        GRAPH,
        "                if (decl.kind == DeclKind::Page\n"
        "                    || (decl.kind == DeclKind::Materialize && decl.ret.is_some()))\n",
        "                if (decl.kind == DeclKind::Page)\n",
    ),
    (
        "one that derives its value reads none that derives its own",
        CHECK,
        "                if derives && deriving {\n",
        "                if derives && deriving && false {\n",
    ),
    (
        "a fragment the host renders is read as a value",
        CHECK,
        "                let derives = sigs.by_def(def).is_some_and(|s| s.returns.is_some());\n",
        "                let derives = true;\n",
    ),
    (
        "a materialization may be called",
        CHECK,
        "        && crate::resolve::declaration(hirs, d).is_some_and(|x| x.kind == DeclKind::Materialize)\n",
        "        && false\n",
    ),
    (
        "`depends_on` looks for a materialization among the views",
        POLICY,
        "    const READ: &[(Namespace, &[K])] = &[(\n        Namespace::Term,\n        &[K::Query, K::Subscription, K::Resource, K::Materialize],\n",
        "    const READ: &[(Namespace, &[K])] = &[(\n        Namespace::Term,\n        &[K::Query, K::Subscription, K::Resource],\n",
    ),
    (
        "inside a block, a clause's value reads on past its line",
        GRAMMAR,
        "                if in_block\n                    && depth == 0\n                    && self.newline_ahead()\n",
        "                if false\n                    && depth == 0\n                    && self.newline_ahead()\n",
    ),
    (
        "a header's clause ends with its line",
        GRAMMAR,
        "                if in_block\n                    && depth == 0\n                    && self.newline_ahead()\n",
        "                if depth == 0\n                    && self.newline_ahead()\n",
    ),
    (
        "a line ending in a comma ends its clause",
        GRAMMAR,
        "                            Kind::Comma\n                                | Kind::Colon\n",
        "                            Kind::Colon\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "materialization_bodies", "--test", "materialization_chains",
        "--test", "calls_name_terms", "--test", "speculated_arms",
    ],
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
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
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
