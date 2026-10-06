# ADR-0233: a speculation on a value of a type that contains itself

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-06.
Milestone: E14. Lifts the speculation bullet of ADR-0205 §5, which an
optimistic reply on the feed's thread page needs: a thread is a `Post`, which
holds its replies.

## Context

- **A value of a type that contains itself crosses the browser's wire as
  its nodes** (ADR-0205): `{ "$graph": [node, ...] }`, each node's own
  values of the type `{ "$node": k }`, in level order.
- **§5 refused what a host writes knowing no type.** One of those was a
  speculation's value.
  - The server sends each value a page speculates on (ADR-0222). It wrote
    the value nested, by `val_to_json`, which knows no type.
  - The page's speculation module decodes it. A decoder written out by such
    a type's shape never ends (ADR-0194), so the module was refused: "a
    speculated value the server cannot send".
- **The host knows the type.** A query's value comes from its component,
  whose export declares its result type. The host already makes a nested
  value its nodes, to pass it to a component (`tangle`, ADR-0194). And it
  reads a browser's graph by that type (`from_browser`, ADR-0205).

## Decision

1. **The server writes a value a page speculates on by its query's
   declared result type.** For a `Result`, by its `Ok`, as a page's binding
   holds it. It is written in the document's speculated entries and in each
   frame that changes one.
2. **A value of a type that contains itself is written as its nodes**, the
   inverse of `from_browser`:
   - in level order, each node's children the next run of indices, as the
     host reads them;
   - each child `{ "$node": k }`, a list of them, or a case holding one;
   - each other field as before, and each value of such a type inside it a
     graph of its own: an option's, each item of a list's, a case's.
3. **Every other value is written as before**: a record by its fields'
   names, a case `{ "$case": name, "value": payload }`.
4. **The page's module reads the value from its nodes**, as a handler reads
   a signal's (ADR-0205): from the last node to the first. A graph that is
   not a tree traps, and so does a value sent nested.
5. **Still refused, by name**, as ADR-0205 §5 leaves them:
   - a handler's capture, which the renderer writes knowing no type;
   - a command's declared error, which the server writes so;
   - two types that hold each other;
   - a case of such a type as a browser's argument.

## Found

- **A case's payload written knowing no type survived its mutant** in the
  first run. The only case held a string, which is written the same either
  way. The test's `Tag` holds a comment in a case too now, a graph, and the
  mutant is killed.

## Acceptance

- **`compiler/pw-core/tests/speculated_graphs.rs`, 2 tests**, each module
  run under Node:
  - a session's thread, sent as its nodes, read nested; the text and the
    reply count computed from it; the reply a press speculates last among
    its replies, and counted;
  - a thread sent nested traps, and so does a node held twice.
- **`compiler/pw-conformance/tests/browser_graphs.rs`, 2 tests**, through
  components the compiler built:
  - a comment a component returns, as the host writes it for a browser, is
    the graph a model writes independently, for 100 random trees. The host
    reads that graph again.
  - each such value inside another is a graph of its own: an option's, a
    case's payload, each item of a list's. Around them, a case and a result
    are written as before.
- **The development server's test**
  `a_value_that_contains_itself_is_written_as_its_nodes`: the thread page's
  `thread`, three posts in level order, each node's replies by index. The
  timeline's rows are written as before.
- **`scripts/speculated_graphs_mutations.py`: 8 mutants**, recorded by `just
  e14-speculated-graphs`.
- **Two older mutants re-anchored**:
  - `graphs_on_the_wire_mutations.py`'s "the host reads a node's slots as
    any field", whose line the writer repeats;
  - `optimistic_posts_mutations.py`'s "what a page shows holds no value it
    speculates on", the value written by its query's type.
- **The workspace, 2,108 tests; the browser suite, 773 in three engines.**

## Not claimed

- **A page that speculates on one keyed by its parameter.** The feed's
  thread page binds `Thread(id)`. A speculation's key matches a page's
  binding by an invocation-context call alone until ruling 0122-d, next.
- **In browsers.** The feed has no page that speculates on such a value
  until ruling 0122-d; its browser test is that ADR's.
