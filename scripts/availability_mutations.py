#!/usr/bin/env python3
"""Mutation controls for ADR-0178: whether an item can be ordered is shown
before the press, and a change to it reaches every page open (charter §15.1,
§15.2).

Each mutant undoes one piece:
- the data layer: every item said to be available;
- the page: a sold-out item's row saying nothing; the `Menu` query not
  listening for a stock change;
- the value analysis: a listener's `_` typed as a name nothing declares, or
  `_` typed as any value wherever it is written;
- the renderer: a block that decides as it did rendered again; one that
  decides otherwise set where it is; a block's handlers not found;
- the server: a stock change announced as the menu's; the pages taken to
  show the menu as it is now; the pages open not told when the menu is read
  again; a document's menu rendered again from its own read, untold
  (ADR-0150's rule);
- the control: `/bench/stock?tell=true` telling nothing.

A value-analysis mutant must fail `pw-core`'s tests, all of them run; a
renderer mutant, `pw-render`'s or the development server's; a data-layer,
page or server mutant, the development server's; the control's,
`e2e/availability.spec.mjs` in Chromium.

Run from the repository root; `just e14-availability` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"
APP = ROOT / "examples/store/app.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "every item is said to be available",
        "server",
        STORE_DATA,
        # Re-anchored by ADR-0181, whose data layer groups the menu.
        '                        ("available".into(), Val::Bool(!sold_out.contains(id))),\n',
        '                        ("available".into(), Val::Bool(true)),\n',
    ),
    (
        "a sold-out item's row says nothing",
        "server",
        APP,
        # Re-anchored by ADR-0181: the row is inside its category's.
        "                                    {:else}\n"
        "                                        <p>Sold out</p>\n",
        "                                    {:else}\n",
    ),
    (
        "the `Menu` query does not listen for a stock change",
        "server",
        APP,
        "    invalidates_on MenuChanged(id), InventoryChanged(id, _)\n"
        "    concurrency    one_per_key\n",
        "    invalidates_on MenuChanged(id)\n"
        "    concurrency    one_per_key\n",
    ),
    (
        "a listener's `_` is a name nothing declares",
        "core",
        VALUES,
        '            None if n == "_" && self.listens_with(id) => Ty::Any,\n',
        '            None if false && n == "_" && self.listens_with(id) => Ty::Any,\n',
    ),
    (
        "`_` is any value wherever it is written",
        "core",
        VALUES,
        '            None if n == "_" && self.listens_with(id) => Ty::Any,\n',
        '            None if n == "_" => Ty::Any,\n',
    ),
    (
        "a block that decides as it did is rendered again",
        "render",
        RENDER,
        "                if x != y {\n"
        "                    return Ok(false);\n"
        "                }\n",
        "                if true {\n"
        "                    return Ok(false);\n"
        "                }\n",
    ),
    (
        "a block that decides otherwise is set where it is",
        "render",
        RENDER,
        "                if x != y {\n"
        "                    return Ok(false);\n"
        "                }\n",
        "                if false {\n"
        "                    return Ok(false);\n"
        "                }\n",
    ),
    (
        "a block's handlers are not found",
        "server",
        RENDER,
        "        for inner in p.nested() {\n"
        "            handlers_within(inner, out);\n"
        "        }\n",
        "",
    ),
    (
        "a stock change is announced as the menu's",
        "server",
        SERVER,
        "            MenuOp::Stock { id } => {\n"
        '                pw_materialize::Event::new("Events.InventoryChanged", &[STORE_ID, id.as_str()])\n'
        "            }\n",
        '            MenuOp::Stock { .. } => pw_materialize::Event::new("Events.MenuChanged", &[STORE_ID]),\n',
    ),
    (
        "the pages are taken to show the menu as it is now",
        "server",
        SERVER,
        # Re-anchored by ADR-0181: the whole change is derived from what the
        # pages show and the menu now, with no step between. Its sibling,
        # "an inserted row is not where the pages show it", went with that
        # step: an insert's place is the derivation's (patch_set's "a new
        # item goes to the head").
        "            rows(&before),\n"
        "            rows(&items),\n",
        "            rows(&items),\n"
        "            rows(&items),\n",
    ),
    (
        "the pages open are not told when the menu is read again",
        "server",
        SERVER,
        "        if items == seen {\n",
        "        if true {\n",
    ),
    (
        "a document's menu is rendered again from its own read, untold",
        "server",
        SERVER,
        "            && let Some(from) = &from\n"
        "        {\n"
        "            return (entry.body, from.clone());\n"
        "        }\n"
        "        // Rendered first, or again where its entry was dropped: from what the\n"
        "        // pages show, where a page shows it.\n"
        "        let from = from.unwrap_or_else(|| rows.clone());\n",
        "            && from.as_ref() == Some(rows)\n"
        "        {\n"
        "            return (entry.body, rows.clone());\n"
        "        }\n"
        "        let from = rows.clone();\n",
    ),
    (
        "`/bench/stock?tell=true` tells nothing",
        "browser",
        SERVER,
        '            if param("tell").as_deref() == Some("true")\n',
        '            if false && param("tell").as_deref() == Some("true")\n',
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
}
BROWSER = ["e2e/availability.spec.mjs", "--project=chromium"]

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
    "render": cargo_tests,
    "server": cargo_tests,
    "browser": browser_tests,
}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed", flush=True)
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
            mutation_baseline.explain()
            return 1

    survivors = 0
    try:
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
    finally:
        # The page and the server as the source says, whatever a browser
        # mutant left built.
        subprocess.run(["bash", "spikes/own-renderer/run.sh"], cwd=ROOT,
                       env={**os.environ, "BUILD_ONLY": "1"}, capture_output=True)
        subprocess.run(["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server"],
                       cwd=ROOT, capture_output=True)

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
