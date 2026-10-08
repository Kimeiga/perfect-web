#!/usr/bin/env python3
"""Mutation controls for ADR-0191: every page speculates from its own module.

Each mutant undoes one piece of the development server's:
- the store's manifest alone read;
- a page given the store's speculation;
- a module no page but the store's names refused;
- a speculated value sent to the store's documents alone.

Every mutant must fail the development server's tests of a second page.

Run from the repository root; `just e14-page-speculation` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the store's manifest alone is read",
        SERVER,
        "        .filter(|p| p.extension().is_some_and(|x| x == \"json\"))\n",
        "        .filter(|p| p.ends_with(\"store.page.StorePage.json\"))\n",
    ),
    (
        "a page is given the store's speculation",
        SERVER,
        "        let speculation = server.speculations.get(page).and_then(|m| {\n",
        "        let speculation = server\n"
        "            .speculations\n"
        "            .get(server.store_page())\n"
        "            .and_then(|m| {\n",
    ),
    (
        "a module the store's manifest does not name is refused",
        SERVER,
        "                    .any(|s| s[\"module\"].as_str() == Some(m))\n",
        "                    .any(|s| s[\"page\"] == \"store.page.StorePage\" && s[\"module\"].as_str() == Some(m))\n",
    ),
    (
        "a speculated value is sent to the store's documents alone",
        SERVER,
        "                .speculates_on_cart(&page)\n",
        "                .speculates_on_cart(&page)\n"
        "                .filter(|_| page == self.store_page())\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "a_page_that_binds_a_query_is_served_at_its_route",
    "a_change_reaches_each_page_that_reads_it_by_its_own_plan",
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
