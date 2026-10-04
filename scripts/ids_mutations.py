#!/usr/bin/env python3
"""Mutation controls for ADR-0185: ids, and the ARIA that names them,
checked at build (charter §8.2).

Each mutant undoes one piece:
- PW5031: two elements of one id let through; an id in a loop taken as one
  row's; one id in two arms of a block taken as two elements;
- PW5032: a reference to nothing let through; one to an element shown only
  some of the time let through; an id in every arm of a block taken as not
  always shown; an id the page computes taken as no id, and one written
  with holes taken as any;
- PW5033: an unknown ARIA attribute, a value its attribute does not take,
  and an unknown role, each let through;
- one mistake, one report: a field named by a reference to nothing, and one
  a label misses for an id two elements have, each reported twice.

Every mutant must fail `pw-core`'s `tests/ids_and_aria.rs` or
`tests/labels.rs`.

Run from the repository root; `just e14-ids` records the output. The source
is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "two elements may share an id",
        CHECK,
        "            if let Some(&first) = together {\n",
        "            if let Some(&first) = together.filter(|_| false) {\n",
    ),
    (
        "an id in a loop names one row",
        CHECK,
        "            if place.repeated {\n",
        "            if false {\n",
    ),
    (
        "one id in two arms of a block is two elements",
        CHECK,
        "                    !matches!(\n"
        "                        (place.arms.get(same), other.arms.get(same)),\n"
        "                        (Some((x, _)), Some((y, _))) if x == y\n"
        "                    )\n",
        "                    same == same\n",
    ),
    (
        "a reference may name nothing",
        CHECK,
        "                    None if ids.may_be(token) => continue,\n",
        "                    None => continue,\n",
    ),
    (
        "a reference may name an element shown only some of the time",
        CHECK,
        "                        if place.scopes.iter().any(|s| always_shows(body, s, token)) {\n",
        "                        if true {\n",
    ),
    (
        "an id in every arm of a block is not always shown",
        CHECK,
        "            complete\n"
        "                && block_arms(body, children)\n",
        "            false\n"
        "                && complete\n"
        "                && block_arms(body, children)\n",
    ),
    (
        "an id the page computes is no id",
        CHECK,
        "        self.anything\n"
        "            || self.computed.iter().any(|(before, after)| {\n",
        "        false\n"
        "            && self.computed.iter().any(|(before, after)| {\n",
    ),
    (
        # A correction to ADR-0185, the same day: a string with holes was
        # taken as an id that may be anything.
        "an id written with holes may be anything",
        CHECK,
        "                    Expr::Interpolated { text, .. } => {\n",
        "                    Expr::Interpolated { text, .. } if false => {\n",
    ),
    (
        "an unknown ARIA attribute is let through",
        CHECK,
        "            let Some((_, kind)) = ARIA_ATTRIBUTES.iter().find(|(name, _)| *name == a.name) else {\n",
        "            let Some((_, kind)) = ARIA_ATTRIBUTES.iter().find(|(name, _)| *name == a.name).or(ARIA_ATTRIBUTES.first()) else {\n",
    ),
    (
        "a value its attribute does not take is let through",
        CHECK,
        "            if !fits {\n",
        "            if false {\n",
    ),
    (
        "an unknown role is let through",
        CHECK,
        "                    if !ARIA_ROLES.contains(&role) {\n",
        "                    if false {\n",
    ),
    (
        "a field named by a reference to nothing is reported twice",
        CHECK,
        "                    if labelled_by_nothing(body, &order, attrs) {\n",
        "                    if false && labelled_by_nothing(body, &order, attrs) {\n",
    ),
    (
        "a field a label misses for an id two have is reported twice",
        CHECK,
        "                    if labelled_but_for_its_id(body, &order, id, attrs) {\n",
        "                    if false && labelled_but_for_its_id(body, &order, id, attrs) {\n",
    ),
]

TESTS = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
    "--test", "ids_and_aria", "--test", "labels",
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the two test files."""
    p = subprocess.Popen(TESTS, cwd=ROOT, start_new_session=True, stdout=subprocess.PIPE,
                         stderr=subprocess.STDOUT, text=True)
    try:
        out, _ = p.communicate(timeout=BOUND)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.communicate()
        return True, 0, 1
    found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
    if not found:
        return False, 0, 0
    return True, sum(int(n) for n, _ in found), sum(int(f) for _, f in found)


def main():
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
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
