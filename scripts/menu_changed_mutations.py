#!/usr/bin/env python3
"""Mutation controls for ADR-0164: a menu change drops what declares it, for
its store.

Each mutant puts back one piece of the drop by name:
- the server drops every store's kept menu, by the query's name, or nothing;
- the event names no store, or another store;
- the store's `Menu` declares no `invalidates_on MenuChanged(id)`.

Each must fail the development server's tests, all of them run. The tests
build the store from `examples`, so a mutant of the store's source is read
when they run.

Run from the repository root; `just e14-menu-changed` records the output.
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
STORE = ROOT / "examples/store/app.pw"

# Re-anchored by ADR-0178, whose stock change is an event of its own.
EVENT = "        self.invalidate_queries(\"\", &[], &[(event.name.clone(), values)]);\n"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a menu change drops every store's menu, by the query's name",
        SERVER,
        EVENT,
        "        self.queries.invalidate(\"store.page.Menu\");\n",
    ),
    (
        "a menu change drops nothing",
        SERVER,
        EVENT,
        "",
    ),
    (
        "the event names no store",
        SERVER,
        # Re-anchored by ADR-0178.
        "            _ => pw_materialize::Event::new(\"Events.MenuChanged\", &[STORE_ID]),\n",
        "            _ => pw_materialize::Event::new(\"Events.MenuChanged\", &[]),\n",
    ),
    (
        "the event is another store's",
        SERVER,
        # Re-anchored by ADR-0178.
        "            _ => pw_materialize::Event::new(\"Events.MenuChanged\", &[STORE_ID]),\n",
        "            _ => pw_materialize::Event::new(\"Events.MenuChanged\", &[SECOND_STORE.0]),\n",
    ),
    (
        "the store's menu declares no invalidation",
        STORE,
        # Re-anchored by ADR-0165, whose recommendations declare it too, and
        # by ADR-0178, whose menu listens for a stock change as well.
        "    invalidates_on MenuChanged(id), InventoryChanged(id, _)\n"
        "    concurrency    one_per_key\n"
        "    on_key_change  cancel\n"
        "    timeout        2.seconds\n",
        "    invalidates_on InventoryChanged(id, _)\n"
        "    concurrency    one_per_key\n"
        "    on_key_change  cancel\n"
        "    timeout        2.seconds\n",
    ),
]

CARGO = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"]

# How long one run may take. Past the bound it is killed with everything it
# started, and the run counts as failing.
BOUND = 900


def run():
    """(built, passed, failed) over the server's tests."""
    p = subprocess.Popen(CARGO, cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
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
    built, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed")
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
            built, passed, failed = run()
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

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
