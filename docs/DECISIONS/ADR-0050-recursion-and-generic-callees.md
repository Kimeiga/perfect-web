# ADR-0050: recursion and generic callees compile

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 1, own backends).

## Context

ADR-0039 §4 compiled a call to another declaration by inlining it, so every
component exports one function and imports only the host. Two kinds of call
were refused by name, in both the Wasm component backend and the JavaScript
modules (ADR-0044):
- a recursive call, because an inlined recursion has no end the compiler can
  see;
- a call to a generic declaration, because nothing specialized it.

KNOWN_LIMITATIONS listed both, and "no recursion" was one of the gaps named
on 2026-09-25.

## Decision

### 1. A recursive call is a call to a function compiled beside the export

A call is still inlined when it can be. When the callee is already being
lowered, directly or through the declarations it inlines, the lowering
compiles it as its own function instead, once per instance, and emits a
`Call`. The exported function's IR carries these as
[`Function::callees`](../../compiler/pw-core/src/backend/ir.rs): every
instance it reaches, each once.

Inlining stays the default, so a program with no recursion compiles as it
did. At `fd95b59`, the store's and kiokun's 19 built artifacts are
byte-identical before and after. **(ruling needed)**: a callee is inlined
until it recurses, and only the recursion is a call, rather than every
declaration being a function of its component.

### 2. A generic callee is instantiated from its arguments

The lowering binds each type parameter the callee declares to the type its
argument has there. It uses the result's expected type for a parameter that
only the result mentions. It then lowers the body under that binding: a `T`
in the body is the type it was bound to. An inlined generic callee is
specialized where it is inlined, and a recursive one is compiled once per
instance. `count_from<Int>` and `count_from<Word>` are two functions. A
parameter that nothing binds is refused by name.

Generic *records* are not instantiated. `Type::Nominal` names a declaration
without arguments, so `Box<Int>` is still refused.

### 3. How a function compiled beside the export passes values

In the Wasm component:
- the functions follow the export in the module, then the string helpers;
- a primitive, a string or a list is passed as its flat values;
- a record or a variant is passed as a pointer to its canonical layout in
  the invocation region, where one built in a body already is. The encoder
  writes a variant's joined flat slots, but does not read them back from
  memory;
- results are passed the same way.

There is no flat-parameter limit inside a component.

In a JavaScript module, each is a module-level function.

## Acceptance

- `compiler/pw-conformance/tests/recursion.rs`, through the E8 host against
  Rust:
  - factorial and Fibonacci;
  - mutual recursion;
  - a generic recursion at two instances;
  - a recursive export;
  - recursions building a string and an `Option`;
  - an overflow at depth, which traps.
- `compiler/pw-conformance/tests/javascript.rs`: five recursive queries more.
  The component and the module agree on 39 queries and 7,800 calls.
- `compiler/pw-conformance/tests/computation.rs`: the recursion and the
  generic callee it refused now compile and run.
- The store's and kiokun's built artifacts are byte-identical to `fd95b59`'s.
- Mutation controls: `scripts/recursion_mutations.py`, `just e10-recursion`.

## Not done

- **An early `return`, `?`, and `for` loops** are still refused. They are
  the next part of the same instruction.
- **A function value** and **a generic record** are still refused.
