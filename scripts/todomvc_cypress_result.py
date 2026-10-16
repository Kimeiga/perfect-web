#!/usr/bin/env python3
"""**What TodoMVC's own Cypress suite says of TodoMVC in Pleris** (track `todomvc`, W7).

`spikes/todomvc-cypress/run.sh` runs the suite, unchanged, against the app and
gives Cypress's output to this script, which writes the evidence
`just e14-todomvc` records and judges it:

    python3 scripts/todomvc_cypress_result.py <acceptance|diagnostic> <output> [exit status]

**A run that did not happen is not a pass.** The verdict is read from what
Cypress printed: its table must count every test the spec has, and each test
must satisfy the acceptance requirement (or explicitly separate diagnostic). A missing binary, a missing
Xvfb, a host that never started, an empty file: none has a results table, and
each fails here, whatever the exit status of the shell around it.

**The acceptance** requires every official test to execute and pass,
including storage checks, with Cypress exit status zero. A failed or skipped
test remains an unmet requirement even when its cause is understood.

**The diagnostic** runs the same files under a name the suite's own tables take
as keeping nothing in `localStorage` (`puremvc`), so each test shows what the
rest of it says of this app. It is not the acceptance. Every test must pass,
bar the persistence test, which the suite skips for such a name, and one test
that races the host (`RACY`).

The tests, and which of them ask `localStorage`, are read from the vendored
spec (`spikes/todomvc-cypress/cypress/e2e/spec.cy.js`, held to upstream's bytes
by `todomvc_cypress_vendored.py`), not written out here.

Exit status 0 only when the selected mode's requirements hold; 1 otherwise. The report goes to the
standard output either way.
"""

import pathlib
import re
import sys
from collections import namedtuple

sys.dont_write_bytecode = True

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPEC = ROOT / "spikes/todomvc-cypress/cypress/e2e/spec.cy.js"
EXPECTED_TESTS = 29
MODES = ("acceptance", "diagnostic")

# What each of the spec's helpers asks of `localStorage`.
CHECKS = {
    "checkItemSaved": "expects the app to have called localStorage.setItem",
    "checkTodosInLocalStorage": "waits for a todo's text to be in localStorage",
    "checkNumberOfTodosInLocalStorage": "waits for localStorage to hold n todos",
    "checkNumberOfCompletedTodosInLocalStorage": "waits for localStorage to hold n completed todos",
    "checkCompletedKeywordInLocalStorage": "waits for 'complete' to be in localStorage",
}

# The persistence test skips itself for a name in `noLocalStorageCheck`.
PERSISTENCE = "should persist its data"

# The one test whose outcome depends on timing, not on the app's design: it
# presses toggle-all and, at once, unchecks the first row's box. This host
# checks that box when its answer reaches the page (the stream is read every
# 25 ms, so tens of milliseconds after the press); when the test's next step
# comes first, `uncheck` finds the box unchecked and does nothing, and
# toggle-all stays checked as every row completes. It fails when the host is
# loaded and passes when it is not (ADR section on the suite's result).
RACY = {
    "complete all checkbox should update state when items are completed / cleared": (
        "expected '<input#toggle-all.toggle-all>' not to be 'checked'",
        "by timing, not by design: the test unchecks the first row's box the moment it has pressed "
        "toggle-all, and the host checks that box when its answer reaches the page. Where the test is "
        "first, its uncheck finds the box unchecked and does nothing, and toggle-all stays checked as "
        "every row completes. It passes on a quiet machine and fails on a loaded one",
    )
}

SECURITY = "Uncaught SecurityError: Failed to read the 'length' property from 'Storage'"

ACCEPTANCE_NOTE = [
    "Acceptance requires all 29 official Cypress tests to execute and pass, with exit status zero.",
    "Storage, timing and skipped-test mismatches remain unmet requirements even when understood.",
]
DIAGNOSTIC_NOTE = [
    "The diagnostic, NOT the acceptance: the same files under a name the suite's own tables take as keeping nothing in",
    "localStorage (--env framework=puremvc: in noLocalStorageCheck, with no known issues), reaching the page through an",
    "address fragment the page ignores. It shows what the rest of each test says of this app.",
]

