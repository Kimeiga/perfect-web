#!/usr/bin/env python3
"""Mutation controls for ADR-0194: a type that contains itself compiles, and
crosses a component boundary as its nodes.

Each mutant undoes one piece:
- the checker's rule for which types have a value;
- the component's private layout: a list waiting for its element, and a
  type that contains itself taking the world's layout;
- the conversions where such a value crosses: an export's parameter, a host's
  answer, and what the world's type is;
- the encoder's indices, and each check the decoder makes of nodes;
- the WIT a node is written as;
- the host's conversions: an argument, an answer, what a data layer is given,
  and how deep a value it nests;
- the contract's invariant paths through every node;
- (the browser's wire, which refused such a value by name until ADR-0205).

Every mutant must fail the tests of a type that contains itself.

Run from the repository root; `just e14-recursive-types` records the output.
The source is restored after every mutant, whatever happens.
Three controls were retired by ADR-0202, one with the code it changed, and
one by ADR-0205, each with its reason, below.
"""

import os
import pathlib
import re
import signal
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOWER = ROOT / "compiler/pw-core/src/backend/lower.rs"
WASM = ROOT / "compiler/pw-core/src/backend/wasm.rs"
WIT = ROOT / "compiler/pw-core/src/wit.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"
CONTRACT = ROOT / "compiler/pw-core/src/contract.rs"
RECURSION = ROOT / "compiler/pw-core/src/recursion.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    # Retired by ADR-0202: "every type that contains itself is refused
    # again", "a type that holds itself in place reaches the backend" and "a
    # type that holds itself in place is written as its nodes". The lowering
    # and the WIT refused a type held in place by name; ADR-0202 boxes it, so
    # it reaches the backend and is written as its nodes by design, and
    # `boxed_types_mutations.py` controls how.
    (
        "a result needs both its sides to have a value",
        RECURSION,
        "                Builtin::Result => t.args().iter().any(|a| self.ty(a, binder, args)),\n",
        "                Builtin::Result => t.args().iter().all(|a| self.ty(a, binder, args)),\n",
    ),
    # Retired at 7d3234b, where it survived: "a list holds its elements in
    # place". It changed `contains_itself_in_place`, which nothing has called
    # since ADR-0202 moved the boxing into the encoder; the function is gone.
    # Which types are boxed is `boxed_types_mutations.py`'s.
    (
        "a list waiting for its element never gets it",
        WASM,
        "                Some(w) => resolve.types[id].kind = TypeDefKind::List(w),\n",
        "                Some(_) => {}\n",
    ),
    (
        "a type that contains itself takes the world's layout",
        WASM,
        "                if !self.holders.contains(&(*def, args.clone()))\n",
        "                if self.holders.len() < usize::MAX\n",
    ),
    (
        "a world's type never holds a node index",
        WASM,
        "            WitType::U32 => true,\n            WitType::Id(id) if seen.insert(id) =>",
        "            WitType::U32 => false,\n            WitType::Id(id) if seen.insert(id) =>",
    ),
    (
        "an export's parameter stays its nodes",
        WASM,
        "            Some(own) if holds_index(resolve, &param.ty) => match enc.convert(arrived, own) {\n",
        "            Some(own) if own == param.ty => match enc.convert(arrived, own) {\n",
    ),
    (
        "a host's answer stays its nodes",
        WASM,
        "                    (Some(rt), Some(own)) if holds_index(resolve, rt) => {\n",
        "                    (Some(rt), Some(own)) if *rt == own => {\n",
    ),
    (
        "the encoder numbers each list's children from zero",
        WASM,
        "            I::I32Add,\n            I::LocalGet(n),\n            I::LocalGet(j),\n"
        "            I::I32Add,\n            I::I32Store(word(0)),\n",
        "            I::I32Add,\n            I::LocalGet(j),\n            I::I32Store(word(0)),\n",
    ),
    (
        "the decoder lets a node hold one at or before itself",
        WASM,
        "            I::LocalGet(next),\n            I::LocalGet(i),\n            I::I32LeU,\n",
        "            I::LocalGet(next),\n            I::LocalGet(i),\n            I::I32LtU,\n",
    ),
    (
        "the decoder takes any index as the run's next",
        WASM,
        "            I::LocalGet(next),\n            I::LocalGet(j),\n            I::I32Add,\n"
        "            I::I32Ne,\n        ]));\n",
        "            I::LocalGet(next),\n            I::LocalGet(j),\n            I::I32Add,\n"
        "            I::I32Ne,\n            I::Drop,\n            I::I32Const(0),\n        ]));\n",
    ),
    (
        "the decoder takes nodes no list holds",
        WASM,
        "    ops.extend(trap_if(vec![I::LocalGet(next), I::LocalGet(len), I::I32Ne]));\n",
        "    ops.extend(trap_if(vec![\n        I::LocalGet(next),\n        I::LocalGet(len),\n"
        "        I::I32Ne,\n        I::Drop,\n        I::I32Const(0),\n    ]));\n",
    ),
    (
        "a node's list of children is written as a list of the type",
        # Re-anchored by ADR-0202: a slot is one of three kinds.
        WIT,
        '            Some(Slot::List) => Ok("list<u32>".to_string()),\n',
        '            Some(Slot::List) => Ok("list<u64>".to_string()),\n',
    ),
    # Retired by ADR-0205: "the browser's wire writes such a value out by its
    # shape". The browser's wire refused such a value by name; ADR-0205
    # writes it as its nodes, and `graphs_on_the_wire_mutations.py` controls
    # how.
    (
        "an invariant inside a tree is checked at its root alone",
        CONTRACT,
        "            let nodes = crate::recursion::contains_itself(sigs, *def);\n",
        "            let nodes = false;\n",
    ),
    (
        "the host passes a nested argument as it is",
        HOST,
        "            .map(|(v, (_, t))| graph::tangle(v, &t))\n",
        "            .map(|(v, (_, _t))| Ok::<Val, String>(v))\n",
    ),
    (
        "a data layer is given nodes",
        HOST,
        "                            .map(|(v, (_, t))| graph::untangle(v, &t))\n",
        "                            .map(|(v, (_, _t))| Ok::<Val, String>(v))\n",
    ),
    (
        "a data layer's nested answer is read as it is",
        HOST,
        "                            *slot = graph::tangle(v, &ty)\n"
        "                                .and_then(|v| project(v, &ty))\n",
        "                            *slot = project(v, &ty)\n",
    ),
    (
        "a host nests a value of any depth",
        HOST,
        "                        if depth[c] > NESTED_DEPTH {\n",
        "                        if depth[c] > usize::MAX - 1 {\n",
    ),
]

TESTS = [
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-core",
        "--test", "recursive_types",
    ],
    [
        "cargo", "test", "--quiet", "--locked", "-p", "pw-conformance",
        "--test", "recursive_types",
    ],
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
