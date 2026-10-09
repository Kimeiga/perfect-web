#!/usr/bin/env python3
"""Mutation controls for ADR-0297: a commit tells the pages that ask, and
tells them at once.

Each mutant undoes one piece: the other sessions told one after another; a
document no page asks for derived all the same, or every document taken for
one a page asks for; a document passed by not told when its page asks, by
its route or by its derivation; the asking page left as it was, so its
telling passes it by again.

Each must fail the development server's tests of it.

Run from the repository root; `just e14-tell-at-once` records the output.
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

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "the other sessions are told one after another",
        "host",
        SERVER,
        "        let tellers = TELLERS.min(others.len());\n",
        "        let tellers = 1;\n",
    ),
    (
        "a document no page asks for is derived all the same",
        "host",
        SERVER,
        "                    Some(waiting) if waiting.live(now) => true,\n",
        "                    Some(waiting) if waiting.live(now) || true => true,\n",
    ),
    (
        "every document is one a page asks for",
        "host",
        SERVER,
        "        now.saturating_duration_since(self.seen) < LIVE\n",
        "        now.saturating_duration_since(self.seen) < LIVE || true\n",
    ),
    (
        "a page that asks is not told what it missed",
        "host",
        SERVER,
        "    server.told_as_it_asks(&doc);\n",
        "",
    ),
    (
        "a document passed by is not derived when its page asks",
        "host",
        SERVER,
        "        if !stale {\n            return;\n        }\n",
        "        if !stale || true {\n            return;\n        }\n",
    ),
    (
        "the asking page is left as it was, and passed by again",
        "host",
        SERVER,
        "                waiting.seen = std::time::Instant::now();\n"
        "                std::mem::take(&mut waiting.stale)\n",
        "                std::mem::take(&mut waiting.stale)\n",
    ),
]

COMMANDS = {
    "host": [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server", "--",
        "a_reader_is_told_while", "a_document_no_page_asks_for",
        "a_telling_that_panics", "a_burst",
    ],
}

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


def tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(COMMANDS[suite], cwd=ROOT)
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
    for suite in COMMANDS:
        built, passed, failed = tests(suite)
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
            built, passed, failed = tests(suite)
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
