#!/usr/bin/env python3
"""Mutation controls for ADR-0282: what a value holds is one label.

Each mutant undoes one piece:
- a secret a declaration answers held, and a secret it only uses not (the
  control's side: every secret held);
- what a value holds read whole by what keeps it (PW5101);
- what a declaration reads answering it a secret, where it runs;
- a materialization reading what it depends on;
- placement reading what a declaration holds, in the checker and in the
  contract;
- a placement refusal naming the label that rules each world out.

The tests of each must then fail: `held_where_it_runs.rs`, and
`reads_through_calls.rs`, whose control holds that a secret used as a key
makes nothing held.

Run from the repository root; `just e14-held-labels` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a secret a declaration answers is not held",
        CHECK,
        "            if !secret.is_public() {\n",
        "            if false {\n",
    ),
    (
        "every secret is left out of what a value holds, as before",
        CHECK,
        "        without_secrets(&self.observed(def)).join(&answered)\n",
        "        without_secrets(&self.observed(def))\n",
    ),
    (
        "every secret is held, one only used as a key among them",
        CHECK,
        "        without_secrets(&self.observed(def)).join(&answered)\n",
        "        self.observed(def).join(&answered)\n",
    ),
    (
        "what a declaration reads answers it no secret",
        CHECK,
        "                held = held.join(a);\n",
        "                let _ = a;\n",
    ),
    (
        "a materialization reads nothing it depends on",
        CHECK,
        "                    read.extend(depends_on(decl, inference, unit));\n",
        "",
    ),
    (
        "placement reads the keywords alone",
        CHECK,
        "    let label = label.join(&reads.holds(def));\n",
        "    let label = label;\n",
    ),
    (
        "the contract reads the keywords alone",
        CONTRACT,
        "                    .join(&reads.holds(def)),\n",
        ",\n",
    ),
    (
        "a refusal names no label",
        CHECK,
        '        (false, false) => format!("it holds {}", held.join(", ")),\n',
        '        (false, false) => format!("it requires {}", written.join(", ")),\n',
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
    "--test", "held_where_it_runs", "--test", "reads_through_calls",
]

# How long one command may run. Past it, it and what it started are stopped.
BOUND = 1800


def run_tests():
    """(built, passed, failed)."""
    p = subprocess.Popen(
        TESTS, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        start_new_session=True,
    )
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        out, _ = p.communicate()
    results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not results:
        return False, 0, 0
    return True, sum(int(a) for a, _ in results), sum(int(b) for _, b in results)


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    # Stopped from outside, the source is still restored: a stop is an
    # exception here, which the `finally` below meets.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
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
