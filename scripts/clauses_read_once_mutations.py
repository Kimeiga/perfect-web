#!/usr/bin/env python3
"""Mutation controls for ADR-0240: a clause is read once.

Each mutant undoes one piece: the resource graph reading the keys lowering
made, each labelled as written, and a key that names nothing refused at its
name; an interface's clause terms lowered into an arena of their own; and
each rule that reads a term reading an interface's: a key's name resolved, a
listener's key bound, a key typed, a speculation gathered, a transition's
effects, its value and its binders checked. The tests of each must then
fail.

Run from the repository root; `just e14-clauses-read-once` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAPH = ROOT / "compiler/pw-core/src/graph.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"

ANY_BODY = "        let Some(body_id) = decl.terms_body() else {\n            continue;\n        };\n"
ITS_BODY = "        let Some(body_id) = decl.body else {\n            continue;\n        };\n"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the graph reads a clause's first key alone",
        GRAPH,
        "                    for k in &p.keys {\n",
        "                    for k in p.keys.iter().take(1) {\n",
    ),
    (
        "an edge's key is not labelled as written",
        GRAPH,
        "                        g.push_edge(&from, &k.name, kind, k.written.clone(), resolve(&k.name));\n",
        "                        g.push_edge(&from, &k.name, kind, Vec::new(), resolve(&k.name));\n",
    ),
    (
        "a key that names nothing is refused at its clause",
        GRAPH,
        "                    primary_span: key.name_span.clone(),\n",
        "                    primary_span: p.span.clone(),\n",
    ),
    (
        "an interface's terms are not lowered",
        LOWER,
        "            None => self.interface_terms(id, &mut policies, span_of(node)),\n",
        "            None => None,\n",
    ),
    (
        "an interface's term names are not resolved",
        CHECK,
        "        // An interface's terms too, in their own arena (ADR-0240).\n" + ANY_BODY,
        "        // An interface's terms too, in their own arena (ADR-0240).\n" + ITS_BODY,
    ),
    (
        "an interface's listener keys are not bound",
        CHECK,
        "        // An interface's listeners too (ADR-0240).\n"
        "        let Some(body) = decl.terms_body().map(|b| hir.body(b)) else {\n",
        "        // An interface's listeners too (ADR-0240).\n"
        "        let Some(body) = decl.body.map(|b| hir.body(b)) else {\n",
    ),
    (
        "an interface's keys are not typed",
        VALUES,
        "        // An interface's keys are typed too, in their own arena (ADR-0240).\n" + ANY_BODY,
        "        // An interface's keys are typed too, in their own arena (ADR-0240).\n" + ITS_BODY,
    ),
    (
        "an interface command's speculations are not gathered",
        CHECK,
        "        let Some(body) = decl.terms_body().map(|b| hir.body(b)) else {\n"
        "            continue;\n"
        "        };\n"
        "        for (policy, target, _) in decl.optimistic_clauses() {\n",
        "        let Some(body) = decl.body.map(|b| hir.body(b)) else {\n"
        "            continue;\n"
        "        };\n"
        "        for (policy, target, _) in decl.optimistic_clauses() {\n",
    ),
    (
        "an interface's transition may perform effects",
        CHECK,
        ANY_BODY + "        let body = hir.body(body_id);\n"
        "        // **Every root, asked its own context's question.**",
        ITS_BODY + "        let body = hir.body(body_id);\n"
        "        // **Every root, asked its own context's question.**",
    ),
    (
        "an interface's transition's target need not be a resource's entry",
        CHECK,
        ANY_BODY + "        let body = hir.body(body_id);\n"
        "        for (policy, target, _) in decl.optimistic_clauses() {\n",
        ITS_BODY + "        let body = hir.body(body_id);\n"
        "        for (policy, target, _) in decl.optimistic_clauses() {\n",
    ),
    (
        "an interface's term may bind a name twice",
        CHECK,
        "        if let Some(body_id) = decl.terms_body() {\n",
        "        if let Some(body_id) = decl.body {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "clauses_read_once"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-cli", "--", "the_committed_graph_matches"],
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
