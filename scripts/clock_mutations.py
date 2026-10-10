#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: what a page shows by the clock is told
when the clock passes it.

Each mutant undoes one piece:
- the zones (`pw-time`): a skipped local time read with the offset after the
  gap, a repeated one as its last occurrence; past the last transition no
  rule, before the first the first transition's offset; daylight time south
  of the equator never; a rule's start read in daylight time; a day begun at
  a midnight a transition skipped; a file's footer unread;
- the checker's: a page built once, or a public materialization, let
  compare the clock;
- the host's: an instant ahead not noted; `passed` answering before its
  instant; a zone's date held by nothing; a read within a read telling the
  outer nothing; a fresh value's instant not kept, a kept one past it
  served, or its instant not noted for the read it is part of; a document's
  reads setting no timer; a later instant replacing an earlier, or a
  replaced one taking its document; a document due not read again; the
  host's clock not the system's, or moved back; the development origin
  granting no clock.

The zones' must fail `pw-time`'s tests; the checker's, `pw-core`'s (its
library and `tests/clock_compared.rs`); the host's, the development
server's tests of the clock. Run from the repository root; `just e14-clock`
records the output. The source is restored after every mutant, whatever
happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ZONES = ROOT / "runtime/pw-time/src/lib.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
RULES = ROOT / "compiler/pw-core/src/rules.rs"
CLOCK = ROOT / "spikes/own-renderer/server/src/clock.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a skipped local time is read with the offset after the gap",
        "zones",
        ZONES,
        "            (None, None) => local - i64::from(before),\n",
        "            (None, None) => local - i64::from(after),\n",
    ),
    (
        "a repeated local time is its last occurrence",
        "zones",
        ZONES,
        "            (Some(a), Some(b)) => a.min(b),\n",
        "            (Some(a), Some(b)) => a.max(b),\n",
    ),
    (
        "past the last transition, no rule",
        "zones",
        ZONES,
        "            (Some(rule), n) if n == self.transitions.len() => rule.offset_at(instant),\n",
        "            (Some(rule), n) if n == self.transitions.len() && n == 0 => rule.offset_at(instant),\n",
    ),
    (
        "before the first transition, the first transition's offset",
        "zones",
        ZONES,
        "            (_, 0) => self.first,\n",
        "            (_, 0) => self.transitions.first().map_or(self.first, |t| t.1),\n",
    ),
    (
        "daylight time south of the equator is never",
        "zones",
        ZONES,
        "            instant < end || start <= instant\n",
        "            start <= instant && instant < end\n",
    ),
    (
        "a rule's start is read in daylight time",
        "zones",
        ZONES,
        "        let start = begins(daylight.start, self.standard);\n",
        "        let start = begins(daylight.start, daylight.offset);\n",
    ),
    (
        "a day begins at a midnight a transition skipped",
        "zones",
        ZONES,
        "        self.instant(day + 1, 0)\n",
        "        (day + 1) * DAY - i64::from(self.offset_at(instant))\n",
    ),
    (
        "a file's footer is unread",
        "zones",
        ZONES,
        "        let rule = if wide {\n",
        "        let rule = if wide && false {\n",
    ),
    (
        "a page built once may compare the clock",
        "core",
        RULES,
        '        "build" if effect.starts_with("clock.compare") => Some((\n',
        '        "build" if false && effect.starts_with("clock.compare") => Some((\n',
    ),
    (
        "a page built once, or a public materialization, may compare the clock",
        "core",
        EFFECTS,
        '    if reuse != Reuse::PerReader && effect.starts_with("clock.compare") {\n',
        '    if reuse == Reuse::Build && effect.starts_with("clock.wall.never") {\n',
    ),
    (
        "a public materialization may compare the clock",
        "core",
        EFFECTS,
        '    if reuse != Reuse::PerReader && effect.starts_with("clock.compare") {\n',
        '    if reuse == Reuse::Build && effect.starts_with("clock.compare") {\n',
    ),
    (
        "an instant ahead is not noted",
        "server",
        CLOCK,
        "            if at > now {\n                note(at);\n            }\n",
        "",
    ),
    (
        "`passed` answers before its instant",
        "server",
        CLOCK,
        "            Ok(vec![Val::Bool(now >= at)])\n",
        "            Ok(vec![Val::Bool(now > at)])\n",
    ),
    (
        "a zone's date is held by nothing",
        "server",
        CLOCK,
        "                note(zone.next_day(seconds(now)) * 1000);\n",
        "",
    ),
    (
        "a read within a read tells the outer nothing",
        "server",
        CLOCK,
        "    if let Some(at) = until {\n        note(at);\n    }\n",
        "",
    ),
    (
        "a later instant replaces an earlier",
        "server",
        CLOCK,
        "        if due.at.get(doc).is_some_and(|&at| at <= until) {\n",
        "        if due.at.get(doc).is_some_and(|&at| at >= until) {\n",
    ),
    (
        "an instant a later read replaced takes its document",
        "server",
        CLOCK,
        "            if due.at.get(&doc) == Some(&at) {\n",
        "            if due.at.contains_key(&doc) || true {\n",
    ),
    (
        "a fresh value's instant is not kept",
        "server",
        SERVER,
        "                    Some(until) => held.insert(key.clone(), until),\n",
        "                    Some(_) => held.remove(key),\n",
    ),
    (
        "a kept value past its instant is served",
        "server",
        SERVER,
        "            let passed = held.get(&key).is_some_and(|&until| until <= self.now());\n",
        "            let passed = held.get(&key).is_some_and(|&until| until <= i64::MIN);\n",
    ),
    (
        "a kept value's instant is not noted for the read it is part of",
        "server",
        SERVER,
        "                        clock::note(until);\n",
        "                        let _ = until;\n",
    ),
    (
        "a document's reads set no timer",
        "server",
        SERVER,
        "            self.timers.hold(&(session.to_string(), document), until);\n",
        "            let _ = (document, until);\n",
    ),
    (
        "a document due is not read again",
        "server",
        SERVER,
        '            .expect("one change of a session at a time");\n'
        "        self.clock.advance(1);\n"
        "        let version = Version(self.clock.now());\n"
        "        self.send_documents_where(\n"
        "            session,\n"
        "            |d| d == doc,\n",
        '            .expect("one change of a session at a time");\n'
        "        self.clock.advance(1);\n"
        "        let version = Version(self.clock.now());\n"
        "        self.send_documents_where(\n"
        "            session,\n"
        "            |d| d != doc,\n",
    ),
    (
        "the host's clock is not the system's",
        "server",
        SERVER,
        "            epoch: wall_millis(),\n",
        "            epoch: 0,\n",
    ),
    (
        "the host's clock moves back",
        "server",
        SERVER,
        "            match ahead.and_then(|ms| u64::try_from(ms).ok()) {\n",
        "            match ahead.map(i64::unsigned_abs) {\n",
    ),
    (
        "the development origin grants no clock",
        "server",
        SERVER,
        '                "clock.compare",\n',
        "",
    ),
]

CARGO = {
    "zones": ["cargo", "test", "--quiet", "--locked", "-p", "pw-time"],
    "core": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--lib", "--test", "clock_compared",
    ],
    "server": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "clock::", "a_page_is_told_when", "a_kept_value_past", "a_page_that_compares_no",
        "the_hosts_clock",
    ],
}

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


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
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
