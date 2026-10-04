#!/usr/bin/env python3
"""Mutation controls for ADR-0170: a loop over a list inside a query's
value, and a part a speculation would not reach.

Each mutant undoes one piece:
- the plan: a list recorded by its binding, not its path; a row's member
  read over a list inside a value not planned;
- the speculation module: a part it would not reach not refused;
- the development server: a list read from the binding, not its path; a
  row read applied only to a list that is the binding itself;
- the renderer: a value computed for a row not read whole.

A plan or speculation mutant must fail `pw-core`'s tests, all of them run;
a server mutant, the server's; a renderer mutant, `pw-render`'s.

Run from the repository root; `just e14-nested-lists` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PLAN = ROOT / "compiler/pw-core/src/page_values.rs"
SPECULATION = ROOT / "compiler/pw-core/src/backend/speculation.rs"
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"
RENDER = ROOT / "runtime/pw-render/src/lib.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "a list is recorded by its binding",
        "core",
        PLAN,
        "            collections.push(entry.value.clone());\n",
        "            collections.push(root.to_string());\n",
    ),
    (
        "a row's read over a list inside a value is not planned",
        "core",
        PLAN,
        "        .and_then(|t| field_type(sigs, t, &fields))\n",
        "        .filter(|_| fields.is_empty())\n",
    ),
    (
        "a part a speculation would not reach is not refused",
        "core",
        SPECULATION,
        "        if !speculated.iter().any(|(n, ..)| n == root) {\n",
        "        if true || !speculated.iter().any(|(n, ..)| n == root) {\n",
    ),
    (
        "a list is read from its binding, not its path",
        "server",
        SERVER,
        "            let Some(Value::List(items)) = value_at_mut(&mut rendered, &fields) else {\n",
        "            let Some(Value::List(items)) = value_at_mut(&mut rendered, &[]) else {\n",
    ),
    (
        "a row read is applied only to the binding itself",
        "server",
        SERVER,
        "            if collection.split('.').next() == Some(binding) {\n",
        "            if collection == binding {\n",
    ),
    (
        "a value computed for a row is not read whole",
        "render",
        RENDER,
        "                if let Some(whole) = fields.get(&rest.join(\".\")) {\n",
        "                if let Some(whole) = None::<&Value> {\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
    "server": ["cargo", "test", "--quiet", "--locked", "-p", "pw-dev-server"],
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render"],
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
    # Stopped from outside, the source is still restored: SIGTERM raises
    # here, and the `finally` below runs.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    for suite in CARGO:
        built, passed, failed = cargo_tests(suite)
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
        if not built or failed or not passed:
            print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
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
        print(f"{what} [{suite}]: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