Test = namedtuple("Test", "n context title line direct hooked")
State = namedtuple("State", "state error")

ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
CONTEXT = re.compile(r"""^[ \t]*context\(\s*(['"`])(.*?)\1""", re.M)
IT = re.compile(r"""^[ \t]*it\(\s*(['"`])(.*?)\1""", re.M)
BEFORE = re.compile(r"^[ \t]*beforeEach\(", re.M)
AFTER = re.compile(r"^[ \t]*afterEach\(", re.M)

RUNNING = re.compile(r"^\s+Running:\s+\S+")
TOTALS = re.compile(r"^\s+\d+\s+(passing|failing|pending)\b|^\s+\(Results\)")
PASS = re.compile(r"^\s+[✓✔]\s+(?P<t>.+?)(?:\s+\(\d+ms\))?\s*$")
FAIL = re.compile(r"^\s+(?P<n>\d+)\)\s+(?P<t>.+?)\s*$")
PENDING = re.compile(r"^\s+-\s+(?P<t>.+?)\s*$")
HOOK = re.compile(r'^"(?:before|after) (?:each|all)" hook for "(?P<t>.*)"$')
BLOCK = re.compile(r"^  (?P<n>\d+)\) ")
ERROR = re.compile(r"^\s+(?P<e>[A-Za-z]*Error: .*?)\s*$")
TABLE = re.compile(r"^\s*│\s*(?P<k>Tests|Passing|Failing|Pending|Skipped):\s+(?P<v>\d+)\s*│")
VERSION = re.compile(r"^\s*│\s*Cypress:\s+(?P<v>\S+)")
BROWSER = re.compile(r"^\s*│\s*Browser:\s+(?P<v>.*?)\s*│")
NOISE = re.compile(r"ERROR:dbus|WARNING:|DevTools listening|gpu_memory_buffer|^\s*$")


def spec_tests(text):
    """The spec's tests in order, with the `localStorage` checks each makes
    in its own body and in its context's `beforeEach` hooks."""
    events = []
    for m in CONTEXT.finditer(text):
        events.append((m.start(), "context", m.group(2)))
    for m in IT.finditer(text):
        events.append((m.start(), "it", m.group(2)))
    for m in BEFORE.finditer(text):
        events.append((m.start(), "before", None))
    for m in AFTER.finditer(text):
        events.append((m.start(), "after", None))
    events.sort()
    tests, context, hooked = [], None, set()
    for i, (pos, kind, name) in enumerate(events):
        end = events[i + 1][0] if i + 1 < len(events) else len(text)
        body = text[pos:end]
        found = {c for c in CHECKS if re.search(r"\b" + c + r"\(", body)}
        if kind == "context":
            context, hooked = name, set()
        elif kind == "before" and context is not None:
            hooked |= found
        elif kind == "it":
            line = text.count("\n", 0, pos) + 1
            tests.append(Test(len(tests) + 1, context, name, line, frozenset(found), frozenset(hooked)))
    return tests


def asks(test):
    return bool(test.direct or test.hooked)


def asks_text(test):
    """The helpers a test asks `localStorage` through, by name; a test whose
    context's `beforeEach` asks it too says so."""
    text = ", ".join(sorted(test.direct | test.hooked))
    return text + " (its context's beforeEach asks it too)" if test.hooked and test.direct else (
        text + " (in its context's beforeEach)" if test.hooked else text
    )


