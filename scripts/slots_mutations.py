#!/usr/bin/env python3
"""Mutation controls for ADR-0165: the store's delivery estimate and
recommendations.

Each mutant undoes one piece:
- the store's declarations: the recommendations kept a bounded time and
  dropped when the store's menu changes, and the estimate streamed;
- the development server: an event reaching a stream's kept answer, and each
  store's recommendations drawn from its own menu as it is now;
- the page: the estimate said to a screen reader when it comes.

A declaration or server mutant must fail the development server's tests, all
of them run: they build the store from `examples`, so a mutant of its source
is read when they run. The page's must fail `e2e/slots.spec.mjs` in
Chromium.

Run from the repository root; `just e14-slots` records the output. The
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
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"
STORE = ROOT / "examples/store/app.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "the recommendations are kept for no time",
        "server",
        STORE,
        "    freshness      10.minutes\n    consistency    eventual\n",
        "    freshness      0.seconds\n    consistency    eventual\n",
    ),
    (
        "the recommendations are kept through a change to the menu",
        "server",
        STORE,
        "    invalidates_on MenuChanged(id)\n"
        "    concurrency    one_per_key\n"
        "    on_key_change  cancel\n"
        "    delivery       streamed\n",
        "    concurrency    one_per_key\n"
        "    on_key_change  cancel\n"
        "    delivery       streamed\n",
    ),
    (
        "the page waits for the estimate",
        "server",
        STORE,
        # Re-anchored by track store-accounts: the estimate is the reader's,
        # its clauses aligned to `invalidates_on`.
        "    invalidates_on AddressesChanged(reader)\n    concurrency    one_per_key\n"
        "    on_key_change  cancel\n    delivery       streamed\n    timeout        3.seconds\n",
        "    invalidates_on AddressesChanged(reader)\n    concurrency    one_per_key\n"
        "    on_key_change  cancel\n    timeout        3.seconds\n",
    ),
    (
        "an event never reaches a stream's kept answer",
        "server",
        SERVER,
        # Re-anchored by ADR-0190: a query's policy is read on every page.
        "                    [\"bindings\", \"streams\"]\n"
        "                        .into_iter()\n",
        "                    [\"bindings\"]\n"
        "                        .into_iter()\n",
    ),
    (
        "every store is recommended store 47's menu",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                    None => match menu_of(r, store)? {\n",
        "                    None => match menu_of(r, STORE_ID)? {\n",
    ),
    (
        "the recommendations are drawn from the menu as it was",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                        Some(menu) => recommended_from(\n"
        "                            &menu\n",
        "                        Some(_) => recommended_from(\n"
        "                            &State::seed().menus[store.as_str()]\n",
    ),
    (
        "the estimate is not said to a screen reader",
        "browser",
        STORE,
        "            <section aria-label=\"Delivery\" aria-live=\"polite\">\n",
        "            <section aria-label=\"Delivery\">\n",
    ),
]

CARGO = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"]
BROWSER = ["e2e/slots.spec.mjs"]

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


def cargo_tests():
    """(built, passed, failed) over the server's tests."""
    out, code = bounded(CARGO, cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


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


SUITES = {"server": cargo_tests, "browser": browser_tests}


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
        print(f"{what} [{suite}]: {verdict}")
    # The page and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
