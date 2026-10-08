#!/usr/bin/env python3
"""Mutation controls for ADR-0271: a reader is told once per burst, and
served in turn.

Each mutant undoes one piece of the development server: the commits a
connection takes told one by one; a commit that reaches a reader being told
dropped, or not told after; a telling that never lets go, or that panics and
silences the reader; and the session's hold taken in no order, or a turn
that panics serving no one. The development server's tests must then fail.

Run from the repository root; `just e14-telling` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "the commits a connection takes are told one by one",
        SERVER,
        "        let others: std::collections::BTreeSet<String> = waiting\n",
        "        let others: Vec<String> = waiting\n",
    ),
    (
        "a commit that reaches a reader being told is dropped",
        SERVER,
        "                *again = true;\n",
        "",
    ),
    (
        "a commit during a telling is not told after it",
        SERVER,
        "                Some(again) if *again => *again = false,\n",
        "                Some(again) if *again && false => *again = false,\n",
    ),
    (
        "a telling ends without letting go of the reader",
        SERVER,
        "                    told.remove(other);\n                    unwound.ended = true;\n",
        "                    unwound.ended = true;\n",
    ),
    (
        "a telling that panics silences the reader",
        SERVER,
        "                if !self.ended {\n",
        "                if false {\n",
    ),
    (
        "a session's hold is taken in no order",
        SERVER,
        "        while tickets.1 != mine {\n",
        "        while false {\n",
    ),
    (
        "a turn that panics serves no one",
        SERVER,
        "        self.0.tickets.lock().expect(\"tickets\").1 += 1;\n",
        "        if !std::thread::panicking() {\n"
        "            self.0.tickets.lock().expect(\"tickets\").1 += 1;\n"
        "        }\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"]

# How long one run may take. Past it, it is killed with everything it
# started, and counts as failing: a turn served to no one waits for ever.
BOUND = 1200


def run_tests():
    """(built, passed, failed)."""
    p = subprocess.Popen(TESTS, cwd=ROOT, start_new_session=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not results:
        return False, 0, 0
    return True, sum(int(p) for p, _ in results), sum(int(f) for _, f in results)


def main():
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed", flush=True)
    if not built or failed or not passed:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