def parse_run(text):
    """What Cypress's spec reporter printed: each test's line, each failure's
    first error line, the table of counts, and whether the browser logged the
    `SecurityError` of a polling loop reading a cleared `Storage`."""
    lines = ANSI.sub("", text).replace("\r", "").split("\n")
    run = {
        "cypress": None,
        "browser": None,
        "passed": [],
        "pending": [],
        "failed": [],  # (number, title)
        "errors": {},  # number -> first error line
        "counts": {},
        "storage_denied": any(SECURITY in line for line in lines),
        "lines": lines,
    }
    for line in lines:
        m = VERSION.match(line)
        if m and run["cypress"] is None:
            run["cypress"] = m.group("v")
        m = BROWSER.match(line)
        if m and run["browser"] is None:
            run["browser"] = m.group("v")
        m = TABLE.match(line)
        if m and m.group("k") not in run["counts"]:
            run["counts"][m.group("k")] = int(m.group("v"))
    start = next((i for i, line in enumerate(lines) if RUNNING.match(line)), None)
    if start is None:
        return run
    end = next((i for i in range(start + 1, len(lines)) if TOTALS.match(lines[i])), len(lines))
    for line in lines[start + 1 : end]:
        for pattern, key in ((PASS, "passed"), (PENDING, "pending")):
            m = pattern.match(line)
            if m:
                run[key].append(m.group("t"))
                break
        else:
            m = FAIL.match(line)
            if m:
                run["failed"].append((int(m.group("n")), m.group("t")))
    number = None
    for line in lines[end:]:
        m = BLOCK.match(line)
        if m:
            number = int(m.group("n"))
            continue
        if number is not None and number not in run["errors"]:
            m = ERROR.match(line)
            if m:
                run["errors"][number] = m.group("e")
    return run


def states(tests, run, problems):
    """Each test's state: pass, fail, hook (its hook failed), pending, skip (not run)."""
    by_title = {t.title: t for t in tests}
    out = {t.n: State("skip", None) for t in tests}

    def mark(title, state, error=None):
        t = by_title.get(title)
        if t is None:
            problems.append(f"the output names a test the spec does not have: {title!r}")
        else:
            out[t.n] = State(state, error)

    for title in run["passed"]:
        mark(title, "pass")
    for title in run["pending"]:
        mark(title, "pending")
    for number, title in run["failed"]:
        error = run["errors"].get(number)
        hook = HOOK.match(title)
        if hook:
            mark(hook.group("t"), "hook", error)
        else:
            mark(title, "fail", error)
    return out


def name(test):
    return f"test {test.n} ({test.title!r})"


def judge_acceptance(tests, st, run, problems):
    for t in tests:
        state = st[t.n].state
        if state != "pass":
            problems.append(f"{name(t)} is {state}: acceptance requires every official test to pass")


def judge_diagnostic(tests, st, run, problems):
    for t in tests:
        state, error = st[t.n]
        if t.title == PERSISTENCE:
            if state != "pending":
                problems.append(f"{name(t)} should be skipped by the suite for this name, and is {state}")
        elif t.title in RACY:
            if state == "fail" and RACY[t.title][0] not in (error or ""):
                problems.append(f"{name(t)} failed for another reason than its race: {error}")
            elif state not in ("pass", "fail"):
                problems.append(f"{name(t)} is {state}: it passes or loses its race")
        elif state != "pass":
            problems.append(f"{name(t)} is {state}, and every test but two should pass: {error}")


def judge(mode, tests, run, exit_status=None):
    """(states, problems). States are None where the run has no results to read."""
    problems = []
    if len(tests) != EXPECTED_TESTS:
        problems.append(f"the spec has {len(tests)} tests, not the {EXPECTED_TESTS} this was written for")
    counts = run["counts"]
    if "Tests" not in counts or run["cypress"] is None:
        problems.append("Cypress did not run the suite: its output has no results table")
        return None, problems
    st = states(tests, run, problems)
    got = {
        "Tests": len(tests),
        "Passing": len(run["passed"]),
        "Failing": len(run["failed"]),
        "Pending": len(run["pending"]),
        "Skipped": sum(1 for t in tests if st[t.n].state == "skip"),
    }
    for key, lines in got.items():
        if counts.get(key) != lines:
            problems.append(f"Cypress's table says {key}: {counts.get(key)}, its lines and the spec say {lines}")
    if exit_status is not None and exit_status != counts.get("Failing"):
        problems.append(f"Cypress exited with status {exit_status}, and its table counts {counts.get('Failing')} failing")
    if mode == "acceptance" and exit_status != 0:
        problems.append("acceptance requires Cypress exit status 0, explicitly recorded")
    (judge_acceptance if mode == "acceptance" else judge_diagnostic)(tests, st, run, problems)
    return st, problems


