#!/usr/bin/env python3
"""Mutation controls for ADR-0059: declared sum types.

Each mutant undoes one piece of how a case is typed where it is written, how
a pattern names one, how a case is built and matched in the lowering, the
component or the JavaScript module, or what is refused by name. The checker
tests, the conformance tests, and the component-against-module differential
must then fail.

Run from the repository root; `just e10-sum-types` records the output.
The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VALUES = ROOT / "compiler/pw-core/src/values.rs"
CHECK = ROOT / "compiler/pw-core/src/check.rs"
NAMES = ROOT / "compiler/pw-core/src/names.rs"
GRAMMAR = ROOT / "compiler/pw-syntax/src/grammar.rs"
WIT = ROOT / "compiler/pw-core/src/wit.rs"
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a case its type lacks is not named",
        VALUES,
        "        None => CaseNamed::Unknown(def, case.to_string()),",
        "        None => return None,",
    ),
    (
        "a case's fields are not its arguments",
        VALUES,
        "                    params: fields.iter().map(|r| Some(r.clone())).collect(),",
        "                    params: fields.iter().map(|_| None).collect(),",
    ),
    (
        "an arm's fields are not bound",
        VALUES,
        "                        TypeResolution::Resolved(t) => substituted(&Ty::of(t), *def, args),",
        "                        TypeResolution::Resolved(_) => Ty::Unknown,",
    ),
    (
        "an arm's body is walked without its bindings",
        VALUES,
        "                        for (b, t) in self.arm_bindings(&st, arm.pat) {\n                            added |= self.bind(b, t);",
        "                        for (b, t) in self.arm_bindings(&st, arm.pat) {\n                            let _ = (b, t);",
    ),
    (
        "a bare case is not typed",
        VALUES,
        "        1 => found.into_iter().next(),",
        "        1 => None,",
    ),
    (
        "a pattern through its type is a binding",
        GRAMMAR,
        "                let is_ctor = takes_args || after > 1;",
        "                let is_ctor = takes_args;",
    ),
    (
        "a pattern's qualifier is not checked",
        CHECK,
        "                    if qualifier(q).is_some_and(|d| subject.defs.get(t) == Some(&d)))",
        "                    if q.len() < usize::MAX)",
    ),
    (
        "a bare case two types declare is not reported",
        NAMES,
        "            && types.len() > 1",
        "            && types.len() > 2",
    ),
    (
        "a bare case with a payload is not told its qualified form",
        CHECK,
        "    if !case_of.is_empty() {",
        "    if false && !case_of.is_empty() {",
    ),
    (
        "`_` takes every case, those taken before it too",
        LOWER,
        "                    (0..cases.len()).filter(|i| !taken.contains(i)).collect(),",
        "                    (0..cases.len()).collect(),",
    ),
    (
        "an or-pattern's alternatives take no case",
        LOWER,
        "                            taken.push(i)",
        "                            let _ = i;",
    ),
    (
        "a type that contains itself reaches the world",
        LOWER,
        "        if contains_itself(sigs, def) {",
        "        if false && contains_itself(sigs, def) {",
    ),
    (
        "a 16-bit discriminant is stored in 8 bits",
        WASM,
        "        Int::U16 => I::I32Store16(at(1)),",
        "        Int::U16 => I::I32Store8(at(0)),",
    ),
    (
        "a joined slot is read without its coercion",
        WASM,
        "            if have == want {\n                values.push(slots[k]);",
        "            if have == want || k < usize::MAX {\n                values.push(slots[k]);",
    ),
    (
        "a case's fields are written at its payload's start",
        WASM,
        "            .map(|(o, t)| (o.size_wasm32() as u64, *t))",
        "            .map(|(_, t)| (0, *t))",
    ),
    (
        "an arm's cases are tested for inequality",
        WASM,
        "                        self.ops.push(I::I32Eq);\n                        if j > 0 {",
        "                        self.ops.push(I::I32Ne);\n                        if j > 0 {",
    ),
    (
        "a flat scrutinee is matched unwritten",
        WASM,
        "                    &mut self.ops,\n                ) {\n"
        "                    Encoding::Encoded(_) => {}\n"
        "                    other => return other.map(|_| unreachable!()),\n"
        "                }\n                (st, area, holder)",
        "                    &mut Vec::new(),\n                ) {\n"
        "                    Encoding::Encoded(_) => {}\n"
        "                    other => return other.map(|_| unreachable!()),\n"
        "                }\n                (st, area, holder)",
    ),
    (
        "a module names a case as Pleris writes it",
        JS,
        "                (crate::wit::ident(name), fields.clone())",
        "                (name.clone(), fields.clone())",
    ),
    (
        "a module binds a case's several fields as one",
        JS,
        '                true => format!("{}.value[{k}]", val(*scrutinee)),',
        '                true => format!("{}.value", val(*scrutinee)),',
    ),
    (
        "a union of one case is written as a record",
        WIT,
        "            && !variants.is_empty()",
        "            && variants.len() > 1",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "sum_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "sum_types"],
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
