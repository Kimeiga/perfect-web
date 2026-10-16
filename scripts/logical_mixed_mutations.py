#!/usr/bin/env python3
"""Mutation controls for ADR-0322: `|` and `&` are parenthesized where they
mix.

Each mutant undoes one piece of the parser's logical-operator diagnostics: a mixed chain parsed as
before, `&` bound tighter instead, the chain's first operator forgotten, and
a chain refused at every switch rather than once, and doubled operators
without spelling advice or separated operators given that advice.

Each must fail one of `compiler/pw-syntax/src/grammar.rs`'s tests.
The whole syntax unit suite runs for every control.

Run from the repository root; `just e14-logical-mixed` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "doubled logical operators carry no spelling advice",
        GRAMMAR,
        "                if doubled_logical {\n",
        "                if doubled_logical && false {\n",
    ),
    (
        "separated logical operators are called doubled",
        GRAMMAR,
        "                if doubled_logical {\n",
        "                if matches!(current.kind, Kind::Amp | Kind::Pipe) {\n",
    ),
    (
        "a chain that mixes `|` and `&` parses as before",
        GRAMMAR,
        "                    Some((first, false)) if first != k => {\n",
        "                    Some((first, false)) if false && first != k => {\n",
    ),
    (
        "`&` binds tighter than `|`",
        GRAMMAR,
        "            Kind::Amp => 3,\n",
        "            Kind::Amp => 4,\n",
    ),
    (
        "a chain's first operator is forgotten",
        GRAMMAR,
        "                    None => logical = Some((k, false)),\n",
        "                    None => {}\n",
    ),
    (
        "a chain is refused at every switch",
        GRAMMAR,
        "                        logical = Some((first, true));\n",
        "",
    ),
]

CARGO = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-syntax", "--lib",
]

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


def cargo_tests():
    """(built, passed, failed) over the tests."""
    out, code = bounded(CARGO, cwd=ROOT)
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
    built, passed, failed = cargo_tests()
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
            built, passed, failed = cargo_tests()
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