def why(mode, test, state, error):
    """The lines under a test that did not pass: the spec rule's outcome and why."""
    out = []
    if state == "fail" and mode == "acceptance":
        out.append(f"unmet official requirement: {asks_text(test)}" if asks(test) else "the official test failed")
    elif state == "fail" and test.title in RACY:
        out.append(RACY[test.title][1])
    elif state == "hook":
        out.append(
            "its beforeEach failed: the polling loops of the failed tests before it read a localStorage that "
            "Cypress had cleared away, which throws in the spec's own window and is charged to this hook; "
            "Cypress then skips the rest of the suite"
        )
    elif state == "skip":
        if asks(test):
            out.append(f"not run (a failed hook skipped it); it asks localStorage ({asks_text(test)}): it would fail there, by design")
        else:
            out.append("not run (a failed hook skipped it); it asks nothing of localStorage: the diagnostic runs it")
    elif state == "pending":
        out.append("skipped by the suite itself for this name (it reloads the page and asks localStorage)")
    if error and state in ("fail", "hook"):
        out.append(f"cypress: {error}")
    return out


def report(mode, tests, run, st, problems, exit_status=None):
    lines = []
    lines += ACCEPTANCE_NOTE if mode == "acceptance" else DIAGNOSTIC_NOTE
    lines.append("")
    if st is None:
        lines.append(f"{mode}: the suite did not run: Cypress printed no results table")
        shown = [line for line in run["lines"] if not NOISE.search(line)]
        lines += ["  | " + line.rstrip() for line in shown[-12:]]
    else:
        n = {s: sum(1 for t in tests if st[t.n].state == s) for s in ("pass", "fail", "hook", "pending", "skip")}
        parts = [f"{n['pass']} of {len(tests)} passed"]
        if n["fail"] + n["hook"]:
            detail = f" ({n['fail']} tests, {n['hook']} hook)" if n["hook"] else ""
            parts.append(f"{n['fail'] + n['hook']} failed{detail}")
        if n["pending"]:
            parts.append(f"{n['pending']} pending")
        if n["skip"]:
            parts.append(f"{n['skip']} not run")
        lines.append(f"{mode}: " + "; ".join(parts))
        lines.append(f"cypress {run['cypress']} on {run['browser']}; exit status {exit_status}")
        lines.append("")
        label = {"pass": "pass", "fail": "FAIL", "hook": "HOOK", "pending": "pending", "skip": "not run"}
        for t in tests:
            state, error = st[t.n]
            lines.append(f"{t.n:3d}  {label[state]:7s} [{t.context}] {t.title}  (spec.cy.js:{t.line})")
            for line in why(mode, t, state, error):
                lines.append(f"{'':13s}{line}")
        if mode == "acceptance" and run["storage_denied"]:
            lines.append("")
            lines.append(f"recorded by the browser: {SECURITY} ...")
    lines.append("")
    if problems:
        lines += [f"PROBLEM: {p}" for p in problems]
        if st is None:
            lines.append(f"{mode}: FAILED: the suite did not run, and a suite that did not run is not a pass")
        else:
            lines.append(f"{mode}: FAILED: {len(problems)} problem(s): the required test outcomes did not pass")
    else:
        lines.append(f"{mode}: the required test outcomes passed")
    return "\n".join(lines)


def main(argv):
    if len(argv) < 3 or argv[1] not in MODES:
        print(__doc__.split("\n\n")[0])
        print("usage: todomvc_cypress_result.py <acceptance|diagnostic> <cypress output> [exit status]")
        return 2
    mode, path = argv[1], pathlib.Path(argv[2])
    exit_status = int(argv[3]) if len(argv) > 3 else None
    tests = spec_tests(SPEC.read_text(encoding="utf-8"))
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as why_not:
        print(f"{mode}: Cypress's output is not there: {why_not}")
        return 1
    run = parse_run(text)
    st, problems = judge(mode, tests, run, exit_status)
    print(report(mode, tests, run, st, problems, exit_status))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
