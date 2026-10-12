#!/usr/bin/env python3
"""Mutation controls for ADR-0310: a name means its binding, in every
analysis.

Each mutant undoes one piece:
- `Lexical`: `<ready as={x}>`'s own name left unbound where it binds;
- the effect inference: a call through a local resolved by its spelling, or
  no body's scopes kept to ask;
- the handler's requests: a call through a local counted as a command's;
- privacy's reads: a call through a local read as its namesake's, in a
  declaration's label, in the reads a body makes, and in the cause a label
  names;
- the template's lowering: a block that binds a name the template can
  already read keeps it, where the page's names, a composed view's use's,
  or the blocks around it are not known, or in a layout; and a block's
  binding does not hide a signal of its spelling.

Each must fail `compiler/pw-core/tests/names_mean_their_binding.rs`. Run
from the repository root; `just e14-name-bindings` records the output. The source
is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEXICAL = ROOT / "compiler/pw-core/src/lexical.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"

GUARD = (
    "            // A call through a local is the local's, never a declaration\n"
    "            // of its spelling's (ADR-0063).\n"
    "            Expr::Call { callee, .. }\n"
    "                if matches!(body.expr(*callee), Expr::Name(_))\n"
    "                    && inference.local(at, body, *callee) =>\n"
    "            {\n"
    "                continue;\n"
    "            }\n"
)

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a stream part's name is left unbound where it binds",
        LEXICAL,
        "                            self.out.uses.insert(e, Binder::Stream(element));\n",
        "",
    ),
    (
        "a call through a local is resolved by its spelling",
        EFFECTS,
        "            if matches!(body.expr(*callee), Expr::Name(_)) && self.local(unit, body, *callee) {\n",
        "            if matches!(body.expr(*callee), Expr::Name(_)) && false {\n",
    ),
    (
        "no body's scopes are kept to ask",
        EFFECTS,
        "            self.lexicals.insert((*unit, *id), types.lexical().clone());\n",
        "            let _ = (unit, id, types);\n",
    ),
    (
        "a handler's call through a local is a command's request",
        CHECK,
        "                        && lexical.binder(*callee).is_some()\n",
        "                        && lexical.binder(*callee).is_some()\n"
        "                        && false\n",
    ),
    (
        "a declaration's label reads a local's namesake",
        CHECK,
        "        // `query Cart(session)` parses as a keyword statement; `Cart(session)`\n"
        "        // as a call. Both name a declaration.\n"
        "        let called = match body.expr(id) {\n" + GUARD,
        "        // `query Cart(session)` parses as a keyword statement; `Cart(session)`\n"
        "        // as a call. Both name a declaration.\n"
        "        let called = match body.expr(id) {\n",
    ),
    (
        "the reads a body makes read a local's namesake",
        CHECK,
        "        let read = match body.expr(id) {\n" + GUARD,
        "        let read = match body.expr(id) {\n",
    ),
    (
        "the cause a label names is a local's namesake",
        CHECK,
        "    let body = hir.body(decl.body?);\n"
        "    for id in body.walk() {\n"
        "        let called = match body.expr(id) {\n" + GUARD,
        "    let body = hir.body(decl.body?);\n"
        "    for id in body.walk() {\n"
        "        let called = match body.expr(id) {\n",
    ),
    (
        "a page's names are not around its markup",
        TEMPLATE,
        "        around,\n        in_layout: false,\n",
        "        around: { let _ = around; BTreeSet::new() },\n        in_layout: false,\n",
    ),
    (
        "a composed view is not given what its use can read",
        TEMPLATE,
        "        around: ctx.around.clone(),\n",
        "        around: BTreeSet::new(),\n",
    ),
    (
        "a layout's blocks keep their names",
        TEMPLATE,
        "        around: BTreeSet::new(),\n        in_layout: true,\n",
        "        around: BTreeSet::new(),\n        in_layout: false,\n",
    ),
    (
        "a view in a layout keeps its blocks' names",
        TEMPLATE,
        "        in_layout: ctx.in_layout,\n",
        "        in_layout: false,\n",
    ),
    (
        "a block's binding is not around the blocks inside it",
        TEMPLATE,
        "            inner.around.insert(name.clone());\n",
        "",
    ),
    (
        "a block that binds a name the template can read keeps it",
        TEMPLATE,
        "            let hides = self.in_layout\n                || self.around.contains(b)\n",
        "            let hides = self.in_layout\n",
    ),
    (
        "a block's binding does not hide a signal of its spelling",
        TEMPLATE,
        "            inner.signals.remove(b);\n",
        "",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "names_mean_their_binding"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 900


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


def tests():
    """(built, passed, failed) over the tests."""
    out, code = bounded(TESTS, cwd=ROOT)
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
    built, passed, failed = tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
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
            built, passed, failed = tests()
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
