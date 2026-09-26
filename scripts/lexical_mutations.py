#!/usr/bin/env python3
"""Mutation controls for ADR-0063: every name means one binding.

Each mutant undoes one piece of how a local name is resolved to the binding
it means (`lexical.rs`), or of how a binding is typed or labelled by where it
is bound: in the value relations, the declared-type environment, a handler's
capture, and the privacy labels. The lexical-scope tests and the
exhaustiveness tests must then fail.

Run from the repository root; `just e10-lexical` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a block's names outlive it",
        LEXICAL,
        "                            self.out.nested.insert(*child, scope.clone());\n"
        "                        }\n"
        "                    }\n"
        "                }\n"
        "                scope.truncate(depth);",
        "                            self.out.nested.insert(*child, scope.clone());\n"
        "                        }\n"
        "                    }\n"
        "                }",
    ),
    (
        "a lambda's parameters outlive it",
        LEXICAL,
        "                self.expr(*body, scope);\n                scope.truncate(depth);\n            }\n            Expr::Match",
        "                self.expr(*body, scope);\n            }\n            Expr::Match",
    ),
    (
        "a `let` is in scope in its own initialiser",
        LEXICAL,
        "            Expr::Let { pat, init, .. } => {\n"
        "                if let Some(init) = init {\n"
        "                    self.expr(*init, scope);\n"
        "                }\n"
        "                if let Some(p) = pat {\n"
        "                    self.pattern(*p, scope);\n"
        "                }\n"
        "            }",
        "            Expr::Let { pat, init, .. } => {\n"
        "                if let Some(p) = pat {\n"
        "                    self.pattern(*p, scope);\n"
        "                }\n"
        "                if let Some(init) = init {\n"
        "                    self.expr(*init, scope);\n"
        "                }\n"
        "            }",
    ),
    (
        "a lambda's parameters bind nothing",
        LEXICAL,
        "                for p in params {\n                    self.pattern(*p, scope);\n                }",
        "                let _ = params;",
    ),
    (
        "a match arm's names bind nothing",
        LEXICAL,
        "                    self.pattern(arm.pat, scope);",
        "                    let _ = arm.pat;",
    ),
    (
        "a `for` loop's names bind nothing",
        LEXICAL,
        "                if let Some(p) = pat {\n                    self.pattern(*p, scope);\n                }\n                self.expr(*body, scope);",
        "                let _ = pat;\n                self.expr(*body, scope);",
    ),
    (
        "an `{#each}` block binds nothing",
        LEXICAL,
        "                        scope.push((name, Binder::Each(*n)));",
        "                        let _ = name;",
    ),
    (
        "a `{#match}` arm binds nothing",
        LEXICAL,
        "                            scope.push((name.clone(), Binder::Arm(*n, i)));",
        "                            let _ = (name, i);",
    ),
    (
        "a record's shorthand field names no binding",
        LEXICAL,
        "                                self.out.shorthand.insert((id, i), b);",
        "                                let _ = (id, i, b);",
    ),
    (
        "a `for` loop's name is not its list's element",
        VALUES,
        "                        added |= self.bind(Binder::Pattern(*pat), element.clone());",
        "                        let _ = element;",
    ),
    (
        "an `{#each}` collection's fields are not read",
        VALUES,
        "                t = self.member_type(&t, segment);",
        "                t = Ty::Unknown;",
    ),
    (
        "a `{#match}` arm's names are not typed",
        VALUES,
        "                added |= self.bind(Binder::Arm(node, i), f);",
        "                let _ = (node, i, f);",
    ),
    (
        "a record's shorthand is read by its name",
        VALUES,
        "                None => match self.lexical.shorthand(id, i) {\n"
        "                    Some(b) => self.local(b),\n"
        "                    None => self.global(&init.name),\n"
        "                },",
        "                None => self.global(&init.name),",
    ),
    (
        "the declared-type environment reads no local's binding",
        INFER,
        "                Some(b) => self.bindings.get(&b).cloned(),",
        "                Some(_) => None,",
    ),
    (
        "a handler's capture is typed by no binding",
        LOWER,
        "        let mut ty = roots.get(root).and_then(|e| types.of(body, *e));",
        "        let mut ty: Option<crate::resolved::ResolvedType> = roots.get(root).and(None);",
    ),
    (
        "a `for` loop's name carries no label",
        LABELS,
        "                    } => (me.label(body, *iterable), vec![*p]),",
        "                    } => (Label::public(), vec![*p]),",
    ),
    (
        "a lambda's parameters carry no label",
        LABELS,
        "                            for p in params {\n                                for (q, span) in bound_binders(body, *p) {",
        "                            for p in params.iter().filter(|_| false) {\n                                for (q, span) in bound_binders(body, *p) {",
    ),
    (
        "an `{#each}` block's name carries no label",
        LABELS,
        "                    me.bindings\n                        .entry(Binder::Each(n))\n                        .or_insert((l, body.node_span(n)));",
        "                    let _ = (n, l);",
    ),
    (
        "a name's label is not read through its binding",
        LABELS,
        "            Expr::Name(_) => self\n                .types\n                .lexical()\n                .binder(id)\n",
        "            Expr::Name(_) => self\n                .types\n                .lexical()\n                .binder(id)\n                .filter(|_| false)\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "lexical_scope"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "match_exhaustiveness"],
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
