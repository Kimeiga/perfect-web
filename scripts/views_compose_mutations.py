#!/usr/bin/env python3
"""Mutation controls for ADR-0136: a view used in another is written where it
is used.

Each mutant undoes one part of composition: finding the view, reading its
parameters as the paths given, hiding nothing a page gave it, carrying a
handler's captures under its own names, the checks made where a view is
used, and the plan and the speculation that read the composed page. The
tests in `views_compose.rs`, `view_elements.rs` and pw-render's
`properties.rs` must then fail.

Run from the repository root; `just e14-views-compose` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = ROOT / "compiler/pw-core/src"
TEMPLATE = CORE / "template_ir.rs"
RESUME = CORE / "resume.rs"
SIGNALS = CORE / "signals.rs"
VALUES = CORE / "values.rs"
CHECK = CORE / "check.rs"
PLAN = CORE / "page_values.rs"
SPECULATION = CORE / "backend/speculation.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a view is not found where it is used",
        TEMPLATE,
        "                    .is_some_and(|d| d.kind == crate::hir::DeclKind::View)\n"
        "                    .then_some(def)\n",
        "                    .is_some_and(|d| d.kind == crate::hir::DeclKind::Page)\n"
        "                    .then_some(def)\n",
    ),
    (
        "a parameter reads as its own name, not as the path it was given",
        TEMPLATE,
        "            (Some(to), Some(rest)) => format!(\"{to}.{rest}\"),\n"
        "            (Some(to), None) => to.clone(),\n",
        "            (Some(_), _) => path.clone(),\n",
    ),
    (
        "a name the view binds is never renamed",
        TEMPLATE,
        "            if hides {\n",
        "            if hides && written.len() > usize::MAX {\n",
    ),
    (
        "a name the view binds does not hide its parameter",
        TEMPLATE,
        "        for b in bound {\n            inner.names.remove(b);\n        }\n",
        "",
    ),
    (
        "a capture is carried under the page's name for it",
        TEMPLATE,
        "                    let to = ctx.names.get(root)?;\n",
        "                    let to = ctx.names.get(root).filter(|_| false)?;\n",
    ),
    (
        "the renderer reads a capture at the handler's name",
        RENDER,
        "                    Some((root, rest)) => renames.get(root).map(|to| format!(\"{to}.{rest}\")),\n"
        "                    None => renames.get(path.as_str()).cloned(),\n",
        "                    Some(_) => None::<String>,\n"
        "                    None => None,\n",
    ),
    (
        "what a view captures is not checked where it is given",
        RESUME,
        "        for (e, tag, given) in captured_props(body, sigs.workspace(), unit, captured) {\n",
        "        for (e, tag, given) in captured_props(body, sigs.workspace(), unit, &BTreeMap::new()) {\n",
    ),
    (
        "a view's handler's capture is not traced to its parameter",
        RESUME,
        "        for (_, _, e) in captures_of(body, &lexical, lambda) {\n"
        "            found.extend(param(e));\n",
        "        for (_, _, e) in captures_of(body, &lexical, lambda) {\n"
        "            let _ = param(e);\n",
    ),
    (
        "a capture of the view's own loop is not traced to its list",
        RESUME,
        "            .and_then(|head| param_of_binder(lexical, body, head)),\n",
        "            .and_then(|_| None),\n",
    ),
    (
        "a signal a view's handler captures is not refused",
        SIGNALS,
        "                    given.iter().find(|(e, ..)| within(body, *e, id))\n",
        "                    given.iter().find(|_| false)\n",
    ),
    (
        "a view's props are not checked",
        VALUES,
        "                crate::hir::Node::Element { tag, attrs, .. } if self.view_named(tag).is_some() => {\n",
        "                crate::hir::Node::Element { tag, attrs, .. } if self.view_named(tag).is_none() => {\n",
    ),
    (
        "a view is looked up among what is called",
        VALUES,
        "        let def = match self.ws.resolve_in(self.at, Namespace::Ui, tag) {\n",
        "        let def = match self.ws.resolve_in(self.at, Namespace::Term, tag) {\n",
    ),
    (
        "every view used in another is refused again",
        CHECK,
        "            if let Some((def, DeclKind::View)) = resolved {\n",
        "            if let Some((def, DeclKind::Fn)) = resolved {\n",
    ),
    (
        "a computed prop composes",
        CHECK,
        "                            if crate::template_ir::value_path_of(body, *e).is_none() =>\n",
        "                            if crate::template_ir::value_path_of(body, *e).is_some() && false =>\n",
    ),
    (
        "a view with values of its own composes",
        TEMPLATE,
        "            .all(|s| matches!(body.expr(*s), Expr::Template { .. })),\n        Expr::Template { .. } => true,\n",
        "            .any(|s| matches!(body.expr(*s), Expr::Template { .. })),\n        Expr::Template { .. } => true,\n",
    ),
    (
        "the plan skips what a view shows",
        PLAN,
        "    for hole in holes {\n",
        "    for hole in holes.into_iter().filter(|h| h.origin == (unit, id)) {\n",
    ),
    (
        "a view's handler is not the page's to speculate for",
        SPECULATION,
        "            .map(|l| l.views)\n",
        "            .map(|_| Vec::<DefId>::new())\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "views_compose"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "view_elements"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "properties"],
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
