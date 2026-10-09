#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a page is shown in its layout, which the
pages that name it share.

Each mutant undoes one piece:
- the composition: the slot not recorded, the page's markup not placed in
  it, the layout's bindings not renamed, its computed parts named by the
  page, its `provide` giving its views nothing, the layout not lowered as
  the slot's owner;
- the plan: the layout's bindings planned after the page's, or not found
  by the parts that read them;
- the checks: a page's `layout` not held to name a layout, nor a page to
  one; a layout given parameters; a slot in a block, in a stream, twice, or
  missing; a page shown in a layout of a narrower audience;
- what the page holds: its privacy label without its layout's, its reads in
  the graph without its layout's, a layout's views provided nothing, its
  speculation without its layout's bindings;
- the clause: `layout` allowed on any declaration, and taken for a clause
  where no name follows it;
- the host: a query a page and its layout both bind read twice for one
  document.

A compiler mutant must fail `pw-core`'s `tests/layouts.rs`; the host's, the
host's tests of the store's pages in their layout and of what a render
reads. Run from the repository root; `just e14-layouts` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
LAYOUTS = ROOT / "compiler/pw-core/src/layouts.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
GRAPH = ROOT / "compiler/pw-core/src/graph.rs"
POLICY = ROOT / "compiler/pw-core/src/policy.rs"
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"

# (what, file, anchor, replacement). Each must fail the compiler's suite,
# but those in `HOST`, the host's.
MUTANTS = [
    (
        "the slot is not recorded",
        TEMPLATE,
        "            ix.slot = Some(Slot {\n",
        "            let _ = Some(Slot {\n",
    ),
    (
        "the page's markup is placed after the layout's, not in its slot",
        TEMPLATE,
        "            chunks.splice(s.at..s.at, own);\n",
        "            chunks.extend(own);\n",
    ),
    (
        "the layout's bindings are read by their own names",
        TEMPLATE,
        "        names.insert(name.clone(), crate::layouts::bound(&decl.name, &name));\n",
        "        names.insert(name.clone(), name.clone());\n",
    ),
    (
        "the layout's computed parts are named by the page",
        TEMPLATE,
        "    let page = std::mem::replace(&mut ix.template, path.clone());\n",
        "    let page = ix.template.clone();\n",
    ),
    (
        "the layout's `provide` gives its views nothing",
        TEMPLATE,
        "        provided.insert(signal, fresh);\n        signals.insert(name);\n    }\n"
        "    // Its handlers are in the page's document",
        "        let _ = (signal, fresh);\n        signals.insert(name);\n    }\n"
        "    // Its handlers are in the page's document",
    ),
    (
        "no declaration owns the slot",
        TEMPLATE,
        "        let own = ix.layout == Some((ctx.unit, ctx.decl));\n",
        "        let own = false;\n",
    ),
    (
        "the layout's bindings are planned after the page's",
        PLAN,
        "        bindings.splice(0..0, planned);\n",
        "        bindings.extend(planned);\n",
    ),
    (
        "the layout's bindings are not found by the parts that read them",
        PLAN,
        "            found.push((named, resource, keys));\n",
        "            let _ = (named, resource, keys);\n",
    ),
    (
        "a page's `layout` may name anything",
        LAYOUTS,
        "        if kind != Some(DeclKind::Layout) {\n",
        "        if kind.is_none() && false {\n",
    ),
    (
        "a page may name two layouts",
        LAYOUTS,
        "        if let [_, second, ..] = clauses.as_slice() {\n",
        "        if let [_, second, _, ..] = clauses.as_slice() {\n",
    ),
    (
        "a layout may declare parameters",
        LAYOUTS,
        "        if decl.kind == DeclKind::Layout\n            && let Some(first) = decl.params.first()\n",
        "        if decl.kind == DeclKind::Layout\n            && let Some(first) = decl.params.get(usize::MAX)\n",
    ),
    (
        "a slot may be in a block",
        LAYOUTS,
        "        } else if !top {\n",
        "        } else if false {\n",
    ),
    (
        "a slot may be in a stream",
        LAYOUTS,
        '                let inside = top && tag != "stream";\n',
        "                let inside = top;\n",
    ),
    (
        "a layout may hold two slots",
        LAYOUTS,
        "        } else if placed {\n",
        "        } else if false {\n",
    ),
    (
        "a layout may hold no slot",
        LAYOUTS,
        "    if decl.kind == DeclKind::Layout && !placed {\n",
        "    if false {\n",
    ),
    (
        "a page may be shown in a layout of a narrower audience",
        LAYOUTS,
        "            (Some(l), Some(p)) => !l.flows_into(p),\n",
        "            (Some(_), Some(_)) => false,\n",
    ),
    (
        "a layout's bindings are not speculated on with the page's",
        SPECULATION,
        "    bindings.extend(layout_bindings(cx.hirs, cx.ws, unit, hir.decl(page_id)));\n",
        "",
    ),
    (
        "a page's privacy label is without its layout's",
        CHECK,
        "    let shown = layout.map_or_else(Label::public, |(l, _)| reads.observed(l));\n",
        "    let shown = layout.map_or_else(Label::public, |_| Label::public());\n",
    ),
    (
        "a page does not read in the graph what its layout reads",
        GRAPH,
        "                    for (name, key) in queried(hirs[layout.unit].body(body_id)) {\n",
        "                    for (name, key) in queried(hirs[layout.unit].body(body_id)).into_iter().take(0) {\n",
    ),
    (
        "a layout's views are provided nothing they need, unchecked",
        SIGNALS,
        "    if !matches!(decl.kind, DeclKind::Page | DeclKind::Layout) {\n",
        "    if decl.kind != DeclKind::Page {\n",
    ),
    (
        "any declaration may name a layout",
        POLICY,
        '        "layout" => (&[K::Page], "a page"),\n',
        '        "layout" => (EVERY, "a page"),\n',
    ),
    (
        "`layout` is a clause whatever follows it",
        GRAMMAR,
        "                || (self.nth_is(1, Kind::Ident) && !self.nth_starts_line(1)))\n",
        "                || true)\n",
    ),
    (
        "a query a page and its layout both bind is read twice",
        SERVER,
        "            let value = match read.iter().find(|(r, a, _)| *r == resource && *a == args) {\n",
        "            let value = match read.iter().find(|_| false) {\n",
    ),
]

# Which suite each mutant must fail: the host's for what the host does.
HOST = {"a query a page and its layout both bind is read twice"}

COMMANDS = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "layouts"],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_store_page_is_shown_in_its_layout", "a_shared_query_is_run_once",
    ],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, **kw):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True, **kw)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def run_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(COMMANDS[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in COMMANDS:
        built, passed, failed = run_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
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
        suite = "host" if what in HOST else "core"
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = run_tests(suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
