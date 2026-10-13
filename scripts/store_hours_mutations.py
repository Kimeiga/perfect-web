#!/usr/bin/env python3
"""Mutation controls for track `store-hours` (ADR-0328, docs/PARALLEL.md,
W9): a store's opening hours, whether it is open by comparison with the
platform's clock (ADR-0304), shown on its page, the home page, a search and
the cart, and an order refused while closed or past the last order.

Each mutant undoes one piece:
- the program (`examples/store/hours.pw`): a period that runs past midnight
  closing the same day; a special date ignored; the days' spans not joined;
  the cut-off ignored; the cut-off not borrowed from the day before a
  midnight closing; a store open before it opens; "tomorrow" said of the
  day after; a time in the evening said "am"; a store open the day round
  not said so; the home page's stores kept past a change to their hours;
  overlapping periods joined only where one ends as the next starts;
- the command (`examples/store/app.pw`): an order placed whatever the hours;
- the layers: memory's hours not written; memory's hours held in the
  order a set lists them, not as PostgreSQL returns them; a weekday past 7
  not refused; PostgreSQL's special date written a day late; PostgreSQL's
  weekly periods read in no order.

A mutant must fail the development server's hours tests in memory
(`server`), or on PostgreSQL (`postgres`, which needs PW_STORE_DATABASE_URL),
narrowed by name to `tests::store_hours::`, the tests that can see their
rules.

Run from the repository root; `just e14-store-hours` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
HOURS_PW = ROOT / "examples/store/hours.pw"
APP = ROOT / "examples/store/app.pw"
HOURS_RS = ROOT / "spikes/own-renderer/server/src/hours.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a period past midnight closes the same day",
        "server",
        HOURS_PW,
        "    let to = if p.closes <= p.opens { normal(n + 1, p.closes) } else { normal(n, p.closes) }\n",
        "    let to = normal(n, p.closes)\n",
    ),
    (
        "a special date is ignored",
        "server",
        HOURS_PW,
        "    let periods = match List.find(h.special, s => s.date == n) {\n",
        "    let periods = match List.find(h.special, s => false) {\n",
    ),
    (
        "the days' spans are not joined",
        "server",
        HOURS_PW,
        "    List.fold(each, none, (out, s) => joined(out, s))\n",
        "    each\n",
    ),
    (
        "the cut-off is ignored",
        "server",
        HOURS_PW,
        "        Open(o) => if clock.passed(o.last_order) {\n",
        "        Open(o) => if false {\n",
    ),
    (
        "the cut-off is not borrowed from the day before a midnight closing",
        "server",
        HOURS_PW,
        "    if s.to >= cutoff {\n",
        "    if true {\n",
    ),
    (
        "a store is open before it opens",
        "server",
        HOURS_PW,
        "    let all = spans(h, zone, today)\n"
        "    match List.find(all, s => clock.passed(s.opens) & !clock.passed(s.closes)) {\n"
        "        Some(s) => OpenState.Open(",
        "    let all = spans(h, zone, today)\n"
        "    match List.find(all, s => !clock.passed(s.closes)) {\n"
        "        Some(s) => OpenState.Open(",
    ),
    (
        "the day after is said tomorrow",
        "server",
        HOURS_PW,
        "    } else if n == today + 1 {\n",
        "    } else if n == today + 2 {\n",
    ),
    (
        "an evening is said am",
        "server",
        HOURS_PW,
        '    let half = if h < 12 { "am" } else { "pm" }\n',
        '    let half = if h < 20 { "am" } else { "pm" }\n',
    ),
    (
        "a store open the day round is not said so",
        "server",
        HOURS_PW,
        "fn always_open(h: OpeningHours) -> Bool !{} {\n",
        "fn always_open(h: OpeningHours) -> Bool !{} {\n    false &\n",
    ),
    (
        "the home page's stores are kept past a change to their hours",
        "server",
        HOURS_PW,
        "    invalidates_on StoreChanged(_)\n",
        "",
    ),
    # The integrator's rulings of 2026-10-12: a day's periods are their
    # union; the layers hand them ordered by opening (the two layers'
    # mutants below), whatever order a set lists them in.
    (
        "overlapping periods are joined only where one ends as the next starts",
        "server",
        HOURS_PW,
        "        Some(last) => if not_after(next.from_day, next.from, last.to_day, last.to) {\n",
        "        Some(last) => if (next.from_day == last.to_day) & (next.from == last.to) {\n",
    ),
    (
        "an order is placed whatever the hours",
        "server",
        APP,
        "        Some(closing) => Err(OrderError.Closed(closing)),\n",
        "        Some(closing) => UserOrders.place(current_user()),\n",
    ),
    (
        "memory's hours are not written",
        "server",
        STORE_DATA,
        "        self.state().hours.insert(store.to_string(), held);\n",
        "",
    ),
    (
        "memory holds a set in the order it lists",
        "server",
        STORE_DATA,
        "        let held = crate::hours::held(hours);\n",
        "        let held = hours.clone();\n",
    ),
    (
        "a weekday past 7 is not refused",
        "server",
        HOURS_RS,
        "        if !(1..=7).contains(&p.weekday) || !minute(p.opens) || !minute(p.closes) {\n",
        "        if !(1..=8).contains(&p.weekday) || !minute(p.opens) || !minute(p.closes) {\n",
    ),
    (
        "PostgreSQL's special date is written a day late",
        "postgres",
        HOURS_RS,
        "             VALUES ($1, DATE '1970-01-01' + $2::integer, $3, $4) \\\n",
        "             VALUES ($1, DATE '1970-01-01' + $2::integer + 1, $3, $4) \\\n",
    ),
    (
        "PostgreSQL's weekly periods are read in no order",
        "postgres",
        HOURS_RS,
        "             WHERE store = $1 ORDER BY weekday, opens_minute\",\n",
        "             WHERE store = $1\",\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::store_hours::",
]
CARGO = {
    "server": (TESTS, {"PW_STORE_TEST_LAYER": "memory", "PW_STORE_DATABASE_URL": ""}),
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
