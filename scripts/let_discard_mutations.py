#!/usr/bin/env python3
"""Mutation controls for ADR-0250 (ruling 0099-a): `let _ = e` is an
explicit discard, and an acquisition is held or refused.

Each mutant undoes one piece: the discard's parse, its refusals of `let mut
_` and `use _`, its lowering, its reading, PW0618's repair and the Koka
translation; and each holder of an acquisition, and each way one is
dropped, in the affine check; what a declaration gives its caller; the
inference of `e?` and of branches; and which functions the repair names.
The tests of each must then fail.

Run from the repository root; `just e14-let-discard` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"
ANNOTATIONS = ROOT / "compiler/pw-core/src/annotations.rs"
KOKA = ROOT / "compiler/pw-core/src/koka.rs"
BACKEND = ROOT / "compiler/pw-core/src/backend/lower.rs"
AFFINE = ROOT / "compiler/pw-core/src/affine.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "`let _` does not parse",
        GRAMMAR,
        "        if self.at(Kind::Underscore) {\n            // `let mut _`: nothing is bound to change.",
        "        if false && self.at(Kind::Underscore) {\n            // `let mut _`: nothing is bound to change.",
    ),
    (
        "`let mut _` is accepted",
        GRAMMAR,
        "            if mutable {\n                self.error_help(\n",
        "            if false && mutable {\n                self.error_help(\n",
    ),
    (
        "`use _` reads as two statements",
        GRAMMAR,
        "        if keyword == \"use\" && self.at(Kind::Underscore) {\n",
        "        if false && keyword == \"use\" && self.at(Kind::Underscore) {\n",
    ),
    (
        "`_` is lowered to no pattern",
        LOWER,
        "                    .map(|w| b.pat(Pattern::Wild, span_of(&w)))\n",
        "                    .filter(|_| false)\n                    .map(|w| b.pat(Pattern::Wild, span_of(&w)))\n",
    ),
    (
        "reading `_` is told to bind it",
        NAMES,
        "    let (message, explanation, repair) = if name == \"_\" {\n",
        "    let (message, explanation, repair) = if false && name == \"_\" {\n",
    ),
    (
        "PW0618's repair names the discard by name",
        ANNOTATIONS,
        "                \"pass the failure on with `?`, handle it with `match`, or discard it, `let _ = ..`\",\n",
        "                \"pass the failure on with `?`, handle it with `match`, or discard it by name, `let _ignored = ..`\",\n",
    ),
    (
        "Koka refuses `_`",
        KOKA,
        "                            Some(Pattern::Wild) => \"_\".to_string(),\n",
        "                            Some(Pattern::Wild) => return Err(\"a destructuring binding\"),\n",
    ),
    (
        "the backend refuses `_`",
        BACKEND,
        "            Some(Pattern::Wild) | None => Lowering::Lowered(()),\n",
        "            None => Lowering::Lowered(()),\n",
    ),
    (
        "an acquisition nothing holds is not reported",
        AFFINE,
        "        for u in unheld {\n",
        "        for u in unheld.into_iter().filter(|_| false) {\n",
    ),
    (
        "`_` holds what it is given",
        AFFINE,
        "                        Some((_, crate::hir::Pattern::Wild)) => {\n                            Holder::Nothing(Dropped::Discarded(body.expr_span(p)))\n",
        "                        Some((_, crate::hir::Pattern::Wild)) => {\n                            Holder::Kept\n",
    ),
    (
        "a statement holds its value",
        AFFINE,
        "                    if i + 1 < stmts.len() {\n                        return Holder::Nothing(Dropped::Statement(",
        "                    if false && i + 1 < stmts.len() {\n                        return Holder::Nothing(Dropped::Statement(",
    ),
    (
        "an `acquire` clause holds nothing",
        AFFINE,
        "                    if matches!(before, Some(Expr::Name(n)) if n == \"acquire\")\n",
        "                    if false && matches!(before, Some(Expr::Name(n)) if n == \"acquire\")\n",
    ),
    (
        "a branch's value is not its construct's",
        AFFINE,
        "                Expr::If { cond, .. } if *cond != at => {}\n",
        "                Expr::If { cond, .. } if *cond != at && false => {}\n",
    ),
    (
        "a `?` keeps what it holds",
        AFFINE,
        "                Expr::Match { .. } | Expr::Try { .. } => {}\n",
        "                Expr::Match { .. } => {}\n",
    ),
    (
        "a match holds what it matches",
        AFFINE,
        "                Expr::Match { scrutinee, .. } if *scrutinee == at => {\n",
        "                Expr::Match { scrutinee, .. } if *scrutinee == at && false => {\n",
    ),
    (
        "a function value's result is the caller's",
        AFFINE,
        "                Expr::Lambda { .. } => {\n                    return Holder::Nothing(Dropped::Returned(body.expr_span(p)));\n",
        "                Expr::Lambda { .. } => {\n                    return Holder::Kept;\n",
    ),
    (
        "a function value's `return` is the declaration's",
        AFFINE,
        "                            Some(l) => Holder::Nothing(Dropped::Returned(body.expr_span(l))),\n",
        "                            Some(_) => self.given(at),\n",
    ),
    (
        "every call releases what it is given",
        AFFINE,
        "            Some(_) => Holder::Nothing(Dropped::Taken(span, path_of(self.body, callee))),\n",
        "            Some(_) => Holder::Kept,\n",
    ),
    (
        "no call releases what it is given",
        AFFINE,
        "                    .any(|e| type_argument(e, \"resource.release\").as_deref() == Some(self.ty)) =>\n",
        "                    .any(|e| type_argument(e, \"resource.release\").as_deref() == Some(self.ty))\n                    && false =>\n",
    ),
    (
        "`Some(..)` keeps nothing it is given",
        AFFINE,
        "    (matches!(n.as_str(), \"Ok\" | \"Err\" | \"Some\") && types.lexical().binder(*callee).is_none())\n",
        "    (false && matches!(n.as_str(), \"Ok\" | \"Err\" | \"Some\") && types.lexical().binder(*callee).is_none())\n",
    ),
    (
        "a body declaring `()` gives its caller its value",
        AFFINE,
        "        .is_some_and(|r| r.as_primitive() != Some(crate::resolved::Primitive::Unit))\n",
        "        .is_some_and(|_| true)\n",
    ),
    (
        "no declaration gives its caller its value",
        AFFINE,
        "    if !matches!(\n        decl.kind,\n        DeclKind::Fn | DeclKind::Query | DeclKind::Command | DeclKind::Task\n    ) {\n",
        "    if true || !matches!(\n        decl.kind,\n        DeclKind::Fn | DeclKind::Query | DeclKind::Command | DeclKind::Task\n    ) {\n",
    ),
    (
        "a `return`'s value is not the body's",
        AFFINE,
        "        returned(body, types, body.root, &mut out);\n",
        "        let _ = returned;\n",
    ),
    (
        "a body declaring `()` moves a bound value to its caller",
        AFFINE,
        "    let mut out = BTreeSet::new();\n    if gives {\n",
        "    let mut out = BTreeSet::new();\n    if gives || true {\n",
    ),
    (
        "the repair names the function being checked",
        AFFINE,
        "                        .is_some_and(|t| t.written_source() == ty)\n",
        "                        .is_some_and(|t| t.written_source() == ty)\n                        || true\n",
    ),
    (
        "`e?` has no type",
        INFER,
        "                    Some(Builtin::Result | Builtin::Option) => t.args().first().cloned(),\n",
        "                    Some(Builtin::Result | Builtin::Option) => None,\n",
    ),
    (
        "branches that agree have no type",
        INFER,
        "                    .is_some_and(|u| u.same_as(&t))\n                    .then_some(t)\n",
        "                    .is_some_and(|u| u.same_as(&t) && false)\n                    .then_some(t)\n",
    ),
    (
        "arms that agree have no type",
        INFER,
        "                    .all(|a| self.of(body, a.body).is_some_and(|u| u.same_as(&t)))\n",
        "                    .all(|a| self.of(body, a.body).is_some_and(|u| u.same_as(&t) && false))\n",
    ),
    (
        "a block has no type",
        INFER,
        "            Expr::Block { stmts } => self.of(body, *stmts.last()?),\n",
        "            Expr::Block { .. } => None,\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib", "--",
        "a_let_may_discard_its_value", "a_discard_is_neither_mutable_nor_a_use",
    ],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "let_discard"],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "koka_backend",
        "--", "a_discard_is_kokas_wildcard",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "handlers",
        "--", "a_handler_that_discards_its_answer",
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
