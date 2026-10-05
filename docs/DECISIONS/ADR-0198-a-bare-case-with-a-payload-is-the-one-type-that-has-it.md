# ADR-0198: a bare case with a payload is the case of the one type that has it

Status: accepted under the owner's delegation of 2026-10-02. It builds the
first half of ADR-0195's ruling 5. Date: 2026-10-05. Milestone: E14.

## Context

- **ADR-0059 typed a case written alone without a payload** (`Empty`) as the
  case of the one sum type the unit sees with it. Where several types had
  it, PW0022 named them.
- **A case with a payload written alone, `Circle(3)`, stayed PW0021**,
  "does not resolve". Its repair named `Shape.Circle(..)`. ADR-0059 left it
  as a "(ruling needed)".
- **The typer already typed a bare `Circle` as the function that builds the
  case**, and passing it, `List.map(radii, Circle)`, compiled. Only a call
  to it was refused, by the name check, the call's argument relation and the
  backend's call.
- **The owner ruled (ADR-0195, ruling 5):** the same rule as a case without
  a payload. It is accepted where the expected type has the case, or where
  exactly one visible type does; otherwise it is refused, naming the
  candidates.

## Decision

1. **`Circle(3)` alone is the case of the one type this unit sees with it**,
   by `values::bare_case`, the rule a case without a payload already has.
   - **The name check** accepts it, and PW0022 names every type where
     several have it, in their declaration order. Where none has it, PW0021
     stands.
   - **The typer** relates its arguments to the case's fields, as it does
     `Shape.Circle(3)`'s, so `Circle("three")` is PW0605.
   - **The backend** builds it as `Shape.Circle(3)`, in the component and in
     the JavaScript module alike.
2. **Not yet: resolution from the expected type**, the ruling's other half.
   With two types that have `Circle`, `fn f() -> Shape { Circle(3) }` is
   still PW0022, as `Empty` there is. That needs the typer to carry an
   expected type to the case, and the name check's ambiguity to move into
   the typer, for both forms at once. The qualified form, `Shape.Circle(3)`,
   always resolves.

## Alternatives

- **Keep PW0021 for a case with a payload.** The two forms of one construct
  would resolve by two rules: `Empty` alone typed, `Circle(3)` alone
  refused.
- **Resolve from the expected type first, now.** Done alone for this form, a
  case with a payload would resolve where `Empty` does not. The two halves
  belong to one change.

## Acceptance

- **`compiler/pw-core/tests/sum_types.rs`'s
  `a_bare_case_with_a_payload_is_the_one_type_that_has_it`:**
  - accepted where one type has the case;
  - its payload's type and its result checked;
  - PW0022 naming both types where two have it, with the repair;
  - PW0021 where none has it.
- **`compiler/pw-conformance/tests/sum_types.rs`'s
  `a_bare_case_with_a_payload_is_built`:** bare `Rect`, `Circle` and `Label`,
  built in a component against a Rust model, 150 calls.
- **`compiler/pw-conformance/tests/javascript.rs`:** the same query, the
  module agreeing with its component.
- **`scripts/bare_cases_mutations.py`**: 4 mutants (`just e14-bare-cases`).
