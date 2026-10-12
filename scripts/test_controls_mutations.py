#!/usr/bin/env python3
"""Mutation controls for ADR-0174: charter §15.5's store delay, cart delay
and one-shot database error.

Each mutant undoes one piece:
- the fault: not consumed, so it fails every time; another session's
  taken; a write's taken by a read; the cart's delay not waited, or
  waited by every session; the store's delay not waited;
- the drain: a document's drain reads the cart when nothing changed, so a
  one-shot read error meets a read nobody sees;
- the controls: `/bench/fail?next=write` arms nothing; `/bench/cart` sets
  another session's delay; `/bench/store` leaves the kept store in place.

A fault or drain mutant must fail the development server's tests, all of
them run; a control mutant, `e2e/controls.spec.mjs` and
`e2e/resource-path.spec.mjs` in Chromium.

Run from the repository root; `just e14-test-controls` records the output.
The source is restored after every mutant, whatever happens.
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

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a fault is not consumed",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                        std::mem::take(if writes {\n",
        "                        *(if writes {\n",
    ),
    (
        "another session's fault is taken",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                    let mut mine = all.get_mut(&session);\n",
        "                    let mut mine = all.values_mut().next();\n",
    ),
    (
        "a write's fault is taken by a read",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        # And by track store-accounts: each operation by the session and by
        # the reader's handle.
        "                                &mut m.fail_write\n"
        "                            } else {\n"
        "                                &mut m.fail_read\n",
        "                                &mut m.fail_read\n"
        "                            } else {\n"
        "                                &mut m.fail_write\n",
    ),
    (
        "the cart's delay is not waited",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        # And by track store-accounts: each operation by the session and by
        # the reader's handle.
        "                    if delay > 0 {\n"
        "                        std::thread::sleep(std::time::Duration::from_millis(delay));\n"
        "                    }\n"
        "                    op(args)\n",
        "                    op(args)\n",
    ),
    (
        "every session waits a cart's delay",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        # And by track store-accounts: each operation by the session and by
        # the reader's handle.
        "                        mine.map(|m| if writes { 0 } else { m.delay_ms })\n"
        "                            .unwrap_or_default()\n",
        "                        mine.map(|m| if writes { 0 } else { m.delay_ms })\n"
        "                            .unwrap_or(1000)\n",
    ),
    (
        "the store's delay is not waited",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "                let delay = store_delay.load(Ordering::SeqCst);\n",
        "                let delay = store_delay.load(Ordering::SeqCst) * 0;\n",
    ),
    (
        "a drain reads the cart when nothing changed",
        "server",
        SERVER,
        # Re-anchored by ADR-0176: a stale entry is read, to be tried again.
        "        if existing && !stale && invalidated.is_empty() {\n"
        "            return;\n"
        "        }\n",
        "",
    ),
    (
        "`next=write` arms nothing",
        "browser",
        SERVER,
        "                Some(\"write\") => mine.fail_write = true,\n",
        "                Some(\"write\") => {}\n",
    ),
    (
        "`/bench/cart` sets another session's delay",
        "browser",
        SERVER,
        "                .entry(session.clone())\n"
        "                .or_default()\n"
        "                .delay_ms = delay;\n",
        "                .entry(\"nobody\".to_string())\n"
        "                .or_default()\n"
        "                .delay_ms = delay;\n",
    ),
    (
        "`/bench/store` leaves the kept store in place",
        "browser",
        SERVER,
        # Re-anchored by ADR-0177: the store page's plan, and every page's.
        "                .store(delay, std::sync::atomic::Ordering::SeqCst);\n"
        "            let resources: std::collections::BTreeSet<String> = std::iter::once(&server.plan)\n",
        "                .store(delay, std::sync::atomic::Ordering::SeqCst);\n"
        "            let resources = std::collections::BTreeSet::<String>::new();\n"
        "            let _kept: std::collections::BTreeSet<String> = std::iter::once(&server.plan)\n",
    ),
]

CARGO = {
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
}
BROWSER = ["e2e/controls.spec.mjs", "e2e/resource-path.spec.mjs", "--project=chromium"]

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


SUITES = {"server": cargo_tests, "browser": browser_tests}


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
