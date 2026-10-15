#!/usr/bin/env python3
"""Mutation controls for NEXT 4b (W9): a read is a bounded number of
statements on PostgreSQL. Each mutant restores one loop that reads a store
at a time, each of which the store ran until 4b:

- the layer (`store_pg.rs`): the items' stores scanned store by store, every
  store's menu read; the stores' hours read a store at a time;
- the operations: `hours#for-stores` reading each store's hours alone; the
  search's reach reading each store it found alone; `checkout#parts`,
  `hours#for-cart` and the order's reach (`cart_reaches`) finding each line's
  store by scanning every store's menu.

A mutant must fail `tests::bounded_statements::`, the store's operations
and its pages and commands counted on PostgreSQL at the seed and with fifty
stores more (`postgres`, which needs PW_STORE_DATABASE_URL).

Run from the repository root; `just e14-bounded-statements` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "spikes/own-renderer/server/src"
STORE_PG = SRC / "store_pg.rs"
HOURS_RS = SRC / "hours.rs"
SEARCH_RS = SRC / "search.rs"
CHECKOUT_RS = SRC / "checkout.rs"
STORE_RS = SRC / "store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "PostgreSQL's items' stores are scanned store by store",
        "postgres",
        STORE_PG,
        "    fn stores_of_items(&mut self, items: &[String]) -> Result<BTreeMap<String, String>, String> {\n",
        "    fn stores_of_items(&mut self, items: &[String]) -> Result<BTreeMap<String, String>, String> {\n"
        "        #[allow(unreachable_code)]\n"
        "        return crate::store::scanned_stores_of_items(self, items);\n",
    ),
    (
        "PostgreSQL's stores' hours are read a store at a time",
        "postgres",
        STORE_PG,
        "        crate::hours::pg_hours_of(&mut self.0, stores)\n",
        "        crate::store::hours_one_by_one(self, stores)\n",
    ),
    (
        "hours#for-stores reads each store's hours alone",
        "postgres",
        HOURS_RS,
        "                    let found = r.hours_of(&stores)?;\n",
        "                    let found = crate::store::hours_one_by_one(r, &stores)?;\n",
    ),
    (
        "the search's reach reads each store it found alone",
        "postgres",
        SEARCH_RS,
        "        match reach.remove(&m.id).flatten() {\n",
        "        match crate::store::coverage(r, &m.id, reader)? {\n",
    ),
    (
        "checkout#parts scans every store's menu for each line's store",
        "postgres",
        CHECKOUT_RS,
        "    let of_item = r.stores_of_items(&items)?;\n",
        "    let of_item = crate::store::scanned_stores_of_items(r, &items)?;\n",
    ),
    (
        "hours#for-cart scans every store's menu for each line's store",
        "postgres",
        HOURS_RS,
        "                let of_item = r.stores_of_items(&items)?;\n",
        "                let of_item = crate::store::scanned_stores_of_items(r, &items)?;\n",
    ),
    (
        "the order's reach scans every store's menu for each line's store",
        "postgres",
        STORE_RS,
        "    let of_item = r.stores_of_items(&items)?;\n",
        "    let of_item = crate::store::scanned_stores_of_items(r, &items)?;\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::bounded_statements::",
]
CARGO = {
    "postgres": (TESTS, {"PW_STORE_TEST_LAYER": "postgres"}),
}

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def bounded(cmd, env):
    """(output, returncode), or (output, None) when it ran past the bound."""
    p = subprocess.Popen(cmd, cwd=ROOT, env={**os.environ, **env}, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
        return out, p.returncode
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
        return out, None


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    cmd, env = CARGO[suite]
    out, code = bounded(cmd, env)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    if not os.environ.get("PW_STORE_DATABASE_URL"):
        print("FAIL: the postgres suite needs PW_STORE_DATABASE_URL, a throwaway database")
        return 1
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
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
            built, passed, failed = cargo_tests(suite)
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

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
