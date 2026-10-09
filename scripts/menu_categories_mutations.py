#!/usr/bin/env python3
"""Mutation controls for ADR-0181: a menu grouped by its category, and a
loop inside a loop planned, rendered and patched where it is.

Each mutant undoes one piece:
- the plan: a loop inside a loop read as if its list were a query's; `*`
  not read as each item of a list;
- the renderer: a keyed list inside a row rendering the row again; an item
  new at the head inserted with no instance before it;
- the server: a list's rows inside each item of another not computed; the
  data layer putting each item in a category of its own, or store 48's
  bakery among its drinks.

An item out of place not moved, and a menu change derived from no
difference, are `patch_set_mutations.py`'s and `availability_mutations.py`'s,
re-anchored here by this ruling.

A plan mutant must fail `pw-core`'s tests, all of them run; a renderer
mutant, `pw-render`'s or the development server's; a server mutant, the
development server's.

Run from the repository root; `just e14-menu-categories` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
STORE_DATA = ROOT / "spikes/own-renderer/server/src/store.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a loop inside a loop is read as if its list were a query's",
        "core",
        PLAN,
        "    match outer.iter().rposition(|(b, _)| b == head) {\n",
        "    match outer.iter().rposition(|(b, _)| b == head).filter(|_| false) {\n",
    ),
    (
        "`*` is not read as each item of a list",
        "core",
        PLAN,
        '        if *field == "*" {\n',
        '        if false && *field == "*" {\n',
    ),
    (
        "a keyed list inside a row renders the row again",
        "render",
        RENDER,
        "            Part::Each {\n"
        "                id,\n"
        "                collection,\n"
        "                key: Some(_),\n"
        "                ..\n"
        "            } => {\n",
        "            Part::Each {\n"
        "                id,\n"
        "                collection,\n"
        "                key: Some(_),\n"
        "                ..\n"
        "            } if false => {\n",
    ),
    (
        "an item new at the head is inserted with no instance before it",
        "render",
        RENDER,
        "                        before: first.map(|(f, _)| f.clone()),\n",
        "                        before: first.map(|_| None).unwrap_or(None),\n",
    ),
    (
        "a list's rows inside each item of another are not computed",
        "server",
        SERVER,
        '        if let Some(at) = fields.iter().position(|f| *f == "*") {\n',
        '        if let Some(at) = fields.iter().position(|f| *f == "*").filter(|_| false) {\n',
    ),
    (
        "each item is a category of its own",
        "server",
        STORE_DATA,
        # Re-anchored by track store-pg: the store's operations, built once
        # over either layer's rows.
        "        match grouped.iter_mut().find(|(c, ..)| *c == item.category) {\n",
        "        match grouped.iter_mut().find(|(c, ..)| *c == item.category).filter(|_| false) {\n",
    ),
    (
        "store 48's bakery is among its drinks",
        "server",
        SERVER,
        '        (_, "scone") => ("bakery", "Bakery"),\n',
        "",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
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


def cargo_tests(suite):
    """(built, passed, failed) over the suite's tests."""
    out, code = bounded(CARGO[suite], cwd=ROOT)
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
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
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
            built, passed, failed = cargo_tests(suite)
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
