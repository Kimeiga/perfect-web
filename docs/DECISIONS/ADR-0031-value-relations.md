# ADR-0031: the value relations, callable generics and function types

Status: accepted under the owner's instruction of 2026-09-24 ("close E9");
implemented with the evidence linked below. Every decision marked **(ruling
needed)** was made without an architect ruling and is offered for reversal
while it is cheap.
Date: 2026-09-24. Milestone: reopened E9 (gates E9-V1..V6).

## Context

E9 was reopened on 2026-08-21 because `takes_str(42)`, `fn wrong_return() ->
String { 42 }` and `takes_store(makes_cart())` all passed `pw check`. ADR-0030
gave signatures resolved identities, which made the relation possible without
performing it. Running the relation for the first time against the corpus found
that the corpus, the example library and the standard library had been written
without one, and that three pieces of syntax the relation needs did not reach
the HIR.

## Decision

### 1. One module owns the value relations

`compiler/pw-core/src/values.rs` decides, for every place a value meets a
declared type, whether the value has that type:

```text
Arity      a call supplies exactly the arguments its callee declares  PW0604
Argument   each argument has its parameter's type                     PW0605
Field      each constructed field has its declared type               PW0605
Return     a body produces its declared result                        PW0606
Binding    `let x: T = e` initialises `x` with a `T`                  PW0607
Annotation every written type resolves, at exactly its arity           PW0026
```

The outcome is three-valued (`Agree`, `Disagree`, `Undecided(why)`), and
diagnostics are a projection of `values::relations`, which `pw audit-values`
and `values::analysis` expose. Undecided is never reported as a violation and
never counted as agreement. `check.rs::call_arity` is deleted; arity moved into
the same walk so that "which declaration does this call reach" has one answer.

Calls covered: path calls, member calls (`x.f(a)`, receiver as the first
parameter), piped calls (`a |> f(b)`), `query X(..)` invocations, record and
opaque constructions (`Cart(..)`, `Cart { .. }`, `PositiveInt(1)`), and calls
inside policy term roots (optimistic transitions, `acquire`, `release`).

### 2. Inference types carry holes, not a second identity

`values::Ty` is `TypeKey` plus `Var` (alive only within one call's solution)
and `Unknown` (the program does not say). Every non-hole node projects a
`ResolvedType`'s `TypeKey`; `Ty` has no derived comparison, and `unify` is its
only relation. `Result<Int, ?>` is what `Ok(1)` is; forcing it into a
`ResolvedType` would invent the `?`.

### 3. Callable type parameters, instantiated per call (E9-V2)

`fn map<T, U>(items: List<T>, f: fn(T) -> U) -> List<U>` parses (it was
`PW0007`). The grammar reuses the `type_params` binder that `opaque type` and
`effect` use. At a call the callee's own parameters become fresh variables,
arguments unify left to right, and the result is closed with unbound variables
as holes. Inside a generic body its parameters are rigid.

`type Box<T> = Box { v: T }` now keeps its parameter. The grammar used
`skip_balanced` there, so `<T>` parsed and was discarded.

### 4. Function types (ruling needed)

`fn(A, B) -> R` is a type, lowered as `DeclaredType("fn", [A, B, R])` and
resolved as `Builtin::Function`. A lambda is typed against the function type it
is passed as (bidirectionally), and a declared callable named as a value has
its function type. WIT and the backend refuse function types explicitly. The
alternative was to leave callbacks untyped and `pw-std`'s signatures dishonest.
`List.map(items: List<Unknown>, f: Decoder)` is what that produced.

### 5. `e?` is a HIR node

`TryExpr` / `Expr::Try { value }`. The parser consumed the `?` and emitted
nothing, so `let id = field(raw, "id", string)?` bound a `Result`. Its value is
the success type, and each `?` is a result site returning the failure, whose
error type must agree with the declared result.

### 6. Declared constructor arity is part of the type (E9-V5)

`resolved::resolve` refuses `Box` and `Box<Int, Int>` for a one-parameter
declaration, as it already did for the builtins. `Workspace` records each type
declaration's arity by `DefId`.

### 7. Qualifiers are not the types they qualify (ruling needed)

`Session<SessionId>` is not `SessionId`, in either direction. The 2026-08-20
ruling made a qualifier *ABI*-transparent and said explicitly that opacity is
not ABI transparency. This applies the same separation to the value relation.
The store's `Carts.add(current_session(), ..)` therefore failed. The repair
declares the label: `Carts.add(s: Session<SessionId>, ..)`, `session query
Cart(session: Session<SessionId>)`. The WIT is unchanged
(`capability-session-id`); the semantic contract now carries the label.

**Alternative, for the architect:** make privacy qualifiers value-transparent,
leaving label flow wholly to `labels.rs`. That would revert these signature
changes and make `Session<X>` and `User<X>` interchangeable to the value
relation.

### 8. A phantom type parameter does not change a layout

`type Money<C> = Money { minor_units: Int }` maps every `Money<C>` to one WIT
record, because no field mentions `C`. The argument is kept by the contract and
dropped at the boundary, the same one-directional loss an opaque alias has. A
generic whose layout depends on its arguments is still refused (specialization
is not implemented). Opaque types are excluded, per the 2026-08-20 ruling.

### 9. Smaller decisions (each an assumption: A-018..A-021)

- **A-018.** A `()` body discards its last value. The language has no
  statement terminator, so the last expression of a unit function is a
  statement.
- **A-019.** `let x = query Q(..)` in a UI declaration binds `Q`'s success
  value (`{store.name}`). Loading and failure belong to the page, as `infer.rs`
  already assumed for `{#each}`.
- **A-020 (ruling needed).** A `LayoutSnapshot<T>` is read explicitly through
  `browser.value`; it is not treated as `T`. The alternative is a phase rule
  owned by the layout checker.
- **A-021.** A callable member whose only parameter is the receiver is read as
  a property (`self.style`, `w.value`), as `infer.rs` already did.

### 10. The resolver treats one declaration reached twice as one

`import domain` beside `import domain.{ MenuItemId }` made `MenuItemId`
ambiguous with itself. Import hits are now deduplicated by `DefId`.

## Acceptance

`compiler/pw-core/tests/value_relations.rs`: one discriminating pair per gate,
plus parser, lowering and non-vacuity controls. `scripts/e9_value_mutations.py`
disables each gate's mechanism in turn; every mutant must fail the tests. The
store program must be decided, not silent: every relation in `store/app.pw`
agrees, including `add_to_cart`'s call to `Carts.add`. The accepted corpus,
every rejected fixture (for its own rule only), the generality witnesses and
the rule fixtures pass `just ci`. Evidence:
[value-relations-2026-09-24.md](../evidence/E9/value-relations-2026-09-24.md).

## Consequences

The first run found defects in the milestone demo, the accepted corpus and the
libraries. They are listed in the evidence and in `docs/CORPUS.md` §C8. What
the relation does not decide is recorded in `docs/KNOWN_LIMITATIONS.md` as
open:

- member existence;
- sum-type variant constructors;
- named-argument calls;
- non-phantom generic layouts at the boundary.
