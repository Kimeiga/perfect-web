#!/usr/bin/env python3
"""Mutation controls for ADR-0050: recursion and generic callees compile.

Each mutant undoes one piece of how a recursive or generic callee is
compiled beside its export, or how the two encoders call it, and the
recursion tests must then fail.

Run from the repository root; `just e10-recursion` records the output. The
source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"

# (what is undone, file, anchor, replacement)
MUTANTS = [
    (
        "a recursion's callee is never compiled",
        LOWER,
        "            if !begun {\n",
        "            if begun && !begun {\n",
    ),
    (
        "every instance of a callee is one",
        LOWER,
        """            return Lowering::Lowered(self.push(Instr::Call {
                result,
                callee,
                instance,""",
        """            let _ = instance;
            return Lowering::Lowered(self.push(Instr::Call {
                result,
                callee,
                instance: Vec::new(),""",
    ),
    (
        "a callee compiled beside the export forgets its instance",
        LOWER,
        """        inlining: vec![def],
        subst,
        internal,""",
        """        inlining: vec![def],
        subst: {
            let _ = subst;
            BTreeMap::new()
        },
        internal,""",
    ),
    (
        "a list's element type binds nothing",
        LOWER,
        """            (Builtin::List, Type::List(t)) | (Builtin::Option, Type::Option(t)) => {
                args.first().is_some_and(|a| instantiate(sigs, a, t, subst))
            }""",
        """            (Builtin::List, Type::List(_)) | (Builtin::Option, Type::Option(_)) => true,""",
    ),
    (
        "a record or variant is passed flat",
        WASM,
        "        Some(f) if !compound => Passed::Flat(core_types(&f)),",
        "        Some(f) => Passed::Flat(core_types(&f)),",
    ),
    (
        "a callee's result is left on the stack",
        WASM,
        """                for l in ls.iter().rev() {
                    self.ops.push(I::LocalSet(*l));
                }
                Held::Flat { ty, locals: ls }""",
        """                Held::Flat { ty, locals: ls }""",
    ),
    (
        "the module omits its callees",
        JS,
        """        let mut before: Vec<&str> = prelude;
        before.extend(callees.iter().map(String::as_str));""",
        """        let before: Vec<&str> = prelude;
        let _ = callees;""",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "recursion"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "computation"],
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
