#!/usr/bin/env python3
"""Mutation controls for ADR-0326: a name a file imports by name is the
file's, before a whole module's.

Each mutant undoes one piece: the name's imports and the whole modules' read
as one set again; the whole modules never read; the ambiguous call not
refused at check; the refusal naming one module; its repair naming one
qualified form.

Each must fail `compiler/pw-core/tests/explicit_imports.rs`.

Run from the repository root; `just e14-explicit-imports` records the
output. The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RESOLVE = ROOT / "compiler/pw-core/src/resolve.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "a name's imports and the whole modules' are one set again",
        RESOLVE,
        "                if by_name != named || (!by_name && !imp.names.is_empty()) {\n",
        "                if !(named || imp.names.is_empty()) || !by_name {\n",
    ),
    (
        "the whole modules are never read",
        RESOLVE,
        "        for by_name in [true, false] {\n",
        "        for by_name in [true] {\n",
    ),
    (
        "a call two whole modules declare is not refused at check",
        CHECK,
        "                if !bound\n                    && matches!(body.expr(*callee), Expr::Name(_))\n                    && let Resolution::Ambiguous(defs) =\n",
        "                if false\n                    && matches!(body.expr(*callee), Expr::Name(_))\n                    && let Resolution::Ambiguous(defs) =\n",
    ),
    (
        "the refusal names one module",
        CHECK,
        "        .join(\" and \");\n",
        "        .join(\" and \")\n        .split(\" and \")\n        .next()\n        .unwrap_or_default()\n        .to_string();\n",
    ),
    (
        "the repair names one qualified form",
        CHECK,
        "        .join(\" or \");\n",
        "        .join(\" or \")\n        .split(\" or \")\n        .next()\n        .unwrap_or_default()\n        .to_string();\n",
    ),
]

CARGO = [
    "cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "explicit_imports",
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
