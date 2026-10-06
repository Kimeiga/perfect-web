#!/usr/bin/env python3
"""Mutation controls for ADR-0172: the cart lists its lines, and a
speculation reaches every part that reads it.

Each mutant undoes one piece:
- the compiler: no region collected; a region's read, or its handler's
  capture, of what the browser does not hold not refused; the page's
  template lowered without what its handlers capture; a speculated read in
  a block no region renders not refused; a loop over a field typed as its
  binding; a record, a list, a record holding what cannot be sent, or a
  field whose name would not come back, sent or not sent wrongly;
- the program: a new line named by its description; a line's total and the
  cart's subtotal miscounted; one fewer keeping a line at one; one more
  adding two; a line's removal removing nothing; a row's member reads not
  compiled;
- the renderer: a computed value captured as a field; a handler's captures
  not set where they are, or set once per handler; an instance's changes
  always a render;
- the host: a field read by its WIT name; a missing field passed as nothing;
  a list not read;
- the development server: one fewer taking two; a line at one kept; a
  removal removing nothing; an item no store has available; a line's price
  not recorded; a command, or a drain, not waiting for its session's change
  in progress; the interactions a value includes not recorded;
- the browser runtime: lists not shown as held before a batch; focus not
  passed on from a row that goes, or not given back after a batch; a kept
  row rendered again; a speculation the value includes shown again; an
  attribute or a block not rendered again; a new row not inserted; the
  document's regions not served.

A mutant must fail the suite it is listed with, all of it run: `pw-core`'s
tests, `pw-conformance`'s, `pw-render`'s, `pw-render-wasm`'s, `pw-host`'s
with the engine, the development server's, or `e2e/cart.spec.mjs` in
Chromium.

Run from the repository root; `just e14-cart-lines` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
DOMAIN = ROOT / "examples/domain.pw"
CARTS = ROOT / "examples/lib/Carts.pw"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
RENDER_WASM = ROOT / "runtime/pw-render-wasm/src/lib.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # --- the compiler -------------------------------------------------------
    (
        "no region is collected",
        "core",
        SPECULATION,
        # Re-anchored by ADR-0183, whose regions have a kind of their own.
        "        regions.push(Region {\n"
        "            binding: root.to_string(),\n"
        "            part: read.part.0,\n"
        "            kind,\n"
        "        });\n",
        "        let _ = (root, kind);\n",
    ),
    (
        "a region's read of what the browser does not hold is not refused",
        "core",
        SPECULATION,
        # Re-anchored by ADR-0234, whose instance check reads the same way.
        "        for (part, path) in paths {\n"
        "            let root = path.split('.').next().unwrap_or_default();\n"
        "            if !held(root) {\n",
        "        for (part, path) in paths {\n"
        "            let root = path.split('.').next().unwrap_or_default();\n"
        "            if false && !held(root) {\n",
    ),
    (
        "a region's handler capture of what the browser does not hold is not refused",
        "core",
        SPECULATION,
        "                    if !held(&root) {\n",
        "                    if false && !held(&root) {\n",
    ),
    (
        "the page's template is lowered without what its handlers capture",
        "core",
        SPECULATION,
        "        &crate::resume::capture_map(cx.hirs, cx.sigs),\n",
        "        &crate::template_ir::Handlers::new(),\n",
    ),
    (
        "a speculated read in a block no region renders is not refused",
        "core",
        SPECULATION,
        "        if read.nested && speculates(root) && !in_a_region(read.part.0) {\n",
        "        if false && read.nested && speculates(root) && !in_a_region(read.part.0) {\n",
    ),
    (
        "a loop over a field is typed as its binding",
        "core",
        INFER,
        "        for field in fields {\n",
        "        for field in fields.iter().take(0) {\n",
    ),
    (
        "a record is not sent",
        "core",
        LOWER,
        "            let Some(fields) = d.record.as_ref() else {\n",
        "            let Some(fields) = d.record.as_ref().filter(|_| false) else {\n",
    ),
    (
        "a list is not sent",
        "core",
        LOWER,
        "        Type::List(t) => sendable_within(cx, t, on_the_way),\n",
        "        Type::List(_) => false,\n",
    ),
    (
        "a record holding what cannot be sent is sent",
        "core",
        LOWER,
        "                        Lowering::Lowered(field) if sendable_within(cx, &field, on_the_way)\n",
        "                        Lowering::Lowered(_)\n",
    ),
    (
        "a field whose name would not come back is sent",
        "core",
        LOWER,
        "                crate::wit::ident(name).replace('-', \"_\") == *name\n"
        "                    && matches!(\n",
        "                matches!(\n",
    ),
    # --- the program, its transitions and what a row reads ------------------
    (
        "a new line is named by its item's description",
        "conformance",
        CARTS,
        "[CartLine { item_id: item.id, name: item.name, quantity: quantity, unit_price: item.price }]",
        "[CartLine { item_id: item.id, name: item.description, quantity: quantity, unit_price: item.price }]",
    ),
    (
        "a line's total is its price once",
        "conformance",
        DOMAIN,
        "Money { minor_units: line.unit_price.minor_units * line.quantity.value }",
        "Money { minor_units: line.unit_price.minor_units }",
    ),
    (
        "the cart's subtotal is its last line's",
        "conformance",
        DOMAIN,
        "fn(n, line) n + total(line).minor_units",
        "fn(n, line) total(line).minor_units",
    ),
    (
        "one fewer keeps a line at one",
        "conformance",
        CARTS,
        "fn(line) !(same_item(line.item_id, item) & line.quantity.count == 1))",
        "fn(line) true)",
    ),
    (
        "one more adds two",
        "conformance",
        CARTS,
        "fn(line) grown(line, item, PositiveInt(1))",
        "fn(line) grown(line, item, PositiveInt(2))",
    ),
    (
        "a line's removal removes nothing",
        "conformance",
        CARTS,
        "fn(line) !same_item(line.item_id, item))",
        "fn(line) true)",
    ),
    (
        "a row's member reads are not compiled",
        "conformance",
        SPECULATION,
        # Re-anchored by ADR-0228, whose computed row value is pushed deeper.
        "            functions.push(f);\n            computed.push((rest.to_string(), functions.len() - 1));\n",
        "            functions.push(f);\n            let _ = rest;\n",
    ),
    # --- the renderer --------------------------------------------------------
    (
        "a computed value is captured as a field",
        "render",
        RENDER,
        "                .filter(|(k, _)| !k.contains('.'))\n",
        "                .filter(|_| true)\n",
    ),
    (
        "a handler's captures are not set where they are",
        "render",
        RENDER,
        # Re-anchored by ADR-0178, which looks into a block that decides.
        "                let y = captures_value(run, after)?;\n"
        "                if x != y {\n",
        "                let y = captures_value(run, after)?;\n"
        "                if false && x != y {\n",
    ),
    (
        "a handler's captures are set once per handler",
        "render",
        RENDER,
        "                if run.first().and_then(|p| p.id()) != Some(*id) {\n"
        "                    continue;\n"
        "                }\n",
        "",
    ),
    (
        "an instance's changes are always a render",
        "render-wasm",
        RENDER_WASM,
        "        Ok(None) => (0, \"null\".to_string()),\n",
        "        Ok(None) | Ok(Some(_)) => (0, \"null\".to_string()),\n",
    ),
    # --- the host ------------------------------------------------------------
    (
        "a field is read by its WIT name",
        "host",
        HOST,
        "                    let written = field.name.replace('-', \"_\");\n",
        "                    let written = field.name.to_string();\n",
    ),
    (
        "a missing field is passed as nothing",
        "host",
        HOST,
        "                        .ok_or_else(|| format!(\"{at}: the record has no field `{written}`\"))?;\n",
        "                        .unwrap_or(&serde_json::Value::Null);\n",
    ),
    (
        "a list is not read",
        "host",
        HOST,
        "            Type::List(list) => {\n"
        "                let items = v.as_array()",
        "            Type::List(list) if false => {\n"
        "                let items = v.as_array()",
    ),
    # --- the development server ---------------------------------------------
    (
        "one fewer takes two",
        "server",
        STORE_DATA,
        "                        lines[at].quantity -= 1;\n",
        "                        lines[at].quantity -= 2;\n",
    ),
    (
        "a line at one is kept",
        "server",
        STORE_DATA,
        "                        lines.remove(at);\n",
        "                        lines[at].quantity = 0;\n",
    ),
    (
        "a removal removes nothing",
        "server",
        STORE_DATA,
        "                lines.retain(|l| l.item != *item);\n",
        "                lines.retain(|_| true);\n",
    ),
    (
        "an item no store has is available",
        "server",
        STORE_DATA,
        "                    catalog.contains_key(item) && !sold_out.contains(item),\n",
        "                    !sold_out.contains(item),\n",
    ),
    (
        "a line's price is not recorded",
        "server",
        STORE_DATA,
        "                            .get(item)\n"
        "                            .cloned()\n",
        "                            .get(item)\n"
        "                            .cloned()\n"
        "                            .map(|(name, _)| (name, 0))\n",
    ),
    (
        "a command does not wait for its session's change in progress",
        "server",
        SERVER,
        "        // The session's one change at a time, through its frames (ADR-0172).\n"
        "        let session_lock = self.one_at_a_time(session);\n"
        "        let _one = session_lock\n"
        "            .lock()\n"
        "            .expect(\"one change of a session at a time\");\n",
        "",
    ),
    (
        "a drain does not wait for its session's change in progress",
        "server",
        SERVER,
        "        let session_lock = self.one_at_a_time(session);\n"
        "        let _one = session_lock\n"
        "            .lock()\n"
        "            .expect(\"one change of a session at a time\");\n"
        "        self.drain_held(session);\n",
        "        self.drain_held(session);\n",
    ),
    (
        "the interactions a value includes are not recorded",
        "server",
        SERVER,
        "                mine.push_back(interaction.to_string());\n",
        "                let _ = (&mine, interaction);\n",
    ),
    # --- the browser runtime -------------------------------------------------
    (
        "lists are not shown as held before a batch",
        "browser",
        RUNTIME,
        "  revertLists();\n  for (const frame of batch.frames ?? []) applyFrame(frame);\n",
        "  for (const frame of batch.frames ?? []) applyFrame(frame);\n",
    ),
    (
        "focus is not passed on from a row that goes",
        "browser",
        RUNTIME,
        "    if (passing) passFocus(nodes, region.part);\n",
        "    if (false) passFocus(nodes, region.part);\n",
    ),
    (
        "focus is not given back after a batch",
        "browser",
        RUNTIME,
        "  reapplySpeculations().then(() => refocus(focused));\n",
        "  reapplySpeculations();\n",
    ),
    (
        "a kept row is rendered again",
        "browser",
        RUNTIME,
        "  if (changes === null) {\n",
        "  if (true) {\n",
    ),
    (
        "a speculation the value includes is shown again",
        "browser",
        RUNTIME,
        "      (p) => !applied.has(p.interaction) && (p.until === undefined || p.until > frame.version),\n",
        "      (p) => p.until === undefined || p.until > frame.version,\n",
    ),
    (
        "an attribute is not rendered again",
        "browser",
        RUNTIME,
        "    } else if (region.kind === \"attribute\") {\n",
        "    } else if (false) {\n",
    ),
    (
        "a block is not rendered again",
        "browser",
        RUNTIME,
        "    } else if (region.kind === \"block\") {\n",
        "    } else if (false) {\n",
    ),
    (
        "a new row is not inserted",
        "browser",
        RUNTIME,
        "      after.after(...nodes);\n",
        "",
    ),
    (
        "the document's regions are not served",
        "browser",
        SERVER,
        "            manifest[\"regions\"] = serde_json::Value::Object(regions);\n",
        "            let _ = regions;\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "conformance": ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render"],
    "render-wasm": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render-wasm"],
    "host": ["cargo", "test", "--quiet", "--locked", "-p", "pw-host", "--features", "engine"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
}
BROWSER = ["e2e/cart.spec.mjs", "--project=chromium"]

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


def browser_tests(_suite="browser"):
    """(built, passed, failed) for the browser tests, after a build: the page,
    and the server the suite runs, which `run.sh` does not build."""
    built = subprocess.run(
        ["bash", "spikes/own-renderer/run.sh"],
        cwd=ROOT,
        env={**os.environ, "BUILD_ONLY": "1"},
        capture_output=True,
        text=True,
    )
    if built.returncode != 0:
        return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *BROWSER, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {
    "core": cargo_tests,
    "conformance": cargo_tests,
    "render": cargo_tests,
    "render-wasm": cargo_tests,
    "host": cargo_tests,
    "server": cargo_tests,
    "browser": browser_tests,
}


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            return 1

    survivors = 0
    for what, suite, path, anchor, replacement in MUTANTS:
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
        print(f"{what} [{suite}]: {verdict}", flush=True)
    # The page and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
