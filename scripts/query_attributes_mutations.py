#!/usr/bin/env python3
"""Mutation controls for ADR-0171: an attribute at the top of the page that
reads a query's value is set again when the value changes.

Each mutant undoes one piece:
- the plan: no such attribute named; one in a block or a row named; one
  written with several values named twice;
- the development server: the attributes not shown; one set again though
  unchanged; a boolean attribute that goes set rather than removed.

A plan mutant must fail `pw-core`'s tests, all of them run; a server
mutant, the server's.

Run from the repository root; `just e14-query-attributes` records the
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
SERVER = ROOT / "spikes/own-renderer/server/src/main.rs"

# (what, suite, file, anchor, replacement)
MUTANTS = [
    (
        "no attribute is named",
        "core",
        PLAN,
        "            attributes.push(read.part.0);\n",
        "            let _ = read.part;\n",
    ),
    (
        "one in a block or a row is named",
        "core",
        PLAN,
        "            && !read.nested\n",
        "",
    ),
    (
        "one written with several values is named twice",
        "core",
        PLAN,
        "            && !attributes.contains(&read.part.0)\n",
        "",
    ),
    (
        "the attributes are not shown",
        "server",
        SERVER,
        "            shown.attributes.insert(id, written);\n",
        "            let _ = (id, written);\n",
    ),
    (
        "an unchanged attribute is set again",
        "server",
        SERVER,
        "            if was.attributes.get(id) == Some(written) {\n",
        "            if false && was.attributes.get(id) == Some(written) {\n",
    ),
    (
        "a boolean attribute that goes is set",
        "server",
        SERVER,
        "                    None => PatchOp::RemoveAttribute { name },\n",
        "                    None => PatchOp::SetAttribute { name, value: String::new() },\n",
    ),
]

CARGO = {
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core"],
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
        print(f"baseline ({suite}): {passed} passed, {failed} failed")
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
        print(f"{what} [{suite}]: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
