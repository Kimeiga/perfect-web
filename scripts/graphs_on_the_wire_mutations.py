#!/usr/bin/env python3
"""Mutation controls for ADR-0205: a value of a type that contains itself on
the browser's wire, as its nodes.

Each mutant undoes one piece:
- the renderer: a graph read as a tree (each node later, held once, held by
  one), JSON read in place, and a value dropped, copied and compared with a
  stack on the heap;
- the browser's modules: a graph read as a tree, a value written in level
  order, a signal read as its nodes, and what a host writes of one, a
  capture, still refused;
- two types that hold each other refused;
- the build writing a first value as its nodes, in the order its fields
  are declared;
- the host reading a browser's graph, each node's slots as indices.

Every mutant must fail `graphs.rs`, `graphs_on_the_wire.rs`,
`browser_graphs.rs` or `recursive_types.rs`.

Run from the repository root; `just e14-graphs-on-the-wire` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RENDER = ROOT / "runtime/pw-render/src/lib.rs"
JS = ROOT / "compiler/pw-core/src/backend/js_pure.rs"
FIRST = ROOT / "compiler/pw-core/src/backend/signals.rs"
HOST = ROOT / "runtime/pw-host/src/lib.rs"

# (what, file, anchor, replacement)
MUTANTS = [
    (
        "the renderer takes an earlier node",
        RENDER,
        "        if k <= self.at || k >= self.built.len() {\n",
        "        if k >= self.built.len() {\n",
    ),
    (
        "the renderer takes a node twice",
        RENDER,
        "        self.built[k]\n            .take()\n",
        "        self.built[k]\n            .clone()\n",
    ),
    (
        "the renderer keeps a node no other holds",
        RENDER,
        "        if built.iter().skip(1).any(Option::is_some) {\n",
        "        if false {\n",
    ),
    (
        "a value is dropped by recursion",
        RENDER,
        "impl Drop for Value {\n"
        "    /// Taken apart with a stack on the heap, not by recursion (ADR-0205).\n"
        "    fn drop(&mut self) {\n",
        "impl Value {\n"
        "    #[allow(dead_code)]\n"
        "    fn not_dropped(&mut self) {\n",
    ),
    (
        "a list's copies come back in another order",
        RENDER,
        "                        steps.push(Step::Up(v));\n"
        "                        steps.extend(items.iter().rev().map(Step::Into));\n",
        "                        steps.push(Step::Up(v));\n"
        "                        steps.extend(items.iter().map(Step::Into));\n",
    ),
    (
        "the reader puts a list's items in another order",
        RENDER,
        "                        steps.push(Step::List(items.len()));\n"
        "                        steps.extend(items.iter().rev().map(Step::Into));\n",
        "                        steps.push(Step::List(items.len()));\n"
        "                        steps.extend(items.iter().map(Step::Into));\n",
    ),
    (
        "the reader gives a record's fields another's values",
        RENDER,
        "                            steps.extend(o.values().rev().map(Step::Into));\n",
        "                            steps.extend(o.values().map(Step::Into));\n",
    ),
    (
        "two numbers compare equal",
        RENDER,
        "                (Value::Int(a), Value::Int(b)) if a == b => {}\n",
        "                (Value::Int(_), Value::Int(_)) => {}\n",
    ),
    (
        "the browser takes an earlier node",
        JS,
        "k <= at || k >= nodes.length || held[k]",
        "k >= nodes.length || held[k]",
    ),
    (
        "the browser takes a node twice",
        JS,
        "k <= at || k >= nodes.length || held[k]",
        "k <= at || k >= nodes.length",
    ),
    (
        "the browser keeps a node no other holds",
        JS,
        "             for (let k = 1; k < nodes.length; k++) if (!held[k]) throw new Error(\\\"trap: a `$graph` that is not a tree\\\"); \\\n",
        "             \\\n",
    ),
    (
        "the browser numbers a node one past its place",
        JS,
        "return {{ $node: queue.length - 1 }}",
        "return {{ $node: queue.length }}",
    ),
    (
        "a signal is read as written nested",
        JS,
        "                self.graphs = true;\n",
        "                self.graphs = false;\n",
    ),
    (
        "a captured value is read as its nodes",
        JS,
        "        if !self.graphs && holds_itself(self.program, ty) {\n",
        "        if false && holds_itself(self.program, ty) {\n",
    ),
    (
        "two types that hold each other are put on the wire",
        JS,
        "fn held_back(program: &Program, def: DefId, args: &[Type]) -> Result<(), String> {\n",
        "fn held_back(program: &Program, def: DefId, args: &[Type]) -> Result<(), String> {\n"
        "    if program.types.len() < usize::MAX {\n        return Ok(());\n    }\n",
    ),
    (
        "the build puts a node's fields in another order",
        JS,
        "            // In the order the fields are declared, as `wire_graph` puts\n"
        "            // them.\n"
        "            for (n, t) in fields {\n",
        "            // In the order the fields are declared, as `wire_graph` puts\n"
        "            // them.\n"
        "            for (n, t) in fields.iter().rev() {\n",
    ),
    (
        "the build writes a first value nested",
        FIRST,
        "    super::js_pure::wire_json(&program, &ty, first)\n",
        "    Ok::<_, String>(first)\n",
    ),
    (
        "the host refuses a browser's graph",
        HOST,
        "            Type::List(_) if graph::is_nodes(ty) => graph::from_browser(ty, v, at, &from_json)?,\n",
        "            Type::List(_) if graph::is_nodes(ty) => return Err(format!(\"{at}: no graph\")),\n",
    ),
    (
        "the host reads a node's slots as any field",
        HOST,
        # Re-anchored by ADR-0233, whose writer reads slots the same way: the
        # reader's, by the place it names.
        "                    let place = format!(\"{at}.{written}\");\n"
        "                    let slot = slots.iter().find_map(|s| match s {\n",
        "                    let place = format!(\"{at}.{written}\");\n"
        "                    let slot = slots.iter().take(0).find_map(|s| match s {\n",
    ),
]

TESTS = [
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-render", "--test", "graphs"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "graphs_on_the_wire"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-core", "--test", "recursive_types"],
    ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance", "--test", "browser_graphs"],
]


def run():
    """(built, passed, failed) over every test command. A test binary that
    aborts, as one overflowing its stack does, is a failure."""
    built, passed, failed = True, 0, 0
    for cmd in TESTS:
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        out = r.stdout + r.stderr
        found = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", out)
        if not found:
            if "error[" in out or "could not compile" in out:
                built = False
            else:
                failed += 1
            continue
        for p, f in found:
            passed += int(p)
            failed += int(f)
        if r.returncode != 0 and not any(int(f) for _, f in found):
            failed += 1
    return built, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    built, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed")
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
            built, passed, failed = run()
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
