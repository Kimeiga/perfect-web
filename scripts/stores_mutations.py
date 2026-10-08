#!/usr/bin/env python3
"""Mutation controls for ADR-0162: the store at its route, and a second
store at its own.

Each mutant undoes one piece:
- the route: a path's segment decoded as a query is, a path of another
  length taken for the route, and no page named by any route;
- the store: a page's parameter given the default store whatever the
  address says, the second store not held, one menu fragment for every
  store, and a change to one store's menu sent to every store's pages.

A mutant must fail the server's tests, all of them run, or the stores'
browser test in Chromium.

Run from the repository root; `just e14-stores` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a path's segment is decoded as a query",
        "cargo",
        SERVER,
        "                let value = path_decoded(p).filter(|v| !v.is_empty())?;\n",
        "                let value = percent_decoded(p).filter(|v| !v.is_empty())?;\n",
    ),
    (
        "a path of another length is taken for the route",
        "cargo",
        SERVER,
        "    let (route, path) = (segments(route), segments(path));\n"
        "    if route.len() != path.len() {\n",
        "    let (route, path) = (segments(route), segments(path));\n"
        "    if route.len() > path.len() {\n",
    ),
    (
        "no page is named by its route",
        "browser",
        SERVER,
        "            route_params(route, path).map(|params| (page.clone(), params))\n",
        "            route_params(route, path).map(|params| (page.clone(), params)).filter(|_| false)\n",
    ),
    (
        "a page's parameter is the default store's",
        "cargo",
        SERVER,
        "                        .map(Val::String)\n"
        "                        .ok_or_else(|| format!(\"the page's `{name}` is given no value here\"))\n",
        "                        .map(|_| Val::String(STORE_ID.into()))\n"
        "                        .ok_or_else(|| format!(\"the page's `{name}` is given no value here\"))\n",
    ),
    (
        "the second store is not held",
        "cargo",
        SERVER,
        "        _ if id == SECOND_STORE.0 => Some(SECOND_STORE.1),\n",
        "        _ if id == SECOND_STORE.0 => None,\n",
    ),
    (
        "one menu fragment for every store",
        "cargo",
        SERVER,
        "        EntryKey::from_identity(&menu_identity(store))\n",
        "        EntryKey::from_identity(&menu_identity(STORE_ID))\n",
    ),
    (
        "a change to one store's menu reaches every store's pages",
        "cargo",
        SERVER,
        "        for (_, waiting) in queue.iter_mut().filter(|(doc, _)| readers(doc)) {\n",
        "        for (_, waiting) in queue.iter_mut().filter(|(doc, _)| readers(doc) || true) {\n",
    ),
]

CARGO = [["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"]]
BROWSER = ["e2e/stores.spec.mjs"]

# How long one command may run. A mutant can make a test wait for ever (a
# lock taken twice); past the bound it is killed with everything it started,
# and the run counts as failing.
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


def cargo_tests():
    """(built, passed, failed) over every cargo test command."""
    built, passed, failed = True, 0, 0
    for cmd in CARGO:
        out, code = bounded(cmd, cwd=ROOT)
        if code is None:
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def browser_tests():
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
        ["pnpm", "exec", "playwright", "test", *BROWSER, "--project=chromium",
         "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {"cargo": cargo_tests, "browser": browser_tests}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run()
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
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
            built, passed, failed = SUITES[suite]()
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
    # The page is built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
