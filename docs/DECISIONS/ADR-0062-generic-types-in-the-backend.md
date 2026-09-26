# ADR-0062: generic types in the backend

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, "generic algebraic data types"; §14 M10).

## Context

KNOWN_LIMITATIONS said: "A generic record or sum type is not instantiated
(ADR-0050, ADR-0059). `Box<Int>` and `Maybe<Int>` have no layout:
`Type::Nominal` names a declaration without its arguments." ADR-0054 refused
a generic opaque type inside a component for the same reason. The checker
typed every generic declaration since ADR-0050. The backend refused each by
name: "an unspecialized generic type", "building a value of a generic
record", "a generic sum type".

Writing the tests for this found a bug in the generated worlds. A type and
a query of one name (a type `Either` and a query `Either`, in their two
namespaces) collided where the worlds are built. The type took the query's
place, so the query had no signature and the whole package failed: every
component of the program was refused.

## Decision

### 1. A nominal type carries its arguments

- **`Type::Nominal(DefId, Vec<Type>)`:** the declaration, and the types it
  is applied to. The list is empty for a declaration without parameters,
  so every existing type is what it was.
- **`TypeDef` is per instance:** the declaration, its arguments, and its
  shape with every field, case and representation under them. `Box<Int>`
  and `Box<String>` are two definitions.

### 2. An instance's arguments come from its fields or its use

- **What a field fixes.** A generic record, case or opaque value is built
  field by field, as a generic callee's arguments are (ADR-0050). A field
  whose declared type mentions a parameter nothing has fixed yet is lowered
  alone, and its type fixes that parameter. `Box { value: 1, .. }` is a
  `Box<Int>`.
- **What the use names.** The instance the context names is read first. In
  `fn f() -> Either<Int, String> { Either.Left(1) }`, the `String` comes
  from the declared result.
- **A case without a payload** takes its instance from its use:
  `Maybe.Nothing` where a `Maybe<Int>` is wanted.
- **A case passed as a function** takes it from the elements:
  `List.map(xs, Maybe.Just)` over a `List<Int>`.
- **A parameter nothing fixes is refused by name:** "a type parameter no
  use instantiates". `let m = Maybe.Nothing` alone is refused.
- **A field read** is typed under its record's instance, and so is a
  match's case under the scrutinee's instance.

### 3. Each backend lays out an instance

- **The component** defines each instance in the private copy of the
  world's `Resolve`, as it defines any type the world does not name
  (ADR-0039 §5). Its recursion guard is keyed by instance, so `Box<Box<Int>>`
  is two instances, not a type containing itself.
- **The JavaScript module** finds a shape by instance too. A record's field
  names and a case's names are its declaration's, so the module's text does
  not change by instance.

### 4. At the boundary, unchanged

A generic type in an exported signature still has no WIT form: "`Box<Int>`
at `boxed` has no WIT form", as before. A phantom parameter, one the layout
does not mention, is one WIT type, as `wit.rs` decided. **Not done:** a WIT
type per instance at the boundary.

### 5. The world keeps a query beside a type of its name

The declarations a component is looked up among are no longer types. A type
is never a component.

## Acceptance

- **`compiler/pw-conformance/tests/generics.rs`,** 7 tests through the E8
  host:
  - a generic record built and read;
  - two instances of one record in one body;
  - a generic sum type built and matched;
  - `Box<Box<Int>>`;
  - a generic opaque type;
  - a generic case as a function, and records built in a lambda;
  - the refusal of a parameter nothing fixes.
- **`compiler/pw-conformance/tests/wit_names.rs`:** a type and a query of
  one name.
- **`compiler/pw-conformance/tests/javascript.rs`:** two generic queries.
  All 86 queries' component and module agree on 17,200 generated calls.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/generic_mutations.py`, `just
  e10-generics`, 9 mutants.
  - ADR-0054's control "a piped value does not build an opaque value" is
    re-anchored where this moved its code.

## Not done

- **A generic type at the component boundary:** a query's parameter or
  result.
- **A lambda's result type,** where nothing but its branches names the
  instance. `x => if c { Either.Left(x) } else { Either.Right("") }` is
  refused. The backend infers an instance locally, and the checker's
  unification across the branches is not carried to it.
