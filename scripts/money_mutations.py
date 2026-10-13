#!/usr/bin/env python3
"""Mutation controls for ADR-0327: money is rounded only where a program
names how.

Each mutant undoes one piece of `domain`'s money: a rounding's tie or sign;
the rate's scale; `percent`'s; an exact operation's arithmetic; a negative
rate kept.

Each must fail `compiler/pw-conformance/tests/javascript.rs`'s
`money_is_rounded_as_the_program_names_it`, which compiles `domain` from
its source.

Run from the repository root; `just e14-money` records the output. The
source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DOMAIN = ROOT / "examples/domain.pw"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "half up rounds a half down",
        DOMAIN,
        "            HalfUp => if r * 2 > d { q + 1 } else if r * 2 < d { q } else if n >= 0 { q + 1 } else { q },\n",
        "            HalfUp => if r * 2 > d { q + 1 } else if r * 2 < d { q } else { q },\n",
    ),
    (
        "half even rounds a half up",
        DOMAIN,
        "            HalfEven => if r * 2 > d { q + 1 } else if r * 2 < d { q } else if q % 2 == 0 { q } else { q + 1 },\n",
        "            HalfEven => if r * 2 > d { q + 1 } else if r * 2 < d { q } else if n >= 0 { q + 1 } else { q },\n",
    ),
    (
        "toward zero floors a negative amount",
        DOMAIN,
        "            TowardZero => if n >= 0 { q } else { q + 1 },\n",
        "            TowardZero => q,\n",
    ),
    (
        "away from zero ceils a negative amount",
        DOMAIN,
        "            AwayFromZero => if n >= 0 { q + 1 } else { q },\n",
        "            AwayFromZero => q + 1,\n",
    ),
    (
        "a rate is read in hundred-thousandths",
        DOMAIN,
        "    Money(rounded(amount.value * rate.value, 1000000, rounding))\n",
        "    Money(rounded(amount.value * rate.value, 100000, rounding))\n",
    ),
    (
        "a percent is a thousand millionths",
        DOMAIN,
        "    millionths(p * 10000)\n",
        "    millionths(p * 1000)\n",
    ),
    (
        "a negative rate is kept",
        DOMAIN,
        "    if n >= 0 { Some(Rate(n)) } else { None }\n",
        "    Some(Rate(n))\n",
    ),
    (
        "an amount times a count is added to it",
        DOMAIN,
        "    Money(amount.value * count)\n",
        "    Money(amount.value + count)\n",
    ),
    (
        "a sum starts at a cent",
        DOMAIN,
        "    Money(List.fold(amounts, 0, (n, amount) => n + amount.value))\n",
        "    Money(List.fold(amounts, 1, (n, amount) => n + amount.value))\n",
    ),
    (
        "plus subtracts",
        DOMAIN,
        "    Money(a.value + b.value)\n",
        "    Money(a.value - b.value)\n",
    ),
    (
        "minus adds",
        DOMAIN,
        "    Money(a.value - b.value)\n",
        "    Money(a.value + b.value)\n",
    ),
]

CARGO = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript",
    "money_is_rounded_as_the_program_names_it",
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
