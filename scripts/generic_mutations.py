#!/usr/bin/env python3
"""Mutation controls for ADR-0062: generic types in the backend.

Each mutant undoes one piece of how an instance of a generic record, sum
type or opaque type is carried, inferred, defined or laid out: in the
lowering, the component encoder, or the world. The generics conformance
tests, the component-against-module differential, and the WIT-names tests
must then fail.

Run from the repository root; `just e10-generics` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
WIT = ROOT / "compiler/pw-core/src/wit.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "an instance's arguments are dropped",
        LOWER,
        "        return Lowering::Lowered(Type::Nominal(def, args));",
        "        let _ = args;\n        return Lowering::Lowered(Type::Nominal(def, Vec::new()));",
    ),
    (
        "a field is read without its instance's arguments",
        LOWER,
        "        let subst = self.instance_subst(def, &instance);",
        "        let subst = self.instance_subst(def, &[]);",
    ),
    (
        "a field's type does not fix the instance",
        LOWER,
        "                if !actual.is_some_and(|a| instantiate(self.cx.sigs, t, &a, &mut subst)) {",
        "                if actual.is_none() {",
    ),
    (
        "the instance the context names is not read",
        LOWER,
        "        if let Some(Type::Nominal(d, args)) = expected\n            && *d == def",
        "        if let Some(Type::Nominal(d, args)) = expected\n            && *d == def\n            && args.is_empty()",
    ),
    (
        "a declaration is defined once, whatever its instance",
        LOWER,
        "        if !seen.insert((def, args.clone())) {",
        "        if !seen.insert((def, Vec::new())) {",
    ),
    (
        "a case's elements do not fix its instance",
        LOWER,
        "            if !instantiate(self.cx.sigs, d.resolved()?, f, &mut subst) {",
        "            if subst.is_empty() && !instantiate(self.cx.sigs, d.resolved()?, f, &mut subst) {",
    ),
    (
        "an instance is laid out as another of its declaration",
        WASM,
        "                        .find(|d| d.def == *def && d.args == *args)?;",
        "                        .find(|d| d.def == *def)?;",
    ),
    (
        "an instance inside another is taken for recursion",
        WASM,
        "                    if self.visiting.contains(&(*def, args.clone())) {",
        "                    if self.visiting.iter().any(|(d, _)| d == def) {",
    ),
    (
        "a type takes a query's place in the world",
        WIT,
        "            if matches!(d.kind, DeclKind::Type | DeclKind::Opaque) {",
        "            if matches!(d.kind, DeclKind::Type | DeclKind::Opaque) && false {",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "generics"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "wit_names"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript"],
]


def run_tests():
    """(built, passed, failed) over every test command."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if m is None:
            built = False
            continue
        passed += int(m.group(1))
        failed += int(m.group(2))
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
