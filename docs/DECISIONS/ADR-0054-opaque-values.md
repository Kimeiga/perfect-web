# ADR-0054: an opaque value is built and read inside a component

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 1, own backends).

## Context

An opaque type is a new type over a representation:
`opaque type Count = Int`. ADR-0048 made its representation readable as
`.value`, in the module that declares it only. Opaque values crossed the
component boundary as their representations. Inside a component, both
directions were refused by name:
- `Count(n)` named no declaration the backend could call, since the type is
  in the type namespace;
- `c.value` was not a record's field.

KNOWN_LIMITATIONS listed it ("An opaque value is not built or read inside a
component").

Writing the tests found two defects on the way:

- **The checker typed `fold`'s accumulator as the list's element**
  (ADR-0053). A fold over a list of opaque values was refused.
- **A representation with type arguments never resolved.** Lowering kept the
  representation as a spelling, and `signatures.rs` resolved only a spelling
  without `<`. So `opaque type Names = List<String>` had no representation,
  and `n.value` was refused in its own module as a member `Names` did not
  have (PW0610). `wit.rs` re-parsed the same spelling to emit the alias.

## Decision

### 1. An opaque type is its representation, retyped

`Instr::Retype { result, value, ty }` is the same value under another type.
The bits do not change.
- **`Count(n)`** lowers `n` against the representation's type, then retypes
  it to the opaque type. A call whose callee resolves to no term is looked
  up as a type; an opaque type there builds its value. So does
  `n |> Count()`.
- **`c.value`** retypes `c` to its representation. The checker allows it
  only in the declaring module (ADR-0048).

In the component, the retyped value keeps its locals, or its address in the
region. An opaque type's component type is an alias of its
representation's, so the layout is the same; a retype between two layouts
is blocked. In the JavaScript module the value is the same value.

**(ruling needed)**: building an opaque value checks nothing. The language
states no invariant for an opaque type (KNOWN_LIMITATIONS, "Opaque
invariants are not checked at the boundary"), and a construction inside the
declaring module is no different.

### 2. A representation is a type tree

`Decl::opaque_of` is a `DeclaredType`, as ADR-0028 made parameters, fields
and results. The checker's signatures and the WIT alias both resolve that
tree; neither re-parses a spelling.

## Acceptance

- `compiler/pw-conformance/tests/opaque.rs`, through the E8 host:
  - an `Int`, a `String`, a record, a list and an `Option` as
    representations;
  - an opaque value built by a call, in a pipe, and in a loop;
  - an opaque value folded, passed to a recursion, and captured by a
    closure.
- `compiler/pw-conformance/tests/javascript.rs`: four opaque queries more.
  The component and the module agree on 53 queries and 10,600 calls.
- `compiler/pw-core/tests/members.rs`: `.value` of an opaque type over
  `List<String>` reads as the list, in its module only. It was PW0610.
- The store's and kiokun's 19 artifacts are byte-identical to `fd95b59`'s.
- Mutation controls: `scripts/opaque_mutations.py`, `just e10-opaque`,
  7 mutants.

## Not done

- **A generic opaque type** (`opaque type Bag<T> = List<T>`) is refused
  inside a component, as a generic record is: `Type::Nominal` names a
  declaration without its arguments.
- **An opaque type's invariant.** None is stated, so none is checked.
