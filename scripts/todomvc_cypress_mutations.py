#!/usr/bin/env python3
"""Mutation controls for track `todomvc`'s Cypress acceptance (ADR-XXXX): the
official suite is vendored unchanged, a run is judged by what Cypress printed,
and a suite that cannot run fails and is never skipped.

A test that passes with a check removed is not evidence for the check. Each
mutant undoes one thing:
- `todomvc_cypress_vendored.py` holds each vendored file to upstream's blob;
- `todomvc_cypress_result.py` reads Cypress's output and judges each test's
  outcome (what the spec asks of `localStorage`, what the app's design gives,
  what a hook that fails makes of the rest, the one test that races the host);
- `run.sh` runs the suite, and fails where it cannot (no Xvfb, no binary, no
  build, no host) and where the verdict fails.

At least one test of `scripts/tests/test_todomvc_cypress.py` must then fail.
Its fixtures are real outputs of Cypress 15.14.2 and 15.21.1 against this app, and `run.sh`
is run whole there with a Cypress that prints what the test says.

Run from the repository root; `just e14-todomvc` records the output. The
sources are restored after every mutant, whatever happens.
"""

import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VENDORED = ROOT / "scripts/todomvc_cypress_vendored.py"
RESULT = ROOT / "scripts/todomvc_cypress_result.py"
RUN = ROOT / "spikes/todomvc-cypress/run.sh"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    # -- the vendored files are upstream's
    (
        "a blob is hashed without its header, so it is not git's",
        VENDORED,
        r'''    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()''',
        "    return hashlib.sha1(data).hexdigest()",
    ),
    (
        "a vendored file that is gone is not a problem",
        VENDORED,
        "        if not file.exists():\n",
        "        if False:\n",
    ),
    (
        "a vendored file that differs from upstream's is not a problem",
        VENDORED,
        "        if got != want:\n",
        "        if False:\n",
    ),
    (
        "another Cypress than upstream's lockfile pins is not a problem",
        VENDORED,
        '    if pinned.get("cypress") != CYPRESS:\n',
        "    if False:\n",
    ),
    (
        "a suite that is not upstream's exits 0",
        VENDORED,
        '''        print(f"todomvc-cypress: {len(found)} problem(s): the suite is not upstream's")\n        return 1\n''',
        '''        print(f"todomvc-cypress: {len(found)} problem(s): the suite is not upstream's")\n        return 0\n''',
    ),
    # -- what the spec asks of localStorage is read from the spec
    (
        "a context's beforeEach asks nothing of localStorage",
        RESULT,
        "            hooked |= found\n",
        "            pass\n",
    ),
    (
        "a test asks localStorage only in its own body",
        RESULT,
        "    return bool(test.direct or test.hooked)\n",
        "    return bool(test.direct)\n",
    ),
    # -- Cypress's output is read as it is
    (
        "colour codes are part of a line",
        RESULT,
        r'''    lines = ANSI.sub("", text).replace("\r", "").split("\n")''',
        r'''    lines = text.replace("\r", "").split("\n")''',
    ),
    (
        "a pending test is not read",
        RESULT,
        '        for pattern, key in ((PASS, "passed"), (PENDING, "pending")):\n',
        '        for pattern, key in ((PASS, "passed"),):\n',
    ),
    (
        "the table of counts is not read",
        RESULT,
        "(?P<k>Tests|Passing|Failing|Pending|Skipped)",
        "(?P<k>Test|Passing|Failing|Pending|Skipped)",
    ),
    (
        "the browser's record of the polling loop's throw is not read",
        RESULT,
        '''SECURITY = "Uncaught SecurityError: Failed to read the 'length' property from 'Storage'"''',
        '''SECURITY = "Uncaught SecurityError: Failed to read the 'size' property from 'Storage'"''',
    ),
    (
        "a failed hook is taken for a failed test of its own name",
        RESULT,
        "        hook = HOOK.match(title)\n",
        "        hook = None\n",
    ),
    # -- a suite that did not run is not a pass
    (
        "output with no results table is read as results",
        RESULT,
        '    if "Tests" not in counts or run["cypress"] is None:\n',
        "    if False:\n",
    ),
    (
        "the spec's number of tests is not held",
        RESULT,
        "    if len(tests) != EXPECTED_TESTS:\n",
        "    if False:\n",
    ),
    (
        "Cypress's table is not held to its own lines",
        RESULT,
        "        if counts.get(key) != lines:\n",
        "        if False:\n",
    ),
    (
        "a failing count one too many is not seen",
        RESULT,
        '        "Failing": len(run["failed"]),\n',
        '        "Failing": len(run["failed"]) + 1,\n',
    ),
    (
        "Cypress's exit status is not held to its failing count",
        RESULT,
        '    if exit_status is not None and exit_status != counts.get("Failing"):\n',
        "    if False:\n",
    ),
    (
        "a test the spec does not have is let by",
        RESULT,
        '            problems.append(f"the output names a test the spec does not have: {title!r}")\n',
        "            pass\n",
    ),
    # -- the acceptance: an app that never writes localStorage
    (
        "acceptance: a test that asks localStorage may pass",
        RESULT,
        '        if state == "pass" and asks(t):\n',
        "        if False:\n",
    ),
    (
        "acceptance: a test that asks nothing of localStorage may fail",
        RESULT,
        '        elif state == "fail" and not asks(t):\n',
        "        elif False:\n",
    ),
    (
        "acceptance: a pending test is let by",
        RESULT,
        '''            problems.append(f"{name(t)} is pending: this name is in no table of the suite, so nothing is skipped")\n''',
        "            pass\n",
    ),
    (
        "acceptance: a test not run needs no failed hook before it",
        RESULT,
        '        elif state == "skip" and not failed_hook:\n',
        "        elif False:\n",
    ),
    (
        "acceptance: a failed hook needs no cause on record",
        RESULT,
        '    if failed_hook and not run["storage_denied"]:\n',
        "    if False:\n",
    ),
    (
        "acceptance: the first tests need not pass",
        RESULT,
        '        if st[t.n].state != "pass" and not asks(t):\n',
        "        if False:\n",
    ),
    # -- the diagnostic: the suite as an app that keeps nothing in localStorage
    (
        "diagnostic: the persistence test need not be skipped by the suite",
        RESULT,
        '            if state != "pending":\n',
        "            if False:\n",
    ),
    (
        "diagnostic: the test that races the host is not known",
        RESULT,
        "        elif t.title in RACY:\n",
        "        elif False:\n",
    ),
    (
        "diagnostic: the test that races the host may fail for any reason",
        RESULT,
        '            if state == "fail" and RACY[t.title][0] not in (error or ""):\n',
        "            if False:\n",
    ),
    (
        "diagnostic: the test that races the host may be skipped",
        RESULT,
        '            elif state not in ("pass", "fail"):\n',
        "            elif False:\n",
    ),
    (
        "diagnostic: no other test need pass",
        RESULT,
        '        elif state != "pass":\n',
        "        elif False:\n",
    ),
    (
        "both modes are judged as the diagnostic",
        RESULT,
        '    (judge_acceptance if mode == "acceptance" else judge_diagnostic)(tests, st, run, problems)\n',
        "    judge_diagnostic(tests, st, run, problems)\n",
    ),
    # -- the report and the exit status
    (
        "a run with problems exits 0",
        RESULT,
        "    return 1 if problems else 0\n",
        "    return 0\n",
    ),
    (
        "output that is not there exits 0",
        RESULT,
        '''        print(f"{mode}: Cypress's output is not there: {why_not}")\n        return 1\n''',
        '''        print(f"{mode}: Cypress's output is not there: {why_not}")\n        return 0\n''',
    ),
    (
        "a report of problems does not say it failed",
        RESULT,
        '''            lines.append(f"{mode}: FAILED: {len(problems)} problem(s): the outcome is not the one the app's design gives")\n''',
        '''            lines.append(f"{mode}: the outcome is the one the app's design gives")\n''',
    ),
    (
        "a report of a suite that did not run does not say so",
        RESULT,
        '''            lines.append(f"{mode}: FAILED: the suite did not run, and a suite that did not run is not a pass")\n''',
        '''            lines.append(f"{mode}: no results")\n''',
    ),
    # -- run.sh fails where Cypress cannot run, and with the verdict
    (
        "run.sh does not look for Xvfb",
        RUN,
        '''  || cannot "Xvfb is not installed (Cypress needs a display and starts Xvfb itself; apt: xvfb)"\n''',
        "  || true\n",
    ),
    (
        "run.sh runs a Cypress whose binary is not installed",
        RUN,
        '''  || cannot "its binary is not installed, or does not start (pnpm exec cypress install, in spikes/todomvc-cypress; UPSTREAM.md says what it fetches)"\n''',
        "  || true\n",
    ),
    (
        "run.sh runs without a build of the app",
        RUN,
        '''[ -d "$DIST/build" ] || cannot "$DIST is not built (bash spikes/own-renderer/todomvc.sh)"\n''',
        "true\n",
    ),
    (
        "run.sh does not look for the host's binary",
        RUN,
        '''[ -x "$SERVER" ] || cannot "the host $SERVER is not built (cargo build -p pw-dev-server)"\n''',
        "true\n",
    ),
    (
        "run.sh's cannot-run exits 0",
        RUN,
        '  echo "run.sh: Cypress cannot run: $*"\n  exit 1\n',
        '  echo "run.sh: Cypress cannot run: $*"\n  exit 0\n',
    ),
    (
        "run.sh's own status is not the verdict's",
        RUN,
        'python3 "$REPO_ROOT/scripts/todomvc_cypress_result.py" "$MODE" "$OUT" "$CODE"\n',
        'python3 "$REPO_ROOT/scripts/todomvc_cypress_result.py" "$MODE" "$OUT" "$CODE" || true\n',
    ),
    (
        "run.sh judges the diagnostic as the acceptance",
        RUN,
        '"$MODE" "$OUT" "$CODE"\n',
        'acceptance "$OUT" "$CODE"\n',
    ),
    (
        "run.sh does not tell the verdict what Cypress exited with",
        RUN,
        '"$MODE" "$OUT" "$CODE"\n',
        '"$MODE" "$OUT"\n',
    ),
    (
        "run.sh leaves the browser's console off, where a failed hook says why",
        RUN,
        "env -u DISPLAY -u WAYLAND_DISPLAY ELECTRON_ENABLE_LOGGING=1 \\\n",
        "env -u DISPLAY -u WAYLAND_DISPLAY \\\n",
    ),
    (
        "run.sh leaves the machine's display in Cypress's environment",
        RUN,
        "env -u DISPLAY -u WAYLAND_DISPLAY ELECTRON_ENABLE_LOGGING=1 \\\n",
        "env -u WAYLAND_DISPLAY ELECTRON_ENABLE_LOGGING=1 \\\n",
    ),
    (
        "run.sh names the acceptance's app as the diagnostic's",
        RUN,
        '  FRAMEWORK="page/todomvc.app.Todos"\n',
        '  FRAMEWORK="puremvc"\n',
    ),
    (
        "run.sh's diagnostic reaches the page without its fragment",
        RUN,
        '  BASE="http://127.0.0.1:$PORT/page/todomvc.app.Todos#"\n',
        '  BASE="http://127.0.0.1:$PORT/page/todomvc.app.Todos"\n',
    ),
]

TESTS = [[sys.executable, "-m", "unittest", "scripts/tests/test_todomvc_cypress.py"]]

# How long one command may run. Past the bound it is killed and counts as failing.
BOUND = 600


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        try:
            r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=BOUND)
        except subprocess.TimeoutExpired:
            failed += 1
            continue
        out = r.stdout + r.stderr
        ran = re.search(r"^Ran (\d+) tests?", out, re.M)
        if ran is None:
            built = False
            continue
        bad = re.search(r"^FAILED \((.*)\)", out, re.M)
        n_bad = sum(int(n) for n in re.findall(r"=(\d+)", bad.group(1))) if bad else 0
        passed += int(ran.group(1)) - n_bad
        failed += n_bad
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline

    # Stopped from outside, the sources are still restored: SIGTERM raises
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
            verdict = "KILLED (does not run)"
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
