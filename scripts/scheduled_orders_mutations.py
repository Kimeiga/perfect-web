#!/usr/bin/env python3
"""Mutation controls for track `scheduled-orders` (W9, NEXT 4's last step,
the integrator's design of 2026-10-14): an order placed for a time its
reader chooses.

Each mutant undoes one rule:
- the times offered (`examples/store/scheduled.pw`): a window within the
  hour offered; one more than a week ahead offered; one while a store is
  closed, or before it opens; one store's hours standing for the cart's; a
  time between two quarter hours taken as offered; a day's windows running
  past its midnight;
- a time held to them: chosen whatever it is; as soon as possible keeping a
  time chosen; placed for a time no longer offered; an order for a time
  held to the hours now, as one for as soon as possible is; the checkout
  saying a time it no longer offers is scheduled;
- when its stores are told: with no lead; the lead alone; by the
  estimate's least; by the last store's estimate, not the longest; at once
  where paid at the provider, or where paid at once; never;
- the words: a window said an hour off; a scheduled order said an hour
  off; noon said AM; an evening said on a 24-hour clock; a week on said
  this week; the day the page's address names not shown; the order page
  and the cart not saying when;
- the clock (`packages/pw-platform-web/clock.pw`): an instant moved later
  moving back; an instant before itself;
- the layers: an order paid for a time placed at once, not scheduled; any
  time taken as the paid order's; a second order placed beside a
  scheduled one; a send placing another order, or one not scheduled; the
  time chosen not kept; memory keeping no order's time; a time sent after
  its window taken; PostgreSQL keeping the first time chosen, forgetting
  it, reading an order's window as when it is sent, and keeping the first
  order's reference for a later order's time.

A mutant must fail the development server's tests of the track in memory
(`memory`) or on PostgreSQL (`postgres`, which needs PW_STORE_DATABASE_URL),
narrowed by name to `tests::scheduled_orders::` and the module's own
(`scheduled_orders::tests`). Each `.pw` mutant is a well-typed program.

Run from the repository root; `just e14-scheduled-orders` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCHEDULED_PW = ROOT / "examples/store/scheduled.pw"
APP = ROOT / "examples/store/app.pw"
CLOCK = ROOT / "packages/pw-platform-web/clock.pw"
SERVER = ROOT / "spikes/own-renderer/server/src"
STORE = SERVER / "store.rs"
SCHEDULED_RS = SERVER / "scheduled_orders.rs"
CHECKOUT_RS = SERVER / "checkout.rs"

REACH = "    !clock.passed(clock.earlier(at, hours(1))) & clock.passed(clock.earlier(at, hours(168)))\n"
OPEN = "    List.any(spans, s => !clock.is_before(at, s.opens) & clock.is_before(at, s.closes))\n"
LEAD = "                Some(scheduled_for(at, clock.earlier(at, minutes(most + 5)), clock.zone(first.hours.zone)))\n"
MOST = "                    Ok(e) => if count(e.max_minutes) > m { count(e.max_minutes) } else { m },\n"
SETTLED_SEND = "            schedule send_to_store(settled.order) at s.sends\n"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a window within the hour is offered",
        "memory",
        SCHEDULED_PW,
        REACH,
        "    !clock.passed(at) & clock.passed(clock.earlier(at, hours(168)))\n",
    ),
    (
        "a window more than a week ahead is offered",
        "memory",
        SCHEDULED_PW,
        REACH,
        "    !clock.passed(clock.earlier(at, hours(1))) & clock.passed(clock.earlier(at, hours(192)))\n",
    ),
    (
        "a window is offered while its store is closed",
        "memory",
        SCHEDULED_PW,
        OPEN,
        "    List.any(spans, s => !clock.is_before(at, s.opens))\n",
    ),
    (
        "a window is offered before its store opens",
        "memory",
        SCHEDULED_PW,
        OPEN,
        "    List.any(spans, s => clock.is_before(at, s.closes))\n",
    ),
    (
        "one store's hours stand for the cart's",
        "memory",
        SCHEDULED_PW,
        "            List.filter(candidates(clock.zone(first.hours.zone)), w => List.all(each, spans => open_at(spans, w)) & in_reach(w))\n",
        "            List.filter(candidates(clock.zone(first.hours.zone)), w => List.any(each, spans => open_at(spans, w)) & in_reach(w))\n",
    ),
    (
        "a time between two quarter hours is held to be offered",
        "memory",
        SCHEDULED_PW,
        "        Some(first) => List.any(candidates(clock.zone(first.hours.zone)), w => same_instant(w, at)) &",
        "        Some(first) => List.any(candidates(clock.zone(first.hours.zone)), w => true) &",
    ),
    (
        "a day's windows run past its midnight",
        "memory",
        SCHEDULED_PW,
        "        windows: List.filter(offered, w => !clock.is_before(w, starts) & clock.is_before(w, ends)),\n",
        "        windows: List.filter(offered, w => !clock.is_before(w, starts) & clock.is_before(w, clock.later(ends, hours(240)))),\n",
    ),
    (
        "an order is placed for a time no longer offered",
        "memory",
        SCHEDULED_PW,
        "        Some(at) => if offered_at(stores, at) {\n",
        "        Some(at) => if true {\n",
    ),
    (
        "the checkout says a time it no longer offers is scheduled",
        "memory",
        SCHEDULED_PW,
        "            Some(first) => if offered_at(stores, at) {\n",
        "            Some(first) => if true {\n",
    ),
    (
        "the store is told with no lead",
        "memory",
        SCHEDULED_PW,
        LEAD,
        LEAD.replace("minutes(most + 5)", "minutes(most)"),
    ),
    (
        "the store is told the lead alone",
        "memory",
        SCHEDULED_PW,
        LEAD,
        LEAD.replace("minutes(most + 5)", "minutes(5)"),
    ),
    (
        "the estimate's least is counted",
        "memory",
        SCHEDULED_PW,
        MOST,
        MOST.replace("max_minutes", "min_minutes"),
    ),
    (
        "the last store's estimate is counted, not the longest",
        "memory",
        SCHEDULED_PW,
        MOST,
        "                    Ok(e) => count(e.max_minutes),\n",
    ),
    (
        "a window is said an hour off",
        "memory",
        SCHEDULED_PW,
        "    let said = clock_time_said(clock.minute_of_day(clock.time_of(at, zone)))\n",
        "    let said = clock_time_said(clock.minute_of_day(clock.time_of(clock.later(at, hours(1)), zone)))\n",
    ),
    (
        "a scheduled order is said an hour off",
        "memory",
        SCHEDULED_PW,
        "day: clock.weekday(clock.date_of(at, zone)), time: clock.time_of(at, zone) }\n",
        "day: clock.weekday(clock.date_of(at, zone)), time: clock.time_of(clock.later(at, hours(1)), zone) }\n",
    ),
    (
        "noon is said AM",
        "memory",
        SCHEDULED_PW,
        '    let half = if h < 12 { "AM" } else { "PM" }\n',
        '    let half = if h < 13 { "AM" } else { "PM" }\n',
    ),
    (
        "an evening is said on a 24-hour clock",
        "memory",
        SCHEDULED_PW,
        "    let hour = if h == 0 { 12 } else if h > 12 { h - 12 } else { h }\n",
        "    let hour = if h == 0 { 12 } else { h }\n",
    ),
    (
        "a week on is said as this week",
        "memory",
        SCHEDULED_PW,
        "    } else if n >= today + 7 {\n",
        "    } else if n >= today + 8 {\n",
    ),
    (
        "the day the page's address names is not shown",
        "memory",
        SCHEDULED_PW,
        '                Some(key) => match List.find(days, d => "{d.number}" == key) {\n',
        '                Some(key) => match List.find(days, d => "{d.number}" == "{key}-") {\n',
    ),
    (
        "a time is chosen whatever it is",
        "memory",
        APP,
        "        Some(at) => if offered_at(hours_of_cart(current_user()), at) {\n",
        "        Some(at) => if true {\n",
    ),
    (
        "as soon as possible keeps the time chosen",
        "memory",
        APP,
        "        None => Ok(set_time(current_user(), None).when),\n",
        "        None => Ok(None),\n",
    ),
    (
        "an order for a time is held to the hours now",
        "memory",
        APP,
        "    match held_to(hours, when) {\n",
        "    match held_to(hours, None) {\n",
    ),
    (
        "an order paid at the provider is sent at once",
        "memory",
        APP,
        SETTLED_SEND,
        "            schedule send_to_store(settled.order) after minutes(0)\n",
    ),
    (
        "an order paid at the provider is never sent",
        "memory",
        APP,
        SETTLED_SEND,
        "",
    ),
    (
        "an order paid at once is sent at once",
        "memory",
        APP,
        "                                schedule send_to_store(order) at s.sends\n",
        "                                schedule send_to_store(order) after minutes(0)\n",
    ),
    (
        "the order page does not say when",
        "memory",
        APP,
        '{:Scheduled(_)}<p id="order-status">Scheduled for {when_scheduled(order)}.</p>\n',
        '{:Scheduled(_)}<p id="order-status">Scheduled.</p>\n',
    ),
    (
        "the cart does not say when",
        "memory",
        APP,
        '<p id="cart-when">{checkout.when_said} <a id="choose-when"',
        '<p id="cart-when"><a id="choose-when"',
    ),
    (
        "an instant moved later moves back",
        "memory",
        CLOCK,
        "    Instant(at.value + by.value)\n",
        "    Instant(at.value - by.value)\n",
    ),
    (
        "an instant is before itself",
        "memory",
        CLOCK,
        "    a.value < b.value\n",
        "    a.value <= b.value\n",
    ),
    (
        "an order paid for a time is placed at once",
        "memory",
        STORE,
        "                    let settled = if timed {\n",
        "                    let settled = if false {\n",
    ),
    (
        "any time is taken as the paid order's",
        "memory",
        STORE,
        "                        .is_some_and(|t| t.reference == *reference);\n",
        "                        .is_some();\n",
    ),
    (
        "a second order is placed beside a scheduled one",
        "memory",
        STORE,
        "                if current.as_deref() == Some(crate::scheduled_orders::SCHEDULED) {\n",
        "                if false {\n",
    ),
    (
        "memory keeps no order's time",
        "memory",
        STORE,
        "        self.state().times.insert(owner.to_string(), time.clone());\n",
        "",
    ),
    (
        "a send places another order",
        "memory",
        SCHEDULED_RS,
        "                    .is_some_and(|t| t.reference == *reference);\n",
        "                    .is_some();\n",
    ),
    (
        "a send places an order that is not scheduled",
        "memory",
        SCHEDULED_RS,
        "                if order.status == SCHEDULED && ours {\n",
        "                if ours {\n",
    ),
    (
        "the time chosen is not kept",
        "memory",
        SCHEDULED_RS,
        "                    chosen.when = when;\n",
        "",
    ),
    (
        "a time sent after its window starts is taken",
        "memory",
        SCHEDULED_RS,
        "    if sends > at || !(0..1440).contains(&minute) {\n",
        "    if !(0..1440).contains(&minute) {\n",
    ),
    (
        "PostgreSQL keeps the first time chosen",
        "postgres",
        CHECKOUT_RS,
        '             promo = EXCLUDED.promo, scheduled_for = EXCLUDED.scheduled_for",\n',
        '             promo = EXCLUDED.promo",\n',
    ),
    (
        "PostgreSQL forgets the time chosen",
        "postgres",
        CHECKOUT_RS,
        "                when: r.get(3),\n",
        "                when: None,\n",
    ),
    (
        "PostgreSQL reads an order's window as when it is sent",
        "postgres",
        SCHEDULED_RS,
        "        at: r.get(1),\n",
        "        at: r.get(2),\n",
    ),
    (
        "PostgreSQL keeps the first order's reference for a later order's time",
        "postgres",
        SCHEDULED_RS,
        "ON CONFLICT (owner) DO UPDATE SET reference = $2, at_ms = $3, sends_ms = $4, \\\n",
        "ON CONFLICT (owner) DO UPDATE SET at_ms = $3, sends_ms = $4, \\\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
    "tests::scheduled_orders::", "scheduled_orders::tests",
]
CARGO = {
    "memory": (TESTS, {"PW_STORE_TEST_LAYER": "memory", "PW_STORE_DATABASE_URL": ""}),
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
