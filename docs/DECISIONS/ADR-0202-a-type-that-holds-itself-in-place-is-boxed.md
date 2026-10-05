# ADR-0202: a type that holds itself in place is boxed, and crosses as its nodes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-05.
Milestone: E14, the app layer the owner put before the AI benchmark. Its
first item, types that contain themselves, continued from ADR-0194.

## Context

- **ADR-0194 compiled a type that contains itself through a list.** A
  list's elements are where the list points, so `replies: List<Comment>` has
  a layout by its type.
- **A type that holds itself in place had none.** `next: Option<Node>` and
  `Add(Expr, Expr)` hold a value of their own type with no list between, and
  a layout by type would be infinite. ADR-0194 refused one by name, twice:
  in the lowering, and where the WIT is written. Linked lists, expression
  trees and parse trees could not be written.
- **Two ways to give such a type a layout:**
  - box every value of it: each value is the address of its cell, as OCaml
    and Haskell hold every value of a recursive type;
  - box only the edges that close a cycle, as Rust requires
    `next: Option<Box<Node>>`.
- **The encoder already builds each record and case in memory.** Its value
  is held by its address while the body runs. So a box is the address the
  encoder already has, given a type of its own.

## Decision

### 1. Which types are boxed

A declared type's instance is boxed where a cycle of the declarations'
shapes runs through it with no list, map, set or function value between:
- through a field;
- through a case's payload;
- through an `Option` or a `Result`;
- through an opaque type's representation.

Two types that hold each other this way are both boxed. A type recursive only
through a list keeps ADR-0194's layout.

### 2. A value of a boxed type is the address of its cell

- **Its WIT type is a `u32` of its own**, an alias named for the type. The
  alias is allocated before the cell is defined, so the cell's parts that
  hold the type are the box.
- **The cell is the record or the variant the type is.** `Option<Node>` is an
  option of a box, a `List<Node>` a list of boxes, and `Add(Expr, Expr)` a
  case holding two.
- **The value is built in its cell**, as every record and case already is.
  The value is the cell's address.
- **It is read and matched through its cell.** A box held as a word is its
  cell's address; one held in memory is read from there first.
- **Nothing in the IR changes.** A `Node` is a `Node`. The lowering's refusal
  by name is gone, and boxing is the encoder's alone.

### 3. At a boundary, it crosses as its nodes

ADR-0194's encoding extends:
- **A node's slots** are a `list<u32>` for a list of the type, a `u32` for
  the type held in place, and an `option<u32>` for an option of it. So
  `record node-node { value: s64, next: option<u32> }`, and
  `variant expr-node { num(s64), add(tuple<u32, u32>) }`.
- **Level order, as before.** A node's slots, in order, take the next run of
  indices: a list its length, a box one, an option one or none. Node 0 is the
  value.
- **The encoder copies each box's cell** after the nodes so far, and writes
  its index where the address was. A list of boxes is copied cell by cell.
  Nothing recurses.
- **The decoder checks each index as ADR-0194's does.** Each must continue
  the run, after its own node, and the runs must end at the last node.
  Otherwise it traps. It then writes each index's cell's address where the
  index was: in place, and nothing copied.
- **The host's slots are the same three**, and its checks the same.

### 4. Still refused where the WIT is written, by name

- a type held deeper: `Option<List<T>>`, `List<Option<T>>`, a `Map` of it;
- through another declaration, `Even` holding an `Option<Odd>` and `Odd` an
  `Even`. Both compile inside a component;
- while holding another type that contains itself;
- an opaque type that contains itself;
- a generic one, `Tree<Int>`, as no generic type crosses yet (ADR-0062). It
  compiles inside a component.

## Alternatives

- **Box the edges only, as Rust's `Box` does.** A `Node` would be its fields
  where it is held, and a pointer only where it holds itself. Every place
  that moves a value between a held `Node` and a boxed one would convert it,
  in the IR or in the encoder. Boxing every value gives one representation
  everywhere, and the encoder already holds records by address.
- **Require the program to write `Box<T>`.** A word for the program to
  learn, for a layout the compiler can decide.
- **Refuse it, and write lists of length one.** A program would encode a
  linked list as `List<Node>`, with no type saying there is at most one.

## Acceptance

Recorded by `just e14-boxed-types` in `docs/evidence/E14/boxed-types.txt`:

- **`compiler/pw-core/tests/recursive_types.rs`:**
  - a boxed record's and a boxed variant's nodes in the WIT, read back by
    `wit-parser`;
  - the refusals that stand;
  - a type held in place compiled, where the lowering refused it.
- **`compiler/pw-conformance/tests/boxed_types.rs`**, 7 tests, through the
  E8 host against a Rust model:
  - a linked list and an expression tree built and walked in a component;
  - both crossing both ways, given and returned unchanged;
  - a chain 50,000 deep crossing as its nodes both ways;
  - malformed nodes trapping in the component and refused by the host;
  - two types holding each other, and a generic type, inside a component;
  - a list of boxes crossing;
  - a data layer answering with a chain and given one.
- **`compiler/pw-conformance/tests/javascript.rs`:** five queries over
  values held in place, each module agreeing with its component.
- **`compiler/pw-conformance/tests/sum_types.rs`'s
  `a_sum_type_that_holds_itself_in_place_is_built`**, which refused `Chain`
  by name until now.
- **ADR-0194's tests pass unchanged.** The store's build is byte-identical.
- **`scripts/boxed_types_mutations.py`**: 17 mutants.
- **Mutation scripts that moved:**
  - re-anchored: `generic_mutations.py`'s "an instance inside another is
    taken for recursion" and `recursive_types_mutations.py`'s "a node's list
    of children is written as a list of the type";
  - retired: three of `recursive_types_mutations.py`'s, which controlled the
    refusals this ADR removes.

## Not claimed

- Crossing for the types §4 refuses.
- The browser's wire, and a view that contains itself (ADR-0194's next
  steps).
- A recursive function's stack: a component's own recursion over a deep
  value traps, as ADR-0050 records.
