#!/usr/bin/env python3
"""Mutation controls for ADR-0192: the stores, as the home page.

Each mutant undoes one piece of the development server's:
- the root answered with store 47's page again, as before;
- the host's list of stores holding one store.

Every mutant must fail the development server's test of the home page.

Run from the repository root; `just e14-home` records the output. The
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

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the root is store 47's page again",
        SERVER,
        "        (\"GET\", \"/StorePage.html\") => {\n",
        "        (\"GET\", \"/StorePage.html\") | (\"GET\", \"/\") => {\n",
    ),
    (
        "the host lists one store",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "            [] => w(&mut |r| Ok(vec![Val::List(r.stores()?.iter().map(store_val).collect())])),\n",
        "            [] => w(&mut |r| Ok(vec![Val::List(r.stores()?.iter().take(1).map(store_val).collect())])),\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "the_stores_are_the_home_page",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests."""
    p = subprocess.Popen(TESTS, cwd=ROOT, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(n) for n, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
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
            built, passed, failed = run_tests()
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
