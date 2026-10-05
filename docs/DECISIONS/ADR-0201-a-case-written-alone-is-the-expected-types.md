# ADR-0201: a case written alone is the case of the type expected where it is written

Status: accepted under the owner's delegation of 2026-10-02. It builds the
second half of ADR-0195's ruling 5. Date: 2026-10-05. Milestone: E14.

## Context

- **A case written alone resolved by one rule:** the one sum type the unit
  sees with that case, for `Empty` (ADR-0059) and for `Circle(3)`
  (ADR-0198). Where several types had it, PW0022 named them, wherever it was
  written.
- **So two types with an `Empty` made every bare `Empty` an error.** That
  held even in `fn f() -> Shape { Empty }`, where the program states which
  type it means.
- **The owner ruled (ADR-0195, ruling 5):** a case written alone is
  accepted where the expected type has the case, or where exactly one
  visible type does. Otherwise it is refused, naming the candidates.
- **Two analyses decided it, and the backend a third time.** The name check
  reported PW0022 for `Empty`, `unresolved_uses` for `Circle(3)`, and the
  backend resolved both by the one-type rule. None of them knew a type.
- **Sources:**
  - The Swift reference: "An *implicit member expression* is an abbreviated
    way to access a member of a type, such as an enumeration case or a type
    method, in a context where type inference can determine the implied
    type." `.empty` where a `Shape` is expected is `Shape.empty`.
  - Pierce and Turner, "Local Type Inference" (POPL 1998; TOPLAS 22(1),
    2000): the type a context gives is carried into the expression written
    there, the direction this rule reads.

## Decision

1. **The type expected where a case is written chooses among the types that
   have it.** It is read from where the case is written, never from what it
   is:
   - a result of the declaration: its declared result;
   - a `let`'s initializer: its annotation;
   - an argument: its parameter's type, by the rule arguments are arranged
     by (ADR-0081);
   - a record's field: the field's declared type;
   - an assignment: what is assigned to;
   - a comparison's side: the other side's type;
   - a list's item: the list's element, or another item's type;
   - a branch of a used `if`, an arm of a used `match`, and a block's last
     value: what the `if`, `match` or block is expected to be.
2. **The case is the expected type's.** `Circle(3)` resolves by the type its
   call is expected to be. Where one type alone has the case, ADR-0198's rule
   stands.
3. **The typer decides it, once.** It owns PW0022 for a case written alone:
   where nothing expected chooses, or what is expected is none of the types,
   it names them all. The name check and `unresolved_uses` no longer report
   it.
4. **The backend asks the typer.** `values::case_at` is the one answer, so
   the component and the JavaScript module build the case the checker
   typed.
5. **A question that leads back to itself answers nothing.** In
   `Empty == Empty`, each side's type is the other's. Each is refused,
   PW0022.

## Alternatives

- **Keep the one-type rule.** Two types with a common case name, `Empty` or
  `None`, are ordinary. The owner ruled the expected type chooses.
- **Infer the case from what it is later used as.** Hindley–Milner would
  unify `let s = Empty` with a later `take(s)`. The rule here reads only what
  is written where the case is. A reader sees the type that chose, at the
  case.
- **Resolve in the name check.** It has no types. Moving the decision into
  the typer puts it where types are known, and the backend asks the same
  function.

## Acceptance

- **`compiler/pw-core/tests/sum_types.rs`'s
  `a_bare_case_is_the_case_of_the_type_expected`:**
  - with two types that have `Empty` and `Circle`, a case written alone is
    accepted at each position: a result, through a branch, an arm and a
    list, an annotation, an argument, a field and a comparison, and a branch
    and an arm whose value an annotation expects;
  - `Circle("three")`, chosen, is typed as `Shape.Circle`: PW0605;
  - PW0022 where the expected type is neither;
  - the case is the expected type's: a `Box`'s `Empty` is no `Shape`
    (PW0605);
  - each side of `Empty == Empty` is refused.
- **The earlier PW0022 tests** now write the case where nothing is
  expected, `let s = Empty`.
- **`compiler/pw-conformance/tests/sum_types.rs`'s
  `a_case_written_alone_is_built_as_the_type_expected`:** three queries
  over two types, built in components against a Rust model, 150 calls each.
- **`compiler/pw-conformance/tests/javascript.rs`'s
  `a_case_from_its_expected_type_agrees_with_its_component`:** the same
  queries, the modules agreeing with their components on 600 calls, traps
  included.
- **No program changes.** The store's build is byte-identical.
- **`scripts/expected_cases_mutations.py`**: 14 mutants (`just
  e14-expected-cases`).
