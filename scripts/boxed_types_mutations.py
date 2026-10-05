#!/usr/bin/env python3
"""Mutation controls for ADR-0202: a type that holds itself in place is
boxed, and crosses a boundary as its nodes.

Each mutant undoes one piece: which types are boxed, a value built in its
cell and held as its address, a box read and matched through its cell, each
kind of slot in the node's layout, the encoder copying a box's child, the
decoder's checks of a box's index, the host's slots, and the WIT and the
analysis a node's box is written by.

Every mutant must fail the tests of the rule, or the compiled programs'.

Run from the repository root; `just e14-boxed-types` records the output.
The source is restored after every mutant, whatever happens.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"
WIT = ROOT / "compiler/pw-core/src/wit.rs"
RECURSION = ROOT / "compiler/pw-core/src/recursion.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "no type is boxed",
        WASM,
        "        boxed: boxed_cycles(declared),\n",
        "        boxed: BTreeSet::new(),\n",
    ),
    (
        "an option's payload is not held in place",
        WASM,
        "            Type::Option(a) => in_place(a, out),\n",
        "            Type::Option(_) => {}\n",
    ),
    (
        "a record is built as its box's word",
        WASM,
        "        let (rt, boxed) = match self.cell_of(rt) {\n",
        "        let (rt, boxed) = match None::<WitType> {\n",
    ),
    (
        "a case is built as its box's word",
        WASM,
        "        let (t, boxed) = match self.cell_of(t) {\n",
        "        let (t, boxed) = match None::<WitType> {\n",
    ),
    (
        "a box held in memory is its cell, unread",
        WASM,
        "                    I::LocalSet(at),\n                ]);\n                Held::Memory { ty: cell, ptr: at }\n",
        "                    I::Drop,\n                ]);\n                Held::Memory { ty: cell, ptr }\n",
    ),
    (
        "a box's field is read without its cell",
        WASM,
        "        let h = self.unboxed(h);\n",
        "",
    ),
    (
        "a box is matched without its cell",
        WASM,
        "        let scrutinee_held = self.held.get(&scrutinee).cloned().map(|h| self.unboxed(h));\n",
        "        let scrutinee_held = self.held.get(&scrutinee).cloned();\n",
    ),
    (
        "a list of boxes is no slot",
        WASM,
        "        } else if a_list.is_some_and(|e| is_box(&e)) && b_list {\n",
        "        } else if a_list.is_some_and(|e| is_box(&e)) && b_list && false {\n",
    ),
    (
        "a box is no slot",
        WASM,
        "        } else if is_box(a) && is_u32(b) {\n",
        "        } else if is_box(a) && is_u32(b) && false {\n",
    ),
    (
        "an option of a box is no slot",
        WASM,
        "        } else if let Some(payload) = a_option.filter(|e| is_box(e)).filter(|_| b_option) {\n",
        "        } else if let Some(payload) = a_option.filter(|e| is_box(e)).filter(|_| b_option && false) {\n",
    ),
    (
        "a box's child is not copied",
        WASM,
        "        body.extend(address(nodes, n, size));\n        body.extend([\n            I::LocalGet(src),\n            I::I32Const(size),\n            copy.clone(),\n",
        "        body.extend(address(nodes, n, size));\n        body.extend([\n            I::Drop,\n            I::LocalGet(src),\n            I::Drop,\n",
    ),
    (
        "a box's child may come before its node",
        WASM,
        "        let mut body = trap_if(vec![I::LocalGet(next), I::LocalGet(i), I::I32LeU]);\n",
        "        let mut body = Vec::new();\n",
    ),
    (
        "a box's index is not held to the run",
        WASM,
        "            I::I32Load(word(offset)),\n            I::LocalGet(next),\n            I::I32Ne,\n",
        "            I::I32Load(word(offset)),\n            I::LocalGet(next),\n            I::I32Ne,\n            I::Drop,\n            I::I32Const(0),\n",
    ),
    (
        "the host reads a u32 as no slot",
        HOST,
        "                Type::U32 => Some(Kind::One),\n",
        "",
    ),
    (
        "the host reads an option<u32> as no slot",
        HOST,
        "                Type::Option(o) if o.ty() == Type::U32 => Some(Kind::Maybe),\n",
        "",
    ),
    (
        "a box crosses as the type it is",
        WIT,
        '            Some(Slot::Box) => Ok("u32".to_string()),\n',
        "            Some(Slot::Box) => wit_type(ty, types, name),\n",
    ),
    (
        "the type itself is no slot",
        RECURSION,
        "    if is_self(ty) {\n",
        "    if is_self(ty) && false {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "recursive_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "boxed_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "recursive_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "javascript", "a_type_that_holds_itself"],
]

# How long one command may run. Past the bound it is killed with everything
# it started, and the run counts as failing.
BOUND = 1200


def run_tests():
    """(built, passed, failed) over the tests. A test process that aborts,
    as a stack overflow does, fails."""
    passed = failed = 0
    for command in TESTS:
        p = subprocess.Popen(command, cwd=ROOT, start_new_session=True,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            out, _ = p.communicate(timeout=BOUND)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.communicate()
            return True, passed, failed + 1
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "could not compile" in out or "error[E" in out:
                return False, passed, failed
            # It built, and the process ended before it reported.
            failed += 1
            continue
        passed += sum(int(n) for n, _ in found)
        failed += sum(int(f) for _, f in found)
    return True, passed, failed


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
