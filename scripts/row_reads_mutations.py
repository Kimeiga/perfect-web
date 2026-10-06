#!/usr/bin/env python3
"""Mutation controls for ADR-0169: what a template reads through a member
function, a host computes or the build refuses.

Each mutant undoes one piece:
- the value typer: a property read never a member's; a loop's list not read
  for one;
- the lowering: an attribute's value, a value written in an attribute, what
  an `{#if}`, an `{:else if}` or a `{#match}` decides by, or a loop's list,
  not recorded as a read;
- the plan: a row's member read in text not planned; one no host computes
  passing silently, in text or elsewhere; reads outside text not checked;
  a row read planned twice; a signal's member not refused;
- the development server: a row given its item's fields alone; a menu
  change rendered from the rows before it; a line priced 450 whatever its
  item;
- the renderer: a path read one field deep;
- `display`: an amount owed back negated whole, or its cents its
  remainder; thousands not grouped, or a group's zeros dropped.

A typer, lowering or plan mutant must fail `pw-core`'s tests, all of them
run; a server mutant, the server's; a renderer mutant, `pw-render`'s; a
`display` mutant, the compiled component's own tests, after it is compiled
again from the mutated source.

Name a part of a mutant's description to run only the mutants that have it.

Run from the repository root; `just e14-row-reads` records the output. The
source is restored after every mutant, whatever happens.
"""

import glob
import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
TEMPLATE_IR = ROOT / "compiler/pw-core/src/template_ir.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
DOMAIN = ROOT / "examples/domain.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a property read is never a member's",
        "core",
        VALUES,
        "            .is_some_and(|sig| self.sigs.by_def(sig.definition).is_some())\n"
        "    }\n\n"
        "    /// [`MemberReads`] in this body (ADR-0169).\n",
        "            .is_some_and(|sig| self.sigs.by_def(sig.definition).is_some() && false)\n"
        "    }\n\n"
        "    /// [`MemberReads`] in this body (ADR-0169).\n",
    ),
    (
        "a loop's list is not read for a member",
        "core",
        VALUES,
        "            calls |= self.calls_a_member(&t, segment);\n",
        "",
    ),
    (
        "an attribute's value is not a read",
        "core",
        TEMPLATE_IR,
        # Re-anchored by ADR-0221, whose textarea writes one more, deeper,
        # and by ADR-0226, whose computed value is named before it is read.
        "                };\n"
        "                ix.read(id, &value, ReadKind::Attribute, ReadAt::Expr(*e), ctx);\n",
        "                };\n",
    ),
    (
        "a value written in an attribute is not a read",
        "core",
        TEMPLATE_IR,
        "        ix.read(id, &path, ReadKind::Attribute, ReadAt::Expr(e), ctx);\n",
        "        let _ = (path, e);\n",
    ),
    (
        "what an `{#if}` decides by is not a read",
        "core",
        TEMPLATE_IR,
        "        ix.read(id, &value, ReadKind::Subject, ReadAt::Expr(e), ctx);\n"
        "        out.push(conditional(",
        "        out.push(conditional(",
    ),
    (
        "what an `{:else if}` decides by is not a read",
        "core",
        TEMPLATE_IR,
        "                        ix.read(nested, &v, ReadKind::Subject, ReadAt::Expr(c), ctx);\n",
        "",
    ),
    (
        "what a `{#match}` decides by is not a read",
        "core",
        TEMPLATE_IR,
        "        ix.read(id, &value, ReadKind::Subject, ReadAt::Expr(e), ctx);\n"
        "        let stray =",
        "        let _ = e;\n"
        "        let stray =",
    ),
    (
        "a loop's list is not a read",
        "core",
        TEMPLATE_IR,
        "        ix.read(id, &collection, ReadKind::List, ReadAt::List(node), ctx);\n",
        "        let _ = node;\n",
    ),
    (
        "a row's member read in text is not planned",
        "core",
        PLAN,
        "            ) {\n                let read = row_read(\n",
        "            ) && false {\n                let read = row_read(\n",
    ),
    (
        "a member read in text no host computes passes silently",
        "core",
        PLAN,
        "                let read = row_read(\n"
        "                    hirs,\n"
        "                    sigs,\n"
        "                    &template.chunks,\n"
        "                    &found,\n"
        "                    hole.part.0,\n"
        "                    &hole.path,\n"
        "                    &mut members,\n"
        "                )?\n"
        "                .ok_or_else(|| unplanned(hole.part.0, &hole.path, \"a text part\"))?;\n",
        "                let Some(read) = row_read(\n"
        "                    hirs,\n"
        "                    sigs,\n"
        "                    &template.chunks,\n"
        "                    &found,\n"
        "                    hole.part.0,\n"
        "                    &hole.path,\n"
        "                    &mut members,\n"
        "                )?\n"
        "                else {\n"
        "                    continue;\n"
        "                };\n",
    ),
    (
        "a read outside a text part is not checked",
        "core",
        PLAN,
        "            || !calls_a_member(hirs, ws, sigs, &mut typed, read.origin, read.at)\n",
        "            || true\n",
    ),
    (
        "a member read outside text no host computes passes silently",
        "core",
        PLAN,
        "        let row = row_read(\n"
        "            hirs,\n"
        "            sigs,\n"
        "            &template.chunks,\n"
        "            &found,\n"
        "            read.part.0,\n"
        "            &read.path,\n"
        "            &mut members,\n"
        "        )?\n"
        "        .ok_or_else(|| unplanned(read.part.0, &read.path, what))?;\n",
        "        let _ = what;\n"
        "        let Some(row) = row_read(\n"
        "            hirs,\n"
        "            sigs,\n"
        "            &template.chunks,\n"
        "            &found,\n"
        "            read.part.0,\n"
        "            &read.path,\n"
        "            &mut members,\n"
        "        )?\n"
        "        else {\n"
        "            continue;\n"
        "        };\n",
    ),
    (
        "a row read in text is planned twice",
        "core",
        PLAN,
        "                if !rows\n"
        "                    .iter()\n"
        "                    .any(|r| r.collection == read.collection && r.path == read.path)\n"
        "                {\n",
        "                if true {\n",
    ),
    (
        "a row read outside text is planned twice",
        "core",
        PLAN,
        "        if !rows\n"
        "            .iter()\n"
        "            .any(|r| r.collection == row.collection && r.path == row.path)\n"
        "        {\n",
        "        if true {\n",
    ),
    (
        "a signal's member is not refused",
        "core",
        PLAN,
        "            ) {\n"
        "                return Err(unplanned(hole.part.0, &hole.path, \"a text part\"));\n"
        "            }\n",
        "            ) {\n"
        "            }\n",
    ),
    (
        "a row is given its item's fields alone",
        "server",
        SERVER,
        # Re-anchored by ADR-0181, whose rows may be inside another list's.
        "                fields.insert(within.to_string(), val_to_value(&read_value));\n",
        "                let _ = (within, read_value);\n",
    ),
    (
        "a menu change is rendered from the rows before it",
        "server",
        SERVER,
        # Re-anchored by ADR-0181.
        "        let items = self.menu_rows(STORE_ID)?;\n"
        "        let (template, _) = self.menu_part();\n",
        "        let items = before.clone();\n"
        "        let (template, _) = self.menu_part();\n",
    ),
    (
        "a line is priced 450 whatever its item",
        "server",
        SERVER,
        "Val::Record(vec![(\"minor-units\".into(), Val::S64(line.price))]),\n",
        "Val::Record(vec![(\"minor-units\".into(), Val::S64(450))]),\n",
    ),
    (
        "a path is read one field deep",
        "render",
        RENDER,
        "                rest = after;\n",
        "                rest = &[];\n",
    ),
    (
        "an amount owed back is negated whole",
        "display",
        DOMAIN,
        "    let dollars = if n >= 0 { q } else if r == 0 { 0 - q } else { 0 - q - 1 }\n",
        "    let dollars = if n >= 0 { q } else { 0 - q }\n",
    ),
    (
        "an amount owed back keeps its remainder as its cents",
        "display",
        DOMAIN,
        "    let cents = if n >= 0 | r == 0 { r } else { 100 - r }\n",
        "    let cents = r\n",
    ),
    (
        "thousands are not grouped",
        "display",
        DOMAIN,
        "    \"{sign}${grouped(dollars)}.{pad}{cents}\"\n",
        "    \"{sign}${dollars}.{pad}{cents}\"\n",
    ),
    (
        "a group's zeros are dropped",
        "display",
        DOMAIN,
        "        let pad = if rest < 10 { \"00\" } else if rest < 100 { \"0\" } else { \"\" }\n",
        "        let pad = \"\"\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render"],
    "display": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine",
        "--test", "display",
    ],
}

