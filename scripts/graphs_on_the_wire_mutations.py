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

Every mutant must complete all 25 named Cargo tests, with a native assertion
failure in `graphs.rs`, `graphs_on_the_wire.rs`, `browser_graphs.rs` or
`recursive_types.rs`. Builds, panics, incomplete runs and process stops are
invalid evidence. The explicit deep-drop property alone can observe a
nonce-bound native stack overflow before its parent asserts the contract.

Run from the repository root; `just e14-graphs-on-the-wire` records the
output. The source is restored after every mutant, whatever happens.
"""

import pathlib
import signal
import subprocess
import sys
import cargo_assertion_results
import mutation_source
import producer_process

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

CARGO = {
    "render": ["cargo", "test", "--quiet", "--locked", "-p", "pw-render",
             "--test", "graphs", "--", "--format=pretty"],
    "core": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
             "--test", "graphs_on_the_wire", "--", "--format=pretty"],
    "recursive": ["cargo", "test", "--quiet", "--locked", "-p", "pw-core",
             "--test", "recursive_types", "--", "--format=pretty"],
    "host": ["cargo", "test", "--quiet", "--locked", "-p", "pw-conformance",
             "--test", "browser_graphs", "--", "--format=pretty"],
}
SOURCES = {
    "render": ROOT / "runtime/pw-render/tests/graphs.rs",
    "core": ROOT / "compiler/pw-core/tests/graphs_on_the_wire.rs",
    "recursive": ROOT / "compiler/pw-core/tests/recursive_types.rs",
    "host": ROOT / "compiler/pw-conformance/tests/browser_graphs.rs",
}
NAMES = {
    "render": (
        "a_graph_is_the_value_its_nodes_make",
        "a_graph_that_is_not_a_tree_is_refused",
        "a_value_as_deep_as_its_data_is_held_without_recursion",
        "a_copy_is_the_value_and_its_own",
        "nested_json_as_deep_as_its_data_is_read_without_recursion",
        "the_reader_keeps_each_item_and_field_in_its_place",
    ),
    "core": (
        "the_build_writes_a_first_value_as_its_nodes",
        "a_handler_reads_a_signal_from_its_nodes_and_writes_one_as_its_nodes",
        "a_graph_that_is_not_a_tree_traps_in_the_handler",
        "a_handler_sends_a_command_a_value_as_its_nodes",
        "what_a_host_writes_of_one_and_two_types_that_hold_each_other_are_refused",
        "each_node_holds_the_next_run_in_the_order_its_fields_are_declared",
    ),
    "recursive": (
        "a_type_no_finite_value_has_is_refused_where_it_is_declared",
        "a_type_that_can_be_built_without_itself_is_not",
        "a_type_that_contains_itself_crosses_as_its_nodes",
        "a_type_that_contains_itself_otherwise_crosses_no_boundary_yet",
        "a_type_that_contains_itself_in_place_is_compiled_boxed",
        "an_invariant_inside_a_tree_holds_at_every_node",
        "a_value_of_a_type_that_contains_itself_is_put_on_the_browsers_wire_as_its_nodes",
    ),
    "host": (
        "a_browsers_graph_is_the_tree_its_nodes_make",
        "a_hosts_graph_for_a_browser_is_the_trees_nodes_in_level_order",
        "a_hosts_graphs_inside_another_value_are_each_its_own",
        "a_chain_as_deep_as_its_data_crosses_from_a_browser",
        "a_graph_that_is_not_a_tree_traps_in_the_component",
        "what_is_not_a_graph_is_refused_by_the_host",
    ),
}
BOUND = 1200

def bounded(cmd, **kw):
    """The native status, or None after the shared owner reaps a timeout."""
    try:
        result = producer_process.run(cmd, timeout=BOUND, **kw)
        return result.stdout, result.returncode
    except subprocess.TimeoutExpired as expired:
        return expired.output, None


def cargo_result(suite, code, out):
    result = cargo_assertion_results.validate(code, out, root=ROOT,
        sources=[SOURCES[suite]], expected=NAMES[suite])
    return (True, result.passed, result.failed) if result.valid else (False, 0, 0)


def run():
    """All 25 names, with failures only at actual parent assertion sites."""
    passed = failed = 0
    for suite, cmd in CARGO.items():
        out, code = bounded(cmd, cwd=ROOT)
        print(out, flush=True)
        valid, good, bad = cargo_result(suite, code, out)
        if not valid:
            return False, 0, 0
        passed += good
        failed += bad
    return True, passed, failed


def main():
    import mutation_baseline  # keeps what each command says, for a red baseline
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    watch = mutation_baseline.mutation_bound.start()
    valid, passed, failed = run()
    print(f"baseline: {passed} passed, {failed} failed")
    if not valid or failed or not passed or watch.stopped:
        print("FAIL: the unmutated baseline is not green; no mutant can mean anything")
        mutation_baseline.explain()
        return 1
    killed = refused = survivors = 0
    for what, path, anchor, replacement in MUTANTS:
        original = path.read_text()
        if original.count(anchor) != 1:
            print(f"{what}: REFUSED (anchor not unique in {path.name})")
            refused += 1
            continue
        with mutation_source.changed(path, original, original.replace(anchor, replacement, 1)):
            valid, passed, failed = run()
        if not valid or watch.stopped:
            verdict = "REFUSED (invalid or stopped producer; no kill)"
            refused += 1
        elif failed:
            verdict = f"KILLED ({failed} of {passed + failed} tests fail)"
            killed += 1
        else:
            verdict = "SURVIVED"
            survivors += 1
        print(f"{what}: {verdict}", flush=True)
        if watch.stopped:
            return 1
    valid, passed, failed = run()
    if not valid or failed or not passed or watch.stopped:
        print("REFUSED: restored source does not complete its green inventory")
        return 1
    print(f"{killed} of {len(MUTANTS)} mutants killed; {refused} refused; {survivors} survived")
    return int(bool(refused or survivors))


if __name__ == "__main__":
    sys.exit(main())
