#!/usr/bin/env python3
"""Mutation controls for track `todomvc`'s two events (ADR-0367): a double
press (`on:dblclick`) and a control's losing focus (`on:blur`), which the
platform declares and the runtime listens for.

Each mutant undoes one piece:
- the platform (`events.pw`): an event not declared (so `on:dblclick` and
  `on:blur` are refused, PW5022), or declared with another event's record;
- the runtime: an event that listens for another (a double press for a
  click, a blur for a change), or whose record the listener does not read.

A platform mutant must fail `compiler/pw-core/tests/declared_events.rs` or
`events.rs`; a runtime mutant, `e2e/events.spec.mjs` in Chromium.

Run from the repository root; `just e14-dom-events` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EVENTS = ROOT / "packages/pw-platform-web/events.pw"
RUNTIME = ROOT / "spikes/own-renderer/public/pw-runtime.mjs"

# The platform mutants the checker refuses a program for, by the rule that is
# the protection each takes away (`scripts/mutant_programs.py`): TodoMVC's
# program names both events, its row's label `on:dblclick` and its edit
# field `on:blur`, so where one is not declared it is refused (PW5022), and
# where one is declared with a key's record its handler's is (PW0602).
CHECKED = {
    "a double press is not declared": {"PW5022"},
    "a blur is not declared": {"PW5022"},
    "a double press is given a key's record": {"PW0602"},
    "a blur is given a key's record": {"PW0602"},
}

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a double press is not declared",
        "core",
        EVENTS,
        "fn dblclick(event: PressEvent) -> () !{} { () }\n",
        "",
    ),
    (
        "a blur is not declared",
        "core",
        EVENTS,
        "fn blur(event: BlurEvent) -> () !{} { () }\n",
        "",
    ),
    (
        "a double press is given a key's record",
        "core",
        EVENTS,
        "fn dblclick(event: PressEvent) -> () !{} { () }\n",
        "fn dblclick(event: KeyEvent) -> () !{} { () }\n",
    ),
    (
        "a blur is given a key's record",
        "core",
        EVENTS,
        "fn blur(event: BlurEvent) -> () !{} { () }\n",
        "fn blur(event: KeyEvent) -> () !{} { () }\n",
    ),
    (
        "a double press listens for a click",
        "browser",
        RUNTIME,
        '  dblclick: "dblclick",\n',
        '  dblclick: "click",\n',
    ),
    (
        "a blur listens for a change",
        "browser",
        RUNTIME,
        '  blur: "blur",\n',
        '  blur: "change",\n',
    ),
    (
        "a double press's record is not read",
        "browser",
        RUNTIME,
        '    case "press":\n    case "dblclick":\n',
        '    case "press":\n',
    ),
    (
        "a blur's record is not read",
        "browser",
        RUNTIME,
        '    case "change":\n    case "blur":\n',
        '    case "change":\n',
    ),
]

CARGO = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "declared_events",
    "--test", "events", "--test", "platform_contracts",
]
SPEC = ["e2e/events.spec.mjs", "--project=chromium"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


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


def core_tests(_suite="core"):
    """(built, passed, failed) over the compiler's tests of the events."""
    out, code = bounded(CARGO, cwd=ROOT)
    if code is None:
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(p) for p, _ in found), sum(int(f) for _, f in found)


def browser_tests(_suite="browser"):
    """(built, passed, failed), after a build: the pages the demo serves
    beside the store, and the servers the suite starts."""
    built = subprocess.run(
        ["bash", "spikes/own-renderer/run.sh"],
        cwd=ROOT,
        env={**os.environ, "BUILD_ONLY": "1"},
        capture_output=True,
        text=True,
    )
    if built.returncode != 0:
        return False, 0, 0
    server = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "-p", "pw-dev-server", "-p", "kiokun-server"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if server.returncode != 0:
        return False, 0, 0
    out, code = bounded(
        ["pnpm", "exec", "playwright", "test", *SPEC, "--reporter=line"],
        cwd=ROOT / "spikes/own-renderer",
    )
    if code is None:
        return True, 0, 1
    out = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", out)
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", out))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", out))
    return passed + failed > 0, passed, failed


SUITES = {
    "core": core_tests,
    "browser": browser_tests,
}


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite, run in SUITES.items():
        built, passed, failed = run(suite)
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
            built, passed, failed = SUITES[suite](suite)
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
    # The pages and the server are built again from the restored source.
    browser_tests()

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
