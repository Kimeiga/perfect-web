#!/usr/bin/env python3
"""Mutation controls for ADR-0248 (ruling 0057-a): a map's key is an `Int`,
a `String`, a `Bool`, or an opaque type over one, refused at check where it
is not.

Each mutant undoes one piece: what a key may be, in the checker and in the
backend; the refusal of a written key, of one inside another type, and of
one a call instantiates; and the order a `Bool` key is kept in, by the
component and by the browser's module. The tests of each must then fail:
the checker's, the host's against Rust's `BTreeMap`, and the browser's
module against the component.

Run from the repository root; `just e14-map-keys` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a key with no order is a key to the checker",
        VALUES,
        "        Ty::Primitive(_) | Ty::Builtin(..) => Some(ordered(key)),\n",
        "        Ty::Primitive(_) | Ty::Builtin(..) => Some(true || ordered(key)),\n",
    ),
    (
        "a Bool is no key to the checker",
        VALUES,
        "            Ty::Primitive(Primitive::Int | Primitive::Str | Primitive::Bool)\n",
        "            Ty::Primitive(Primitive::Int | Primitive::Str)\n",
    ),
    (
        "an opaque type is no key to the checker",
        VALUES,
        "                .is_some_and(|r| matches!(r, TypeResolution::Resolved(t) if ordered(&Ty::of(t)))),\n",
        "                .is_some_and(|r| matches!(r, TypeResolution::Resolved(t) if ordered(&Ty::of(t)) && false)),\n",
    ),
    (
        "a key inside another type is not read",
        VALUES,
        "        for a in args {\n            unordered_keys(sigs, a, out);\n        }\n",
        "        for a in args.iter().filter(|_| false) {\n            unordered_keys(sigs, a, out);\n        }\n",
    ),
    (
        "a written key is not refused",
        VALUES,
        "            unordered_keys(sigs, &Ty::of(t), &mut keys);\n",
        "            unordered_keys(sigs, &Ty::Unknown, &mut keys);\n",
    ),
    (
        "a key a call instantiates is not refused",
        VALUES,
        "            unordered_keys(self.sigs, &solved.result, &mut keys);\n",
        "            unordered_keys(self.sigs, &Ty::Unknown, &mut keys);\n",
    ),
    (
        "every call's result is taken for a generic map's",
        VALUES,
        "                |r| matches!(r, TypeResolution::Resolved(t) if keyed_by_a_parameter(t)),\n",
        "                |r| matches!(r, TypeResolution::Resolved(_)),\n",
    ),
    (
        "a Bool is no key to the backend",
        LOWER,
        "        Type::Int | Type::Str | Type::Bool => true,\n",
        "        Type::Int | Type::Str => true,\n",
    ),
    (
        "an opaque type is no key to the backend",
        LOWER,
        "                    Lowering::Lowered(Type::Int | Type::Str | Type::Bool)\n",
        "                    Lowering::Lowered(Type::Int | Type::Str | Type::Bool) if false\n",
    ),
    (
        "the component orders a Bool key backwards",
        WASM,
        "                I::I32GtU,\n                I::LocalGet(x[0]),\n                I::LocalGet(y[0]),\n                I::I32LtU,\n",
        "                I::I32LtU,\n                I::LocalGet(x[0]),\n                I::LocalGet(y[0]),\n                I::I32GtU,\n",
    ),
    (
        "the browser's module orders a Bool key backwards",
        JS,
        "         if (typeof a === \\\"boolean\\\") return a === b ? 0 : a ? 1 : -1;\\n  \\\n",
        "         if (typeof a === \\\"boolean\\\") return a === b ? 0 : a ? -1 : 1;\\n  \\\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "map_keys"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "maps"],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript",
        "--", "every_query_agrees_with_its_component_under_node",
    ],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        results = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not results:
            built = False
            continue
        for p, f in results:
            passed += int(p)
            failed += int(f)
    return built, passed, failed


def main():
    built, passed, failed = run_tests()
    print(f"baseline: {passed} passed, {failed} failed")
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
        print(f"{what}: {verdict}")

    print(f"{len(MUTANTS) - survivors} of {len(MUTANTS)} mutants killed")
    return 1 if survivors else 0


if __name__ == "__main__":
    sys.exit(main())
