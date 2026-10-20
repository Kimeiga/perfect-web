#!/usr/bin/env python3
"""Mutation controls for track `device-store` (ADR-XXXX): a value the
reader's device keeps, and what the browser computes for one.

Each mutant undoes one piece:
- the checker (`compiler/pw-core/src/device_store.rs`, `signatures.rs`,
  `signals.rs`): the `Device` label at a read; each sink (a command's
  argument, a call a device value decides, a query's key, a navigation, an
  address, a form's control); the device keeping its own; the clauses (a
  name, a well-formed name, one name a program, `privacy Device`, `conflict
  last_write_wins`); the versions (a step for each, pure, chained, to the
  declared type); the form (a function, a `where`); no `provide`; the device
  as the signal's provider;
- the plan and the build (`template_ir.rs`, `page_values.rs`, `resume.rs`,
  `backend/computed.rs`): the page's instance of it; the marks; the plan's
  entry; a capture of it landing on the device; several inputs; a computed
  list; a block's reads of what a computed value inside it reads; each input
  given to the module's function;
- the host (`server/src/device.rs`, `main.rs`): the root's mark, the head's
  style, the policy allowing it, the manifest's entries, the module served
  for the steps, the first value computed from each input;
- the browser (`public/pw-runtime.mjs`): the read at start, the write on a
  change, another tab heard, nothing written back, the decoder, the
  migration and its old key, a newer version stopping the writes, the
  unreadable kept aside, a refused write said, the mark kept until what the
  device decides is rendered; and for TodoMVC, the morph, rows by key, a
  box's state following the rendering, each input given, the reads a
  computed part is computed again for.

A checker or plan mutant must fail `compiler/pw-core/tests/device_store.rs`;
a host mutant, the server's `tests::todomvc::` and `tests::device_store::`; a browser mutant,
`e2e/device-store.spec.mjs` or `e2e/todomvc.spec.mjs` in Chromium, after the
pages are built again (`run.sh`, `todomvc.sh`).

Run from the repository root; `just e14-device-store` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECKER = ROOT / "compiler/pw-core/src/device_store.rs"
SIGNATURES = ROOT / "compiler/pw-core/src/signatures.rs"
SIGNALS = ROOT / "compiler/pw-core/src/signals.rs"
INFER = ROOT / "compiler/pw-core/src/infer.rs"
TEMPLATE = ROOT / "compiler/pw-core/src/template_ir.rs"
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
RESUME = ROOT / "compiler/pw-core/src/resume.rs"
COMPUTED = ROOT / "compiler/pw-core/src/backend/computed.rs"
DEVICE = ROOT / "spikes/own-renderer/server/src/device.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    # --- the checker ------------------------------------------------------
    (
        "a read of a value the device keeps carries no label",
        "core",
        SIGNATURES,
        "                Some(Label::device())\n",
        "                None\n",
    ),
    (
        "a command's argument is not asked",
        "core",
        CHECKER,
        "                    for a in args {\n                        if self.device(a.value) {\n",
        "                    for a in args {\n                        if false && self.device(a.value) {\n",
    ),
    (
        "a command call a device value decides is let through",
        "core",
        CHECKER,
        "                        && label.restrictions().any(|r| *r == Restriction::Device)\n",
        "                        && label.restrictions().any(|r| *r == Restriction::Device && false)\n",
    ),
    (
        "a query's key is not asked",
        "core",
        CHECKER,
        '                Expr::Keyword { keyword, args, .. } if keyword == "query" => {\n                    for &a in args {\n                        if self.device(a) {\n',
        '                Expr::Keyword { keyword, args, .. } if keyword == "query" => {\n                    for &a in args {\n                        if false && self.device(a) {\n',
    ),
    (
        "an address is not asked",
        "core",
        CHECKER,
        "                        crate::template_ir::Context::Url => Some(format!(\n",
        "                        crate::template_ir::Context::Url if false => Some(format!(\n",
    ),
    (
        "a form's control is not asked",
        "core",
        CHECKER,
        '                let in_form = in_form || tag == "form";\n',
        "                let in_form = in_form && false;\n",
    ),
    (
        "a session's value may be kept on the device",
        "core",
        CHECKER,
        "                .filter(|r| **r != Restriction::Device)\n                .map(|r| r.to_string())\n",
        "                .filter(|r| **r != Restriction::Device && false)\n                .map(|r| r.to_string())\n",
    ),
    (
        "a provide of one is not refused",
        "core",
        CHECKER,
        "            if kept_named(self.sigs, self.kept, self.unit, n).is_none() {\n                continue;\n            }\n            out.push(diagnostic(\n                codes::DEVICE_SIGNAL_PROVIDED,\n",
        "            if kept_named(self.sigs, self.kept, self.unit, n).is_some() {\n                continue;\n            }\n            out.push(diagnostic(\n                codes::DEVICE_SIGNAL_PROVIDED,\n",
    ),
    (
        "a value a page's signal holds is said to be kept itself",
        "core",
        CHECKER,
        '            Some((_, n)) => (format!("`{n}`"), false),\n',
        '            Some((_, n)) => (format!("`{n}`"), true),\n',
    ),
    (
        "a refusal names no place where its value is the device's",
        "core",
        CHECKER,
        "        related: vec![boundary],\n",
        "        related: {\n            let _ = boundary;\n            Vec::new()\n        },\n",
    ),
    (
        "a refusal does not name the declaration of what the device keeps",
        "core",
        CHECKER,
        "            if let Some(def) = kept_named(self.sigs, self.kept, self.unit, n)\n                && let Some(decl) = crate::resolve::declaration(self.hirs, def)\n",
        "            if false\n                && let Some(def) = kept_named(self.sigs, self.kept, self.unit, n)\n                && let Some(decl) = crate::resolve::declaration(self.hirs, def)\n",
    ),
    (
        "a value kept under no name is accepted",
        "core",
        CHECKER,
        '            Some(("device", None)) => out.push(clauses(\n',
        '            Some(("device", None)) if false => out.push(clauses(\n',
    ),
    (
        "a name of any characters is accepted",
        "core",
        CHECKER,
        "    !name.is_empty()\n        && name.len() <= 128\n",
        "    true || !name.is_empty()\n        && name.len() <= 128\n",
    ),
    (
        "two values may be kept under one name",
        "core",
        CHECKER,
        "                    .find(|(d, k)| k.name == written && **d < me && **d != me)\n",
        "                    .find(|(d, k)| k.name == written && **d < me && false)\n",
    ),
    (
        "whose it is is not asked",
        "core",
        CHECKER,
        '        Some((_, "Device")) => {}\n',
        "        Some(_) => {}\n",
    ),
    (
        "what two tabs do is not asked",
        "core",
        CHECKER,
        '        Some((_, "last_write_wins")) => {}\n',
        "        Some(_) => {}\n",
    ),
    (
        "a version needs no step",
        "core",
        CHECKER,
        "    if written.len() + 1 != version as usize {\n",
        "    if written.len() + 1 != version as usize && written.len() > version as usize {\n",
    ),
    (
        "a step that performs something is accepted",
        "core",
        CHECKER,
        "            .is_some_and(|row| row.is_empty());\n",
        "            .is_none_or(|_| true);\n",
    ),
    (
        "steps need not chain",
        "core",
        CHECKER,
        "        if !made.same_as(takes) {\n",
        "        if false && !made.same_as(takes) {\n",
    ),
    (
        "the last step may make another type",
        "core",
        CHECKER,
        "        && !made.same_as(ty)\n",
        "        && !made.same_as(ty)\n        && false\n",
    ),
    (
        "a map is kept",
        "core",
        CHECKER,
        "            Builtin::Map | Builtin::Set => Some(format!(\n",
        "            Builtin::Map | Builtin::Set if false => Some(format!(\n",
    ),
    (
        "an opaque type with a where is kept",
        "core",
        CHECKER,
        "    if declared.invariant.is_some() {\n",
        "    if false && declared.invariant.is_some() {\n",
    ),
    (
        "a page must provide what the device keeps",
        "core",
        SIGNALS,
        "        if provision.device.contains(&signal) {\n            continue;\n        }\n",
        "        if provision.device.contains(&signal) && false {\n            continue;\n        }\n",
    ),
    (
        "a signal with a value that says nowhere to keep it is also not provided",
        "core",
        SIGNALS,
        "                if decl.kind == DeclKind::Signal && decl.body.is_some() {\n",
        "                if decl.kind == DeclKind::Signal && decl.body.is_some() && false {\n",
    ),
    (
        "a row of a list the device keeps has no type",
        "core",
        INFER,
        "                        .is_none_or(of_a_module_signal) =>\n",
        "                        .is_none() =>\n",
    ),
    # --- the plan and the build ---------------------------------------------
    (
        "a page holds no instance of it",
        "core",
        TEMPLATE,
        "    if decl.kind == crate::hir::DeclKind::Page {\n        for (name, signal) in crate::device_store::held_by(hirs, ws, sigs, unit, id) {\n",
        "    if false && decl.kind == crate::hir::DeclKind::Page {\n        for (name, signal) in crate::device_store::held_by(hirs, ws, sigs, unit, id) {\n",
    ),
    (
        "no element it decides is marked",
        "core",
        TEMPLATE,
        "    if decided_by_the_device(body, attrs, children, ctx) {\n",
        "    if false && decided_by_the_device(body, attrs, children, ctx) {\n",
    ),
    (
        "the plan carries no entry for it",
        "core",
        PLAN,
        "            device: crate::device_store::held(hirs, ws, sigs, unit, id)?,\n",
        "            device: Vec::new(),\n",
    ),
    (
        "a capture of it may not land in its document",
        "host",
        RESUME,
        "                        label: written_by_its_device(labels.label(body, expr)),\n",
        "                        label: labels.label(body, expr),\n",
    ),
    (
        "a value from several of the browser's is refused",
        "core",
        PLAN,
        '    if browsers && (inputs.len() > 1 || nested) && !path.contains(".#") {\n',
        '    if browsers && false && !path.contains(".#") {\n',
    ),
    (
        "a computed list is refused",
        "core",
        TEMPLATE,
        "            && (browsers.is_empty() || !browsers.iter().all(|(name, _)| ctx.signals.contains(name)))\n",
        "            && (browsers.is_empty() || true)\n",
    ),
    (
        "a block is not rendered again for what a value computed in it reads",
        "core",
        PLAN,
        "    let root = |p: &str| match computed.get(p) {\n",
        "    let root = |p: &str| match computed.get(p).filter(|_| false) {\n",
    ),
    (
        "a value computed from the signals inside a host's block is built",
        "core",
        PLAN,
        "    let signal = |path: &str| signals.contains(&root(path)) || computed.contains_key(path);\n",
        "    let signal = |path: &str| signals.contains(&root(path));\n",
    ),
    (
        "a refusal names one of the signals a value is computed from",
        "core",
        PLAN,
        "                        Some([init @ .., last]) if !init.is_empty() => format!(\n",
        "                        Some([init @ .., last]) if init.is_empty() => format!(\n",
    ),
    (
        "the module gives a part its first value alone",
        "core",
        COMPUTED,
        "        for (i, (_, input)) in f.params.iter().enumerate() {\n",
        "        for (i, (_, input)) in f.params.iter().enumerate().take(1) {\n",
    ),
    # --- what decision 7 lifts, for the browser's values alone (the
    # integrator's ruling 3, 2026-10-20): each refusal it lifts still fires
    # where a host's value is read, held by
    # `a_value_a_host_holds_is_still_refused_where_decision_7_lifts_the_refusals`.
    (
        "a value from a browser value and a host's together is built as the browser's",
        "core",
        PLAN,
        "            .all(|(_, read)| signals.iter().any(|s| *s == root_of(read)));\n",
        "            .any(|(_, read)| signals.iter().any(|s| *s == root_of(read)));\n",
    ),
    (
        "a value a host computes inside a block a signal decides is let through",
        "core",
        PLAN,
        "                    .any(|v| v.starts_with('#') && !computed.contains_key(v))\n",
        "                    .any(|v| v.starts_with('#') && false)\n",
    ),
    (
        "a list computed from a host's value is built as the browser's",
        "core",
        TEMPLATE,
        "            && (browsers.is_empty() || !browsers.iter().all(|(name, _)| ctx.signals.contains(name)))\n",
        "            && browsers.is_empty()\n",
    ),
    # --- the host -------------------------------------------------------------
    (
        "the root does not say it is reading the device",
        "host",
        DEVICE,
        '        true => " lang=\\"en\\" data-pw-device=\\"reading\\"",\n',
        '        true => " lang=\\"en\\"",\n',
    ),
    (
        "the head holds no style hiding what the device decides",
        "host",
        DEVICE,
        '            format!("<style>{READING}</style>\\n<noscript><style>{UNSCRIPTED}</style></noscript>\\n")\n',
        '            format!("<noscript><style>{UNSCRIPTED}</style></noscript>\\n")\n',
    ),
    (
        "the policy does not allow the style",
        "host",
        SERVER,
        "            needs.styles.insert(device::READING.to_string());\n",
        "            let _ = device::READING;\n",
    ),
    (
        "the policy does not allow the style shown with scripts off",
        "host",
        SERVER,
        "            needs.styles.insert(device::UNSCRIPTED.to_string());\n",
        "            let _ = device::UNSCRIPTED;\n",
    ),
    (
        "the manifest carries nothing of the device",
        "host",
        DEVICE,
        '        manifest["device"] = plan["device"].clone();\n',
        '        let _ = &manifest;\n',
    ),
    (
        "the host computes a first value from its first input alone",
        "host",
        SERVER,
        "                Some(several) if !several.is_empty() => {\n",
        "                Some(several) if false && !several.is_empty() => {\n",
    ),
    (
        "the steps between versions are not served",
        "host",
        SERVER,
        "        || device::migrates(plan);\n",
        "        || (device::migrates(plan) && false);\n",
    ),
    # --- the browser: the demo --------------------------------------------------
    (
        "nothing is read at start",
        "browser",
        RUNTIME,
        "    for (const held of deviceHeld.values()) {\n      const got = await readKept(store, held);\n",
        "    for (const held of [].values()) {\n      const got = await readKept(store, held);\n",
    ),
    (
        "a change is not written",
        "browser",
        RUNTIME,
        "    store.setItem(key, wire(value));\n    deviceState[held.signal] = \"read\";\n",
        "    deviceState[held.signal] = \"read\";\n",
    ),
    (
        "another tab's write is not heard",
        "browser",
        RUNTIME,
        '  addEventListener("storage", (e) => void heardKept(e), { signal: life.signal });\n',
        "  void heardKept;\n",
    ),
    (
        "another tab's write is written back",
        "browser",
        RUNTIME,
        '      setSignal(held.signal, "value" in got ? got.value : firstValues.get(held.signal), null, false);\n',
        '      setSignal(held.signal, "value" in got ? got.value : firstValues.get(held.signal), null, true);\n',
    ),
    (
        "what is read is used undecoded",
        "browser",
        RUNTIME,
        '  const why = unlike(json, schema, "the value");\n',
        "  const why = null;\n",
    ),
    (
        "an earlier version is not migrated",
        "browser",
        RUNTIME,
        "  for (let v = held.version - 1; v >= 1; v--) {\n",
        "  for (let v = held.version - 1; v >= held.version; v--) {\n",
    ),
    (
        "the old key stays after a migration",
        "browser",
        RUNTIME,
        "      store.removeItem(deviceKey(held, v));\n",
        "      void deviceKey;\n",
    ),
    (
        "a newer version does not stop the writes",
        "browser",
        RUNTIME,
        '  if (newerKept(store, held)) return { state: "stale" };\n',
        '  if (newerKept(store, held) && false) return { state: "stale" };\n',
    ),
    (
        "an unreadable value is not kept aside",
        "browser",
        RUNTIME,
        "      if (unread !== null) store.setItem(`${key}!unreadable`, unread);\n",
        "      void unread;\n",
    ),
    (
        "a refused write is not said",
        "browser",
        RUNTIME,
        '    if (source?.el) tell(source.el, DEVICE_SAYS.full, "device-full");\n    else say(DEVICE_SAYS.full);\n',
        "    void DEVICE_SAYS;\n",
    ),
    (
        "each value is set as it is read, and the mark waits for the last one's render alone",
        "browser",
        RUNTIME,
        "      read.push([held, got]);\n",
        '      if ("value" in got) setSignal(held.signal, got.value, null, false);\n      read.push([held, got]);\n',
    ),
    (
        "the mark is taken away before what the device decides is rendered",
        "browser",
        RUNTIME,
        "  const store = deviceStore();\n  try {\n",
        '  const store = deviceStore();\n  document.documentElement.dataset.pwDevice = "read";\n  try {\n',
    ),
    # --- the browser: TodoMVC ----------------------------------------------------
    (
        "a block rendered again is replaced, not morphed",
        "todomvc",
        RUNTIME,
        "      inserted.push(...morphBlock(live.part, await renderBlock(live.part)));\n",
        "      inserted.push(...replaceBlock(live.part, await renderBlock(live.part)));\n",
    ),
    (
        "rows are matched by their place, not their key",
        "browser",
        RUNTIME,
        "    const a = rowAnchor(nodes[i]);\n    if (a?.start) {\n",
        "    const a = rowAnchor(nodes[i]);\n    if (false && a?.start) {\n",
    ),
    (
        "a box's state does not follow the rendering",
        "todomvc",
        RUNTIME,
        '    old.checked = fresh.hasAttribute("checked");\n',
        "    void fresh;\n",
    ),
    (
        "a computed part is given its first value alone",
        "todomvc",
        RUNTIME,
        "  return (live.paths?.length ? live.paths : [live.path]).map(signalAt);\n",
        "  return [live.path].map(signalAt);\n",
    ),
    (
        "a computed part is computed again for its first value alone",
        "browser",
        RUNTIME,
        "  return live.reads?.length ? live.reads : [live.signal];\n",
        "  return [live.signal];\n",
    ),
]

CARGO = {
    "core": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "device_store",
    ],
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "tests::todomvc::", "tests::device_store::",
    ],
}
SPECS = {
    "browser": ["e2e/device-store.spec.mjs", "--project=chromium"],
    "todomvc": ["e2e/todomvc.spec.mjs", "--project=chromium"],
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1500


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


def built():
    """Whether the pages, TodoMVC's and the server build: what each browser
    run serves, built again from the source as it is now."""
    for cmd, env in (
        (["bash", "spikes/own-renderer/run.sh"], {"BUILD_ONLY": "1"}),
        (["bash", "spikes/own-renderer/todomvc.sh"], {}),
        (["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"], {}),
    ):
        done = subprocess.run(cmd, cwd=ROOT, env={**os.environ, **env},
                              capture_output=True, text=True)
        if done.returncode != 0:
            return False
    return True


def browser_tests(suite):
    """(built, passed, failed) of the suite's spec in Chromium, after a build."""
    if not built():
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPECS[suite], "--reporter=line"],
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
    "host": cargo_tests,
    "browser": browser_tests,
    "todomvc": browser_tests,
}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        ok, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not ok or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
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
            ok, passed, failed = SUITES[suite](suite)
        finally:
            path.write_text(original)
        if not ok:
            verdict = "KILLED (does not build)"
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what} [{suite}]: {verdict}", flush=True)
    # The pages and the server are built again from the restored source.
    built()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
