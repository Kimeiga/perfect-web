# ADR-0205: a value of a type that contains itself crosses the browser's wire as its nodes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-05.
Milestone: E14, the app layer the owner put before the AI benchmark. Its
first item's browser wire, ADR-0194's next step.

## Context

- **The browser's wire refused such a value by name** (ADR-0194 §6).
  - The JavaScript module writes its decoders and encoders out by a type's
    shape, which for a type that contains itself never ends. A handler that
    set a signal to one, or sent one to a command, was refused.
  - The host refused one as a browser's argument.
- **A signal's first value was written nested.** The browser's renderer
  reads its requests with serde_json, which parses at most 128 nested
  values: a thread about 60 levels deep (ADR-0203).
- **Lifting serde_json's limit moves the overflow.** Its documentation says
  `disable_recursion_limit` parses "without any consideration for
  overflowing the stack". It points to `serde_stacker`, and to the `Drop`
  and `Debug` of the result.
- **The renderer's values recursed too.** `Value` was dropped, cloned and
  compared by derived code, one call per level.
- **A component already reads such a value as its nodes** (ADR-0194): level
  order, node 0 the value, each node's slots the next run of indices.

## Decision

### 1. The wire form

- **A value of such a type is `{ "$graph": [node, ...] }`.** Node 0 is the
  value. Inside a node, each value of the type is `{ "$node": k }`, for a
  later node `k`.
- **Each node but the first is held by exactly one.** The nodes are a tree,
  and a reader refuses any that are not.
- **In ADR-0194's level order.** Each node's own values of the type, in the
  order its fields are declared, take the next run of nodes. A component's
  decoder reads the same indices.
- **The JSON is as shallow as the type**, and the value as deep as its data.
- **One graph per value of the type.** A list of comments is a list of
  graphs. Another such type inside a node is a graph of its own.

### 2. Who writes it

- **The build**, a signal's first value.
- **A handler**, a signal's value it sets and a command's argument it
  sends. Its module writes each graph with a queue, generated for the type.

### 3. Who reads it

- **A handler**, a signal's value, from the last node to the first. A graph
  that is not a tree traps.
- **The renderer** (`Value::from_wire`), any graph, knowing no type: it is
  self-describing. One that is not a tree is refused, and a request holding
  one is malformed.
- **The host**, a browser's argument: each `$node` is the index the
  component's node holds there. The component checks the level order, as it
  checks a host's nodes.

### 4. The renderer's values

- **Read, dropped, cloned and compared with a stack on the heap**, not by
  recursion. A value 100,000 deep is all four on a 256 KiB thread, and so
  is nested JSON 50,000 deep, read.
- **No longer moved out of by a pattern**, since `Value` implements `Drop`.
  A caller changes one in place, through `&mut`.

### 5. Still refused, by name

- **What a host writes, knowing no type**: a handler's capture, which the
  renderer writes; a command's declared error, which the server writes; a
  speculation's value, which the server sends.
- **Two types that hold each other**, as at a component's boundary
  (ADR-0202 §4).
- **A case of such a type as a browser's argument**: the host takes no case
  from a browser.

## Alternatives

- **Nested JSON, with serde_json's limit lifted.** See above: the parse,
  and what is done with its result, recurse with the data.
- **Nested JSON, with a parser of our own that does not recurse.** Two JSON
  parsers, and the value is still as deep in Rust.
- **The component's nodes, plain indices.** The renderer reads values
  knowing no type, and could not tell an index from an `Int`.
- **References by path, as JSON Pointer writes them.** A path allows two
  parents and cycles, and a tree is what the type promises.

## Acceptance

Recorded by `just e14-graphs-on-the-wire` in
`docs/evidence/E14/graphs-on-the-wire.txt`:

- **`runtime/pw-render/tests/graphs.rs`**, 6 tests:
  - a graph is the value its nested JSON is: a list's, a case's, and
    another type's graph inside a node;
  - one that is not a tree is refused, each way;
  - a value 100,000 deep read, cloned, compared and dropped on 256 KiB;
  - a copy is equal, ordered and its own, and each kind compares;
  - nested JSON 50,000 deep read on 256 KiB;
  - each item and field read into its place.
- **`compiler/pw-core/tests/graphs_on_the_wire.rs`**, 6 tests, each module
  run under Node:
  - the build's first value, read back by the renderer;
  - a handler that reads a signal from its nodes and writes it wrapped,
    20,000 deep;
  - one that is not a tree trapping;
  - a handler sending one to a command;
  - two fields that hold the type, numbered in the order they are
    declared;
  - the refusals that stand.
- **`compiler/pw-conformance/tests/browser_graphs.rs`**, 4 tests, through
  the host and a component the compiler built: random trees against a
  model, a chain 50,000 deep, graphs that are not trees trapping in the
  component, and what is not a graph refused by the host.
- **`compiler/pw-core/tests/recursive_types.rs`**: its test of the wire's
  refusal now accepts the value as its nodes.
- **`spikes/own-renderer/e2e/thread.spec.mjs`**, in three engines: a
  handler adds a heading to the outline a signal holds, the browser renders
  it, and a command is sent the outline, as its nodes.
- **`scripts/graphs_on_the_wire_mutations.py`**: 19 mutants.
- **`recursive_types_mutations.py`**: "the browser's wire writes such a value
  out by its shape" retired, as the refusal it controlled is gone.

## Not claimed

- **A query's value deeper than its host nests** (`NESTED_DEPTH`, 128). The
  server still renders a query's value nested from its host.
- **What §5 refuses.**
- **A component's own recursion over a deep value**, which traps
  (ADR-0050).
- **A deep value printed with `{:?}`**, which recurses.
