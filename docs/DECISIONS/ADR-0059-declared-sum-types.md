# ADR-0059: declared sum types, typed, built and matched

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, "generic algebraic data types; tagged unions; exhaustive
pattern matching"; §14 M10, compiled pure computation).

## Context

KNOWN_LIMITATIONS said two things:
- **The checker:** "Sum-type variant constructors (`Circle(3)`) have no type,
  because the workspace does not resolve variant names as terms."
- **The backend:** it refused "a declared variant built or matched", by
  name.

Writing this found that the first was wider than it said. Each of these
passed `pw check` on 2026-09-25:
- **A field of the wrong type:** `Shape.Circle("x")`, though `Circle`
  holds an `Int`.
- **A case nothing declares:** `Shape.Bogus(1)`.
- **A pattern through its type matched everything.** `match s {
  Shape.Empty => 0 }`, over a `Shape` of four cases, was proven exhaustive.
  The grammar read `Shape.Empty` as a binding of that dotted name, so the arm
  matched everything. `Shape.Circle(r)` did not parse at all.
- **An arm's bindings were unknown in its body.** `Some(x) => x + "a"`, over
  an `Option<Int>`, passed. The value relations did not bind an arm's names
  while walking its body, so every relation inside an arm read them as
  unknown. The arm's result was checked, and nothing else in it.

Two more were found in the backend's surroundings:
- **A union of one case lost its payload.** `type Only = | Only(Int)` was
  written to WIT as a record of the declaration's fields, which a union has
  none of, so it became `tuple<>`.
- **A type that contains itself** reached the world as WIT that does not
  parse. It was reported as a failure upstream of the backend.

## Decision

### 1. Where a case is written

- **In an expression, through its type:** `Shape.Circle(3)`,
  `Shape.Empty`, and `geometry.Shape.Circle(3)` through a module. The corpus
  writes all three of its constructions so (A-005, A-012,
  `examples/lib/StoreId.pw`). The ownership gate
  (`tests/semantic_ownership.rs`) owns a constructor call by this form.
- **A case without a payload may be written alone**, `Empty`, where exactly
  one sum type the unit sees has a case of that name and no term has it.
  ADR-0047 already resolved such a name, and now it is typed. Where two
  visible types have it, PW0022 says so and names both qualified forms.
  **(ruling needed)**
- **A case with a payload, written alone,** `Circle(3)`, stays PW0021. Its
  repair now names `Shape.Circle(..)`. **(ruling needed)**: the alternative
  is to type a bare call to a case, as a bare case without a payload is
  typed.
- **The language's own cases keep their names.** `Some`, `None`, `Ok` and
  `Err` alone are always the language's. A program's own case of one of
  those names is written through its type.
- **A case with a payload, named as a value,** is the function that builds
  it: `List.map(radii, Shape.Circle)`.
- **In a pattern, a case is written alone, as before, or through its type:**
  `Shape.Circle(r)`, `Shape.Empty`.
  - A dotted name in a pattern is always a constructor pattern, never a
    binding, since a binding is one name. This is a grammar change.
  - The qualifier must name the type matched. `Box.Empty` against a `Shape`
    is PW0608. Read by its last segment alone, it would have matched.

### 2. What is checked

- **Payload types are trees.** `Signatures` records each sum type's cases
  (`TypeDecl::variants`). Each payload field's type is resolved from where
  the declaration is written. `VariantDef.fields` holds type trees, where it
  held spellings that each reader re-parsed, as an opaque type's
  representation did before ADR-0054.
- **A case is a call.** `Shape.Rect(w, h)` is typed as one: its arity
  (PW0604), and each field against its declared type (PW0605). The type's
  parameters are instantiated per use, so `Maybe.Just(1)` is a `Maybe<Int>`.
- **A case its type does not declare is PW0608.** Its invariant broadens to
  "a constructor must name a case of the type it builds or matches".
  Revision 2: the same invariant as a pattern's, now held where a case is
  built.
- **An arm's fields are typed in its body.** `Rect(w, h)` binds each field
  under the scrutinee's type arguments.
- **The value relations walk each arm with its bindings,** for `Some`, `Ok`
  and `Err` as for a declared case.

### 3. How a case is built and matched

**The IR:**
- `Instr::Case { case, fields }` builds a declared sum type's case. A case is
  named by its position in the declaration, which is its discriminant.