# The program `domain.display` is compiled from, as `just e10-component`
# compiles it.
PROGRAM = [
    "packages/pw-std/*.pw",
    "packages/pw-platform-web/*.pw",
    "examples/domain.pw",
    "examples/lib/*.pw",
    "examples/store/*.pw",
]

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


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def display_tests(suite="display"):
    """(built, passed, failed) for the compiled `display`, compiled again
    from the source as it is now: a mutant of the program is a mutant of
    the component."""
    sources = [f for pattern in PROGRAM for f in sorted(glob.glob(str(ROOT / pattern)))]
    emitted = subprocess.run(
        ["cargo", "run", "--quiet", "--locked", "-p", "pw-cli", "--", "emit-component",
         "--component", "domain.display",
         "--out", str(ROOT / "docs/evidence/E10/domain.display.wasm"), *sources],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if emitted.returncode != 0:
        return False, 0, 0
    return cargo_tests(suite)


SUITES = {
    "core": cargo_tests,
    "server": cargo_tests,
    "render": cargo_tests,
    "display": display_tests,
}


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    chosen = [m for m in MUTANTS if not sys.argv[1:] or any(a in m[0] for a in sys.argv[1:])]
    for suite in sorted({m[1] for m in chosen}):
        built, passed, failed = SUITES[suite](suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in chosen:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: ANCHOR NOT FOUND EXACTLY ONCE in {path.name}")
            survivors += 1
            continue
        try:
            path.write_text(original.replace(anchor, replacement, 1))
            built, passed, failed = SUITES[suite](suite)
        finally:
            path.write_text(original)
        if not built:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}")
    # The component is compiled again from the restored source.
    if any(m[1] == "display" for m in chosen):
        display_tests()

    print(f"{len(chosen) - survivors} of {len(chosen)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
