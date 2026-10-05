# ADR-0194: a type that contains itself compiles, and crosses a boundary as its nodes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-05.
Milestone: E14, the app layer the owner put before the AI benchmark on
2026-10-04. Its first item: types that contain themselves, lifting
ADR-0059's refusal.

## Context

- **Apps are full of trees.** Examples: a reply thread, a folder of folders,
  a menu's sections, a JSON value. A program could declare
  `type Comment = Comment { text: String, replies: List<Comment> }` and the
  checker accepted it, but nothing compiled. ADR-0059 §4 refused any type
  that contains itself where it first met the backend: "the Canonical ABI has
  no recursive types, and this backend lays every value out by its type".
- **The owner asked for the refusal to be lifted**, not worked around with
  lists of parent ids. The suggested route had four steps:
  1. an indirect layout inside a component;
  2. a list of nodes and indices at the WIT boundary, rebuilt on the other
     side;
  3. a browser wire encoding;
  4. views that contain themselves, with addresses qualified by instance.
- **The component model has no recursive types.**
  WebAssembly/component-model#56 has been open since 2022.
  - Luke Wagner, 2022: they would make canonical adapters "a lot more
    complicated".
  - Luke Wagner, 2023, named two hazards: lifted memory encoding a cycle, and
    stack-based recursion overflowing in the fused lift and lower.
  - What people do instead (esoterra, 2023): "indices into a list as
    indirection".
  - Its people are on preview 3 (Luke Wagner, 2025).
- **The checker accepted types no value has**, such as
  `type Loop = Loop { again: Loop }`.
- **Depth limits elsewhere:**
  - serde_json 1.0.151, the version the workspace locks, stops parsing at
    128 levels (its `remaining_depth: 128`).
  - Blink's HTML parser nests no deeper than 512 open elements
    (`kMaximumHTMLParserDOMTreeDepth`, `html_construction_site.h`). Past
    that, it attaches an element as a sibling of its parent.

## Decision

### 1. A type no finite value has is refused where it is declared (PW0624)

A value is built from values that exist already. So a type every value of
which must hold another value no finite one fills has no value at all.

The checker computes which declared types have a value as a least fixed
point:
- a record has one when every field has one;
- a sum type has one when some case's fields all have one;
- `List`, `Map`, `Set`, `Option` and a function always have one;
- `Result` has one when either side does;
- a generic declaration is asked under its arguments, each taken only as
  having a value or not. So the states are finitely many, and the iteration
  ends.

What it decides:
- **Refused:** `Loop`, `Wrap(Wrap)`, a `Boxed` holding a `Box<Boxed>`, two
  records that hold each other, and an opaque type over one of these.
- **Accepted:** a list or option of itself, a case without it, and a
  `Maybe<Loop>`, which is `Nothing`.

### 2. Inside a component, through a list, it is laid out by its type

