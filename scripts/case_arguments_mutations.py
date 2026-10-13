#!/usr/bin/env python3
"""Mutation controls for ADR-XXXX: a case crosses from a browser (the
integrator's, 2026-10-13; W9's finding in NEXT 6b).

Each mutant undoes one piece: the handler's compiler refusing an `Option`,
a `Result` or a declared case as a command's argument, or sending a case of
a type that contains itself; the browser's wire refusing them; and the host
reading `none` for `some`, a payload a case has none of or one it lacks, a
key a case does not carry, and too few of a case's several fields.

Each must fail `compiler/pw-conformance/tests/case_arguments.rs`.

Run from the repository root; `just e14-case-arguments` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WIRE = ROOT / "compiler/pw-core/src/backend/js_pure.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the handler's compiler refuses an `Option` sent",
        LOWER,
        "        Type::Option(t) => sendable_within(cx, t, on_the_way),\n",
        "        Type::Option(_) => false,\n",
    ),
    (
        "the handler's compiler refuses a `Result` sent",
        LOWER,
        "            sendable_within(cx, t, on_the_way) && sendable_within(cx, e, on_the_way)\n",
        "            false && sendable_within(cx, t, on_the_way) && sendable_within(cx, e, on_the_way)\n",
    ),
    (
        "the handler's compiler refuses a declared case sent",
        LOWER,
        "            if let Some(cases) = d.variants.as_ref() {\n",
        "            if let Some(cases) = d.variants.as_ref().filter(|_| false) {\n",
    ),
    (
        "a case of a type that contains itself is sent",
        LOWER,
        "                if !on_the_way.insert(*def) {\n                    return false;\n",
        "                if !on_the_way.insert(*def) {\n                    return true;\n",
    ),
    (
        "the browser's wire refuses an `Option` and a `Result`",
        WIRE,
        "            Type::Option(_) | Type::Result(..) => self.wire_value(v, ty),\n",
        "",
    ),
    (
        "the browser's wire refuses a declared case",
        WIRE,
        "                Some(Shape::Record { .. } | Shape::Variant { .. }) => self.wire_value(v, ty),\n",
        "                Some(Shape::Record { .. }) => self.wire_value(v, ty),\n",
    ),
    (
        "the host reads `some` as `none`",
        HOST,
        '                ("none", None) => Val::Option(None),\n',
        '                ("none", _) => Val::Option(None),\n',
    ),
    (
        "the host takes a declared case's payload it has none of, or lacks",
        HOST,
        "                    (None, None) => None,\n                    _ => {\n                        return Err(format!(\n                            \"{at}: case `{c}`'s payload, as sent, is not its type's\"\n                        ));\n                    }\n                };\n                Val::Variant(",
        "                    _ => None,\n                };\n                Val::Variant(",
    ),
    (
        "the host takes a key a case does not carry",
        HOST,
        '        if let Some(other) = o.keys().find(|k| *k != "$case" && *k != "value") {\n',
        '        if let Some(other) = o.keys().find(|k| *k != "$case" && *k != "value").filter(|_| false) {\n',
    ),
    (
        "the host takes too few of a case's several fields",
        HOST,
        "                if items.len() != types.len() {\n",
        "                if items.len() > types.len() {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "case_arguments"],
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
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        out, code = bounded(cmd, cwd=ROOT)
        if code is None:
            failed += 1
            continue
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            built = False
            continue
        passed += sum(int(p) for p, _ in found)
        failed += sum(int(f) for _, f in found)
    return built, passed, failed


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
