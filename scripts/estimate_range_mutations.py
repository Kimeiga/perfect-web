#!/usr/bin/env python3
"""Mutation controls for ADR-0180: a delivery estimate is a range, and says
when it was made (charter §15.1).

Each mutant undoes one piece:
- the data layer: no range unless asked; the control's `max` ignored; no
  `generated_at`; the benchmark's own `minutes` no longer answered;
- the page: the least minutes alone;
- the domain: a bound that is any `Int`, which the host then does not check.

A data-layer, page or domain mutant must fail the development server's
tests, all of them run; the control's, `e2e/slots.spec.mjs` in Chromium.

Run from the repository root; `just e14-estimate-range` records the output.
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
APP = ROOT / "examples/store/app.pw"
DOMAIN = ROOT / "examples/domain.pw"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "no range unless one is asked for",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        '                    ("max-minutes".into(), Val::S64(max.unwrap_or(minutes + 10))),\n',
        '                    ("max-minutes".into(), Val::S64(max.unwrap_or(minutes))),\n',
    ),
    (
        "no `generated_at` is answered",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows; and by track store-accounts, the store's
        # page reading the estimate to the reader's chosen address.
        '                    ("max-minutes".into(), Val::S64(max + travel)),\n'
        '                    ("generated-at".into(), Val::S64(wall_millis())),\n',
        '                    ("max-minutes".into(), Val::S64(max + travel)),\n',
    ),
    (
        "the benchmark's own `minutes` is no longer answered",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        '                    ("minutes".into(), Val::S64(minutes)),\n',
        "",
    ),
    (
        "the page says the least minutes alone",
        "server",
        APP,
        "Delivery in {estimate.min_minutes} to {estimate.max_minutes} min",
        "Delivery in {estimate.min_minutes} min",
    ),
    (
        "a bound is any `Int`, unchecked",
        "server",
        DOMAIN,
        "    min_minutes: PositiveInt,\n",
        "    min_minutes: Int,\n",
    ),
    (
        "the control's `max` is ignored",
        "browser",
        SERVER,
        # Re-anchored by track store-pg: the estimate is written through the
        # layer.
        '            let max = q("max").and_then(|v| v.parse().ok());\n',
        "            let max: Option<i64> = None;\n",
    ),
]

CARGO = {
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
}
BROWSER = ["e2e/slots.spec.mjs", "--project=chromium"]

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