- A `MatchArm` takes one case or several (`cases`), and binds its payload's
  fields (`bindings`).
  - `_` or a name takes every case no earlier arm takes.
  - `A | B` takes each alternative's case, binding nothing.
  - Each case is taken by exactly one arm. A case taken twice, and an arm no
    case reaches, are refused rather than compiled.
- Matches over `Option` and `Result` gain the same arms.

**The component:**
- **A sum type is a WIT `variant`.** A case of one field carries it, and a
  case of several carries a `tuple` of them, as `wit.rs` writes the world's.
  A type the world does not name is defined in the private copy of the
  world's `Resolve`, as a record is (ADR-0039 §5).
- **Every layout number is `wit-parser`'s.** The discriminant's width is
  `Variant::tag`: 8 bits up to 256 cases, 16 up to 65,536. The payload's
  offset, and each field's place in a tuple, are `SizeAlign`'s.
- **A match** loads the discriminant once and tests each arm's cases in
  turn. The last arm takes what is left. A variant of two cases with an arm
  each keeps the form every artifact was built with.
- **A variant held flat is matched through memory.** A parameter arrives
  flat, and is written to the invocation region before it is matched. Each
  case's values are read back from the slots the flattening joined them
  into, as the Canonical ABI's `lift_flat_variant` reads them.
- **A variant read from memory into flat values** widens each case's values
  into their joined slots, as `lower_flat_variant` does. It is read so for a
  host call's argument.
- Until now both directions were refused, so an `Option` parameter could not
  be matched either.

**The JavaScript module:** a case is `{ $case: "<its WIT name>", value }`. A
case of several fields holds them in an array, as a tuple is held.

### 4. Refused by name

Superseded in part: a generic sum type compiles since ADR-0062, a nested or
literal pattern since ADR-0060, and a template's `{#match}` takes a sum type
apart since ADR-0061.

- **A generic sum type in the backend.** A `Type::Nominal` carries no type
  arguments, so the layout is unknown. Generic records share this
  limitation.
- **A type that contains itself.** The Canonical ABI has no recursive
  types, and this backend lays every value out by its type. The refusal is
  where a type first meets the backend.
- **Unchanged:**
  - a nested or literal pattern;
  - a sum type in a template's `{#match}` (PW5019);
  - a captured sum-type value in a handler, since the document carries none.
- **`==` on a sum type, or on a record.** The backends compare primitives
  only. Listed in KNOWN_LIMITATIONS since this ADR, though it predates it.

**(ruling needed)**: an arm that no case reaches is refused by the backend.
The checker computes such arms (`exhaust::MatchReport::unreachable`) and
does not report them. So a match with a `_` after every case checks, and
does not compile.

## Acceptance

- **`compiler/pw-core/tests/sum_types.rs`**, 10 tests, each with a control:
  - a case's fields and arity;
  - a case its type lacks;
  - a case as its type's value and as a function;
  - a bare case, and one two types declare;
  - a bare case with a payload;
  - a pattern through its type;
  - a qualifier naming another type;
  - an arm's fields;
  - an option's payload in its arm;
  - a generic sum type.
- **`compiler/pw-conformance/tests/sum_types.rs`**, 12 tests through the E8
  host against a model in Rust:
  - each case built;
  - a parameter matched case by case, and returned as itself;
  - a case as a function value, and a bare case;
  - a case inside a record and a list;
  - every joined slot: an `Int`, a `Float`, a `String`, a `Bool` and a pair;
  - a case inside an `Option`;
  - a host's answer matched by case;
  - a 300-case type's 16-bit discriminant;
  - a union of one case;
  - the two refusals.
- **`compiler/pw-conformance/tests/javascript.rs`:** nine new queries, over
  `Shape` and `Mixed`. The component and the module agree on each of 200
  generated calls.
- **The store's and kiokun's artifacts are byte-identical** to the build
  before this change. The two store handlers differ from that build by
  ADR-0058's change, and equal their committed modules.
- **Mutation controls:** `scripts/sum_type_mutations.py`,
  `just e10-sum-types`, 20 mutants.

## Not done

- Nested and literal patterns.
- A generic sum type, or a generic record, in the backend.
- A type that contains itself.
- A declared sum type in a template's `{#match}`.
- Reporting an arm no case reaches, in the checker.
- Structural equality.