**A list's elements are where the list points.** So `replies:
List<Comment>` is 8 bytes of a 16-byte `Comment`, under the Canonical ABI's
own layout rules, and so is recursion through a `Map` or a `Set`. Two
records that hold each other through a list compile too.

**The component's private `Resolve` holds the cycle.** A list waiting on a
type being defined is allocated first with a placeholder element. It is
given its element once that type is defined. So `SizeAlign` lays out each
type after the types it reads. A list whose element never resolves is left
out, with every type holding it, and the encoder refuses whatever needs
one.

**Held in place, it is refused by name** until the backend boxes the value.
`Option<Node>` and `Add(Expr, Expr)` hold the type in place. Two refusals
stand: one where the WIT is written, and one in the lowering for a type no
signature names.

### 3. At a boundary, it crosses as its nodes

- **WIT.** `type m-comment = list<m-comment-node>`. The node has the
  type's fields as written, except that each `List` of itself is a
  `list<u32>` of node indices. Node 0 is the value. The nodes are in level
  order: each node's lists hold, in order, the next run of indices, so all
  the lists together hold `1, 2, …, n − 1`.
- **Why this encoding:**
  - **Canonical.** A value has one encoding, so equal values pass equal
    bytes, which a cache key or a comparison of results needs.
  - **Checked in one pass** with a counter, and no set of visited nodes.
  - **Encoded with no recursion.** The node list is its own queue: node `i`
    is a copy of a value whose lists still point at its children. Those
    children are copied after the nodes so far, one copy per list, since a
    list's elements lie one after another.
  - **Decoded in place, with nothing copied.** In level order a node's
    children lie one after another among the nodes, and a value's list of
    its type is exactly that. Each list is rewritten to point there.
- **The component's decoder traps unless the nodes are one tree.** Every
  list must continue the run of indices, from 1, each run after its own
  node, and the runs must end at the last node. Then:
  - no node holds one before it, so there is no cycle (the first hazard);
  - none is shared, so a value cannot grow exponentially when it is read;
  - none is left over.

  Neither side recurses on the value (the second hazard): a chain 50,000
  deep crosses both ways.
- **A `u32` in a world is always a node index.** No Pleris type is one, and
  a function value never crosses. The encoder and the host find a value's
  nodes by that.
- **Whatever holds such a type is converted** at all four points: an
  export's parameter and result, a host call's argument and answer. That
  covers lists, options, results, records and variants that hold one. A
  value is built in the body's own layout, and the world's type is never
  pushed onto it.
- **Refused by name where the WIT is written**, for a type that contains
  itself:
  - in place;
  - deeper in a field (`List<List<T>>`, `Map<K, T>`);
  - through another declaration;
  - while holding another type that contains itself;
  - when the type is opaque.

  A node holds only lists of its own type yet.

### 4. A host holds the value nested

`pw-host` converts at the boundary, by the types the component declares:
- it makes a nested argument its nodes;
- it gives a data layer its arguments nested;
- it makes a data layer's nested answer its nodes, then projects it
  (ADR-0166) and holds it to its invariants (ADR-0179);
- `Prepared::untangled` reads an export's results nested.

Its decoder makes the same checks as the component's.

**It nests no deeper than 128** (`NESTED_DEPTH`), serde_json's own default.
A nested value is cloned, compared, printed and dropped by recursion. In a
debug build, where a level costs most, a clone or a comparison overflows
half of a 2 MiB thread stack near 330 levels. A host that needs a deeper
value reads its nodes, which have no depth to overflow.

### 5. An invariant inside a tree holds at every node

The contract's path goes through every node at once: `["*", "likes"]`. So a
data layer's answer with a bad count deep in a thread is refused like one
at its root. ADR-0179's walk followed a type that contains itself once, so
it would have checked the root alone.

### 6. The browser's wire does not carry one yet

The JavaScript module writes its decoders and encoders out by a type's
shape. For a type that contains itself they would never end, so each is
refused by name: a handler capturing one, a signal set to one, a command
sent one. The host refuses one as a browser's argument. The module's own
values are nested, as JavaScript's are, so its queries agree with the
components.

## Alternatives

- **Lists of parent ids.** `Comment { id, parent: Option<CommentId> }` in a
  flat list. The type no longer says it is a tree, every reader rebuilds
  it, and a cycle is a runtime bug. The owner ruled it out.
- **Wait for the component model.** #56 has been open four years, and preview
  3 comes first.
- **A resource (handle) per node.** A host call per node read. Luke Wagner
  suggests it for a DOM, which lives on one side and is mutated. A reply
  thread is data, and data crosses as data.
- **Serialize to bytes.** The WIT would say `list<u8>`, and nothing would
  check the shape. The host would parse it with a depth limit.
- **Pre-order, with child indices.** Natural for printing. But a node's
  children's indices depend on its earlier siblings' subtree sizes, so
  encoding needs a size pass or patches, and checking needs a stack.
- **An explicit `root` field.** A degree of freedom with nothing to choose.
  It allows nodes no root reaches, and two encodings of one value.
- **Box every value of such a type.** One allocation per node, and a list of
  them becomes an array of pointers. Through a list, the layout needs
  nothing.
- **Nested JSON on the browser's wire, now.** The browser's renderer
  (`pw-render-wasm`) parses with serde_json. A thread 40 levels deep nests
  past its 128, since each level is an object and an array. The wire needs
  its own encoding, which is the next step.

## Acceptance

Recorded by `just e14-recursive-types` in
`docs/evidence/E14/recursive-types.txt`:

- **`compiler/pw-core/tests/recursive_types.rs`**, 7 tests:
  - PW0624 on five types with no value, and on none of six controls;
  - the WIT a record and a sum type cross as, read back by `wit-parser`;
  - each refusal where the WIT is written, by name;
  - the lowering's refusal of a type held in place, by name;
  - an invariant's path through every node;
  - the browser's wire refusing a handler's value by name.
- **`compiler/pw-conformance/tests/recursive_types.rs`**, 10 tests, through
  the E8 host against a Rust model:
  - random trees built, walked and rebuilt;
  - two types that hold each other through a list, built and walked in a
    component;
  - the exact level-order nodes, and nodes in and out unchanged;
  - lists, options, results and records of trees, both ways;
  - a sum type that contains itself, with a tuple payload;
  - a host answering with a tree and given one;
  - a chain 50,000 deep, both ways;
  - ten malformed node lists, each trapping in the component and refused by
    the host;
  - a value at the host's bound, cloned, compared, printed and dropped
    within half a thread stack.
- **`compiler/pw-conformance/tests/javascript.rs`**: five queries over trees,
  each module agreeing with its component under Node.
- **`scripts/recursive_types_mutations.py`**: 21 mutants.
- **Every other program is unchanged.** The store's 66 built artifacts are
  byte-identical to `9e53158`'s.
- **Mutation scripts that moved:**
  - re-anchored: `descriptions_mutations.py` and `wit_name_mutations.py`;
  - retired: `sum_type_mutations.py`'s "a type that contains itself
    reaches the world". One refusal became two, and that script's tests,
    which hold the type in a signature, pass with either undone. Each layer
    is a mutant of this ADR's script, against a test that tells them apart.

## Not claimed

- **A type that holds itself in place**, which needs a box: `Option<Node>`,
  `Add(Expr, Expr)`.
- **Crossing a boundary** for mutual recursion, a list of lists of itself,
  a node holding another such type, or an opaque type that contains itself.
- **The browser's wire.**
- **A view that contains itself.** PW5020 still refuses one, so a page shows
  a tree to the depth its template writes out.
- **A recursive function's stack.** A component's own recursion over a deep
  tree overflows wasm's stack and traps (ADR-0050). Only the boundary is
  free of depth.
