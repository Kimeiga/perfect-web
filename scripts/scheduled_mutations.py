#!/usr/bin/env python3
"""Mutation controls for the scheduled-commands track: a command runs when
the clock passes its time.

Each mutant undoes one piece of the language's side (step one):
- the grammar: a schedule's time read as words;
- the checker: a command a reader presses scheduled; a schedule outside a
  command taken; a schedule that says no time taken; a run's time of any
  type, or of a program's own type spelt as the clock's;
- the relations: a schedule's arguments related to nothing; its time
  related as the command's argument;
- the effects: a schedule performing nothing;
- `invoked_by Clock`: the clock no invoker; a payment's one `Settled`, or
  its one command, asked of the clock.

Each must fail the compiler's tests of scheduling (`tests/scheduled.rs`),
of `invoked_by` (`tests/invoked_by.rs`), or the corpus's
(`generality.rs`, `checking_source.rs`).

Run from the repository root; `just e14-scheduled` records the output. The
sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
SCHEDULED = ROOT / "compiler/pw-core/src/scheduled.rs"
VALUES = ROOT / "compiler/pw-core/src/values.rs"
EFFECTS = ROOT / "compiler/pw-core/src/effects.rs"
INVOKED = ROOT / "compiler/pw-core/src/invoked.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a schedule's time is read as words",
        GRAMMAR,
        '        if keyword == "schedule"\n            && matches!(self.cur_text(), "at" | "after")\n',
        '        if keyword == "never"\n            && matches!(self.cur_text(), "at" | "after")\n',
    ),
    (
        "a command a reader presses is scheduled",
        SCHEDULED,
        '                    && t.policy("invoked_by")\n                        .is_some_and(|p| p.value.trim() == "Clock")\n',
        "                    && true\n",
    ),
    (
        "a schedule outside a command is taken",
        SCHEDULED,
        "            if decl.kind != DeclKind::Command {\n",
        "            if false {\n",
    ),
    (
        "a schedule that says no time is taken",
        SCHEDULED,
        '                        && matches!(modifiers[1].as_str(), "at" | "after")\n',
        "                        && true\n",
    ),
    (
        "a run's time is of any type",
        SCHEDULED,
        "                (Ty::Nominal(d, _), Some(want)) => *d == want,\n",
        "                (Ty::Nominal(_, _), Some(_)) => true,\n",
    ),
    (
        "a program's own type spelt as the clock's is the clock's",
        SCHEDULED,
        "                (Ty::Nominal(d, _), Some(want)) => *d == want,\n                // A type",
        "                (Ty::Nominal(_, _), Some(_)) => shown.ends_with(expected),\n                // A type",
    ),
    (
        "a schedule's arguments are related to nothing",
        VALUES,
        '                "query" | "subscription" | "navigate" | "schedule" | "unschedule"\n',
        '                "query" | "subscription" | "navigate"\n',
    ),
    (
        "a schedule's time is related as the command's argument",
        VALUES,
        '                        ("schedule", Some("at" | "after")) => &args[..args.len().saturating_sub(1)],\n',
        '                        ("schedule", Some("never")) => &args[..args.len().saturating_sub(1)],\n',
    ),
    (
        "a schedule performs nothing",
        EFFECTS,
        '                && matches!(keyword.as_str(), "schedule" | "unschedule")\n            {\n                let effect',
        '                && matches!(keyword.as_str(), "never")\n            {\n                let effect',
    ),
    (
        "the clock invokes nothing",
        INVOKED,
        'pub const INVOKERS: &[&str] = &["Payments", "Clock"];\n',
        'pub const INVOKERS: &[&str] = &["Payments"];\n',
    ),
    (
        "the clock's command is given one `Settled`",
        INVOKED,
        '        if invoker == "Payments" && !given_settled(unit, ws, decl) {\n',
        "        if !given_settled(unit, ws, decl) {\n",
    ),
    (
        "the clock runs one command",
        INVOKED,
        '        if invoker == "Payments"\n            && let Some(each) = all.get(invoker)\n',
        "        if true\n            && let Some(each) = all.get(invoker)\n",
    ),
]

TESTS = [
    "cargo", "test", "--locked", "-p", "pw-core",
    "--test", "scheduled", "--test", "invoked_by", "--test", "generality", "--test", "checking_source",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) for the tests."""
    import os
    p = subprocess.Popen(TESTS, cwd=ROOT, start_new_session=True,
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
    return True, sum(int(a) for a, _ in found), sum(int(b) for _, b in found)


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
