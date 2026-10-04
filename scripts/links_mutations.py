#!/usr/bin/env python3
"""Mutation controls for ADR-0189: a `<link>` is written where HTML allows it.

Each mutant undoes one piece of PW5035:
- the rule not run;
- a relation the head holds let through, or one compared by its case;
- a link with no relation and no item's property, one with both, one whose
  relation is blank, and one whose relation is computed in place, each let
  through;
- a body-ok relation, `stylesheet`, refused; an item's property refused.

Every mutant must fail `pw-core`'s `tests/links.rs`.

Run from the repository root; `just e14-links` records the output. The
source is restored after every mutant, whatever happens.
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
        "the rule is not run",
        CHECK,
        "        per_unit.extend(links(&u.hir));\n",
        "",
    ),
    (
        "a relation the head holds is let through",
        CHECK,
        "                            .find(|r| !BODY_OK.iter().any(|ok| ok.eq_ignore_ascii_case(r)))\n",
        "                            .find(|_| false)\n",
    ),
    (
        "relations are compared by their case",
        CHECK,
        "                            .find(|r| !BODY_OK.iter().any(|ok| ok.eq_ignore_ascii_case(r)))\n",
        "                            .find(|r| !BODY_OK.iter().any(|ok| *ok == **r))\n",
    ),
    (
        "a link with no relation and no item's property is let through",
        CHECK,
        "                (None, None) => Some(format!(\n"
        "                    \"`<link>` in `{}` names no relation and no item's property\",\n"
        "                    decl.name\n"
        "                )),\n",
        "                (None, None) => None,\n",
    ),
    (
        "a relation and an item's property at once are let through",
        CHECK,
        "                (Some(_), Some(_)) => Some(format!(\n"
        "                    \"`<link>` in `{}` is a relation and an item's property at once: HTML \\\n"
        "                     allows one of `rel` and `itemprop`\",\n"
        "                    decl.name\n"
        "                )),\n",
        "                (Some(_), Some(_)) => None,\n",
    ),
    (
        "a blank relation is let through",
        CHECK,
        "                            _ if relations.is_empty() => {\n",
        "                            _ if false => {\n",
    ),
    (
        "a relation computed in place is let through",
        CHECK,
        "                    _ => Some(format!(\n"
        "                        \"`<link rel>` in `{}` is a value computed in place, and which relation \\\n"
        "                         it names decides whether HTML allows it in the body\",\n"
        "                        decl.name\n"
        "                    )),\n",
        "                    _ => None,\n",
    ),
    (
        "a stylesheet is refused",
        CHECK,
        "        \"preload\",\n        \"stylesheet\",\n    ];\n",
        "        \"preload\",\n        \"no-stylesheet\",\n    ];\n",
    ),
    (
        "an item's property is refused",
        CHECK,
        "                (None, Some(_)) => None,\n",
        "                (None, Some(_)) => Some(String::from(\"an item's property\")),\n",
    ),
]

TESTS = ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "links"]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the test file."""
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
