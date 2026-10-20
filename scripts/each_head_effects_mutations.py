#!/usr/bin/env python3
"""Mutation controls for ADR-0345: an `{#each}`'s list and key are
expressions every walker of a view reads (the each-head-effects track).

Each mutant takes one position away from the one reading of a node's
expressions (`Node::exprs`), or one reader's visit of it: the template's
parts without a loop's head (the reading before the fix); the head not
lowered; the list read in no scope, or the key outside the row's; the
names check reading neither; a row's label, type or dialog not its
list's; Marko writing the list as a name; and the printed-handle rule
printing a loop's list. The tests of `each_head_effects.rs`,
`view_positions.rs`, `each_heads.rs` and the dialogs' must then fail.

Run from the repository root; `just e14-each-head-effects` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HIR = ROOT / "compiler/pw-core/src/hir.rs"
LOWER = ROOT / "compiler/pw-core/src/lower.rs"
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"
LABELS = ROOT / "compiler/pw-core/src/labels.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
MARKO = ROOT / "compiler/pw-core/src/marko.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a text hole is no position",
        HIR,
        "            Node::Interpolation(e) => vec![(Position::Hole, *e)],\n",
        "            Node::Interpolation(_) => Vec::new(),\n",
    ),
    (
        "an attribute's value is no position",
        HIR,
        "                        AttrValue::Expr(e) => Some((position, *e)),\n",
        "                        AttrValue::Expr(e) => Some((position, *e))\n"
        "                            .filter(|(p, _)| *p != Position::Attribute),\n",
    ),
    (
        "a handler is no position",
        HIR,
        "                        AttrValue::Expr(e) => Some((position, *e)),\n",
        "                        AttrValue::Expr(e) => Some((position, *e))\n"
        "                            .filter(|(p, _)| *p != Position::Handler),\n",
    ),
    (
        "a handler is read as an attribute",
        HIR,
        "                        Some(_) => Position::Handler,\n",
        "                        Some(_) => Position::Attribute,\n",
    ),
    (
        "a block's subject is no position",
        HIR,
        "                    subject.iter().map(|s| (Position::Subject, *s)).collect();\n",
        "                    subject.iter().map(|s| (Position::Subject, *s)).filter(|_| false).collect();\n",
    ),
    (
        "a branch's condition is no position",
        HIR,
        "                    .map(|c| (Position::Condition, *c))\n",
        "                    .map(|c| (Position::Condition, *c))\n"
        "                    .filter(|_| false)\n",
    ),
    (
        "a loop's list is no position",
        HIR,
        "                    out.push((Position::List, *list));\n",
        "                    let _ = list;\n",
    ),
    (
        "a loop's key is no position",
        HIR,
        "                    out.extend(key.iter().map(|k| (Position::Key, *k)));\n",
        "                    let _ = key;\n",
    ),
    (
        "a template's parts leave a loop's head out, as before the fix",
        LOWER,
        "                    .flat_map(|(_, n, _)| n.exprs())\n",
        "                    .flat_map(|(_, n, _)| {\n"
        "                        n.exprs().into_iter().filter(|(p, _)| {\n"
        "                            !matches!(p, crate::hir::Position::List | crate::hir::Position::Key)\n"
        "                        })\n"
        "                    })\n",
    ),
    (
        "a loop's list is lowered to nothing",
        LOWER,
        "        let list = sub.expr(b, &list);\n",
        "        let list = b.expr(Expr::Error, span_of(&list));\n",
    ),
    (
        "a loop's key is lowered to nothing",
        LOWER,
        "        let key = key.as_ref().map(|k| sub.expr(b, k));\n",
        "        let key = key.as_ref().map(|k| b.expr(Expr::Error, span_of(k)));\n",
    ),
    (
        "a loop's list is read in no scope",
        LEXICAL,
        "                        self.expr(each.list, scope);\n",
        "                        let _ = each.list;\n",
    ),
    (
        "a loop's key is read outside the row's scope",
        LEXICAL,
        "                        scope.push((each.binder.clone(), Binder::Each(*n)));\n"
        "                        if let Some(key) = each.key {\n"
        "                            self.expr(key, scope);\n"
        "                        }\n",
        "                        if let Some(key) = each.key {\n"
        "                            self.expr(key, scope);\n"
        "                        }\n"
        "                        scope.push((each.binder.clone(), Binder::Each(*n)));\n",
    ),
    (
        "the names check reads no loop's list",
        NAMES,
        "                    self.expr(each.list);\n",
        "                    let _ = each.list;\n",
    ),
    (
        "the names check reads a loop's key outside the row's scope",
        NAMES,
        "                        w.expr(k);\n",
        "                        let _ = k;\n",
    ),
    (
        "a row's label is not its list's",
        LABELS,
        "                let l = me.label(body, list);\n",
        "                let l = Label::public();\n",
    ),
    (
        "a computed list's rows have no type",
        INFER,
        "                    None if crate::lexical::path_root(body, *list).is_none() => types\n",
        "                    None if false && crate::lexical::path_root(body, *list).is_none() => types\n",
    ),
    (
        "a loop's list shows no dialog",
        SIGNALS,
        "                let each = head.as_ref().and_then(|h| signal_of(h.list));\n",
        "                let each = head.as_ref().and_then(|h| signal_of(h.list)).filter(|_| false);\n",
    ),
    (
        "Marko writes a loop's list as a name, as before the fix",
        MARKO,
        "            source: expr_text(body, each.list, ctx)?,\n",
        "            source: ctx.name(&each.written.list),\n",
    ),
    (
        "a loop's list is printed",
        CHECK,
        "                    | crate::hir::Position::List\n"
        "                    | crate::hir::Position::Key => false,\n",
        "                    | crate::hir::Position::Key => false,\n"
        "                    crate::hir::Position::List => true,\n",
    ),
    # Not a mutant: "a handler is printed". A handler's value is a function
    # (PW0614 refuses any other), whose type never holds a session's handle,
    # so the rule printing it refuses nothing more: what a handler captures
    # is the resume manifest's (PW5007). It survived on 2026-10-15, an
    # equivalent mutant, and is left out.
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "each_head_effects", "--test", "view_positions",
        "--test", "each_heads", "--test", "dialogs",
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
