#!/usr/bin/env python3
"""Mutation controls for ADR-0047: every name resolves.

Each mutant undoes one piece of the scoped name walk, or one of the two parse
repairs it needed, and the name tests must then fail. A piece whose mutant
survives is a piece nothing tests.

Run from the repository root; `just e10-names` records the output. The source
is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
NAMES = ROOT / "compiler/pw-core/src/names.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the walk is not run",
        CHECK,
        "per_unit.extend(crate::names::check(&workspace, &hirs, i, &u.src));",
        "",
    ),
    (
        "a `let` binds before its initialiser",
        NAMES,
        """                if let Some(init) = *init {
                    self.expr(init);
                }
                if let Some(p) = *pat {
                    let mut names = BTreeSet::new();
                    self.pattern_names(p, &mut names);""",
        """                if let Some(p) = *pat {
                    let mut names = BTreeSet::new();
                    self.pattern_names(p, &mut names);
                    self.bind(names);
                }
                if let Some(init) = *init {
                    self.expr(init);
                }
                if let Some(p) = *pat {
                    let mut names = BTreeSet::new();
                    self.pattern_names(p, &mut names);""",
    ),
    (
        "a block's bindings outlive it",
        NAMES,
        "self.scoped(BTreeSet::new(), |w| w.statements(&stmts));",
        "self.statements(&stmts);",
    ),
    (
        "a lambda's parameters outlive its body",
        NAMES,
        """                let body = *body;
                self.scoped(names, |w| w.expr(body));
            }
            Expr::Match { scrutinee, arms } => {""",
        """                let body = *body;
                self.bind(names);
                self.expr(body);
            }
            Expr::Match { scrutinee, arms } => {""",
    ),
    (
        "a match arm's bindings reach the arms after it",
        NAMES,
        "self.scoped(names, |w| w.expr(arm.body));",
        "self.bind(names); self.expr(arm.body);",
    ),
    (
        "a loop's pattern outlives the loop",
        NAMES,
        """                let body = *body;
                self.scoped(names, |w| w.expr(body));
            }
            Expr::Record { fields, .. } => {""",
        """                let body = *body;
                self.bind(names);
                self.expr(body);
            }
            Expr::Record { fields, .. } => {""",
    ),
    (
        "a record shorthand is not read",
        NAMES,
        "None => self.name(&f.name, f.span.clone()),",
        "None => {}",
    ),
    (
        "a nested template part is walked again in the outer scope",
        NAMES,
        """    fn expr(&mut self, id: ExprId) {
        self.visited.insert(id);""",
        """    fn expr(&mut self, id: ExprId) {""",
    ),
    (
        "an `{#each}` source is not read",
        NAMES,
        "if written && !self.bound(head) && !self.resolves(head) {",
        "if false {",
    ),
    (
        "an `{#each}` item outlives its block",
        NAMES,
        """                    scope.insert(binding);
                }
                self.scoped(scope, |w| {""",
        """                    self.bind([binding]);
                }
                self.scoped(scope, |w| {""",
    ),
    (
        "a `{#match}` arm's binding reaches the arms after it",
        NAMES,
        """                            if in_arm {
                                w.scopes.pop();
                            }
                            if let Some(e) = condition {""",
        """                            if let Some(e) = condition {""",
    ),
    (
        "a stream part's `as` is read as a use",
        NAMES,
        '&& matches!(tag.as_str(), "ready" | "failed")',
        "&& false",
    ),
    (
        "a receiver is not read",
        NAMES,
        """                if !module_shaped {
                    self.name(&h, self.body.expr_span(head));
                }""",
        """                let _ = module_shaped;""",
    ),
    (
        "a module path used as a value is not read",
        NAMES,
        "self.found.push((self.body.expr_span(id), path));",
        "let _ = path;",
    ),
    (
        "a clause needs no value or block",
        NAMES,
        "crate::policy::domain_of(head).is_some() && (block_follows || self.same_line(s, next))",
        "crate::policy::domain_of(head).is_some()",
    ),
    (
        "`release(h)` does not bind `h` in its block",
        NAMES,
        "self.scoped(names, |w| w.expr(block));",
        "let _ = names; self.expr(block);",
    ),
    (
        "a nested declaration does not see its parent's parameters",
        NAMES,
        "outer.extend(enclosing.params.iter().map(|p| p.name.clone()));",
        "",
    ),
    (
        "a spawn form's words are uses",
        NAMES,
        'const SPAWN_FORMS: &[&str] = &["task.spawn", "durable.spawn"];',
        "const SPAWN_FORMS: &[&str] = &[];",
    ),
    (
        "`derived` is a bare name again",
        GRAMMAR,
        """                if self.at_kw("derived") {
                    return self.derived_expr();
                }""",
        "",
    ),
    (
        "a statement's named argument is an assignment again",
        GRAMMAR,
        """                // A named argument, as in a call: `observe intersection(self,
                // threshold = 0.1)`. Until 2026-09-25 this list read `threshold
                // = 0.1` as an assignment to a name nothing declares (ADR-0047).
                if self.at(Kind::Ident) && (self.nth_is(1, Kind::Eq) || self.nth_is(1, Kind::Colon))
                {
                    self.bump();
                    self.bump();
                }""",
        "",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "every_name_resolves"],
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
