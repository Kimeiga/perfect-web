#!/usr/bin/env python3
"""Mutation controls for ADR-0251: a resource's clauses are held to PW2005.

Each mutant undoes one piece: the term roots walked; the `acquire` clause
holding its value, a clause's value dropped, and a statement's `release`
block's; the obligation a `release` clause owes, in each form, and only
where its `acquire` acquires; the clause it runs through, the releases and
bindings counted in it, an alias read in it; what an `acquire` clause binds
moving to its resource; and the label the diagnostic gives. The tests of
each must then fail.

Run from the repository root; `just e14-resource-clauses` records the
output. The source is restored after every mutant, whatever happens.
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
        "a clause's terms are not walked",
        AFFINE,
        "        .chain(decl.term_roots().map(|(_, r)| r.root))\n        .flat_map(|r| body.walk_from(r))\n",
        "        .chain(decl.term_roots().map(|(_, r)| r.root).filter(|_| false))\n        .flat_map(|r| body.walk_from(r))\n",
    ),
    (
        "an `acquire` clause holds nothing",
        AFFINE,
        "                if self.resource.contains(&at) {\n",
        "                if false && self.resource.contains(&at) {\n",
    ),
    (
        "a clause's value is the declaration's",
        AFFINE,
        "                if at != body.root {\n                    return Holder::Nothing(Dropped::Clause(",
        "                if false && at != body.root {\n                    return Holder::Nothing(Dropped::Clause(",
    ),
    (
        "a statement's `release` block holds its value",
        AFFINE,
        "                    if matches!(before, Some(Expr::Call { callee, .. })\n",
        "                    if false && matches!(before, Some(Expr::Call { callee, .. })\n",
    ),
    (
        "a release clause owes nothing",
        AFFINE,
        "        owned.extend(released_clauses(body, decl, sigs, &types, &reached));\n",
        "        let _ = released_clauses;\n",
    ),
    (
        "a declaration's release clause owes nothing",
        AFFINE,
        "        if r.context != Cx::Release {\n            continue;\n        }\n",
        "        if true || r.context != Cx::Release {\n            continue;\n        }\n",
    ),
    (
        "a statement's release clause owes nothing",
        AFFINE,
        "    for (clause, acquire) in types.lexical().released() {\n",
        "    for (clause, acquire) in types.lexical().released().filter(|_| false) {\n",
    ),
    (
        "a release owes what its acquire did not acquire",
        AFFINE,
        "            .find_map(|x| type_argument(x, \"resource.acquire\"))\n",
        "            .find_map(|x| type_argument(x, \"resource.acquire\").or(Some(\"MapHandle\".to_string())))\n",
    ),
    (
        "a release clause's obligation runs through the body",
        AFFINE,
        "        (paths.expr(clause), body.expr_span(clause))\n",
        "        (paths.expr(body.root), body.expr_span(body.root))\n",
    ),
    (
        "a release in a clause is not counted",
        AFFINE,
        "    let (mut out, mut given) = (Vec::new(), Vec::new());\n    for &id in reached {\n",
        "    let (mut out, mut given) = (Vec::new(), Vec::new());\n    for &id in &body.walk() {\n",
    ),
    (
        "a binding in a clause is not followed",
        AFFINE,
        "        let (stmts, at) = reached.iter().find_map(|&b| match body.expr(b) {\n",
        "        let (stmts, at) = body.walk().into_iter().find_map(|b| match body.expr(b) {\n",
    ),
    (
        "what an `acquire` clause binds does not move to its resource",
        AFFINE,
        "    for r in resource {\n        value_of(body, types, *r, &mut out);\n    }\n",
        "    for r in resource.iter().filter(|_| false) {\n        value_of(body, types, *r, &mut out);\n    }\n",
    ),
    (
        "an alias in a clause is not read",
        AFFINE,
        "    body.exprs().find_map(|(_, e, _)| match e {\n",
        "    body.walk().into_iter().map(|id| ((), body.expr(id), ())).find_map(|(_, e, _)| match e {\n",
    ),
    (
        "the label calls what a clause is given acquired here",
        AFFINE,
        "    let acquired = if a.clause.is_some() {\n",
        "    let acquired = if false && a.clause.is_some() {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "resource_clauses"],
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
