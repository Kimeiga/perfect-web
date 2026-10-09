# ADR-0284: a value of any type is fixed by the call that meets it

Status: accepted under the owner's delegation of 2026-10-02, on W6's
finding of 2026-10-09 and the integrator's probes of it. Date: 2026-10-09.
Milestone: E14. Amends ADR-0065 (its second decision).

## Context

- **W6's finding** (track `kiokun`, kiokun's word page, 2026-10-09).
  `List.fold(xs, [], (out, x) => List.concat(out, [..]))` checked clean,
  and `pw build` refused it: "an empty list whose element type nothing fixes
  yet". Writing the seed's type, `let none: List<T> = []`, built.
- **The backend's part.** It types `[]` by what its context expects. A
  fold's seed is expected to be what the fold is expected to be, and `let
  all = List.fold(..)` expects nothing.
- **The checker's part, found probing it at `33dd1ce`: it was unsound.** It
  typed that fold as `List<_>`, a list of anything (ADR-0065), however its
  function built the list. So it passed, where a `List<String>` was declared:
  - `List.fold(xs, [], (out, x) => List.concat(out, [1]))`;
  - the same through a binding, and its first element returned as a
    `String`: an `Int`.
  - **The cause.** ADR-0065's second decision says a variable a value of any
    type meets is left free, and fixed by what else it meets, as `T` in
    `List.concat([], ["a"])` is. A fold's accumulator `A` is not met by a
    value of any type but by `[]` whole, `List<_>`. It was bound to that,
    and the function's `List<Int>` then agreed with it, since `_` agrees
    with everything.
  - **What stood behind it.** The backend refused each such program, for
    want of a type or because the function produced a `List<Int>` where a
    `List<String>` was needed. The checker's verdict was wrong, and the
    build was its only backstop.

## Decision

1. **A variable bound to a type with parts of any type gets a variable for
   each part**, which what else the call meets may fix. A part nothing fixes
   closes to any type, as before. This completes ADR-0065's second decision:
   `A` met by `[]` is `List<B>`, and the function's `List<Int>` fixes `B`.
   The checker refuses each of the folds above (PW0606).
2. **The backend gives a seed its context does not type the fold's own
   type, as the checker solved it**, where every part of it is one the
   backend lays out. A seed that nothing fixes anywhere stays refused: it
   has no layout.

## Alternatives

- **Report `[]` where the build would refuse it** (W6's second option).
  Refused: the checker knows the type, once decision 1 lets the function
  fix it, and a program that says it once should not have to say it twice.
- **Type a lambda's parameters after the call is solved, and solve again.**
  Refused: decision 1 reaches the same type in the one pass the solver
  makes, with nothing re-run.
- **Let the backend infer the seed from the function's body.** Refused: a
  second inference would be free to disagree with the checker's.

## Acceptance

- **`compiler/pw-core/tests/any_type.rs`**, two tests:
  - a fold's accumulator is what its function builds: three ill-typed folds
    refused (a list seed, through a binding, an `Option` seed), with their
    controls, and a part nothing fixes still of any type;
  - a seed its context does not type is built at the fold's type, and one
    nothing fixes is still refused by the backend.
- **The corpus, the reference apps and `examples/kiokun-site` check as
  before**: no accepted program was ill-typed this way.
- **`scripts/what_a_component_does_mutations.py`** carries this ruling's
  mutants beside ADR-0283's, from the same report.

## Not claimed

- **Only a fold's seed takes the solved type in the backend.** Another `[]`
  its context does not type, an argument to `List.concat` first among them,
  is refused as before, and says why.
