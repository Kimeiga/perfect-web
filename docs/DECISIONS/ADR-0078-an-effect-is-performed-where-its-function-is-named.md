# ADR-0078: an effect is performed where its function is named

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.4, pure
rendering; §7.5A, geometry reads).

## Context

A view has an empty external effect row (charter §7.4), and R-037 states
the rule for a generic helper: "a helper preserves its callback's effects;
it cannot launder them". The effect inference counted what a body calls:
a declaration's row where it is called, and a lambda's calls where it is
written. Found writing ADR-0077, on 2026-09-26 at ca34ab6, a view declared
`!{}` read the clock, and each of these passed:
- **`let f = clock.now` then `f()`.**
- **A named function passed to a helper that calls it**, `apply(stamp)`.
- **A named function held in a record field** and called through it.
- **`List.map(xs, stamp)`.** R-037's invariant held only for a callback
  written as a lambda.

And the rows of helpers that declare none, which a fixed point fills in,
counted calls alone. A view that called one of these passed:
- **a helper reading `el.offsetWidth`**, a geometry read, which a view may
  not do (PW0401);
- **a helper calling `clock.now` through a local.**

## Decision

- **A declaration named as a value performs its effects where it is
  named**, as a lambda's calls do where it is written: the value may be
  called wherever it goes. The explanation says so: "`stamp` is named as a
  value here, and performs it wherever the value is called".
- **A binding in scope is its own value**, whatever declaration shares its
  name (ADR-0063).
- **A name inside an event handler, a stream or a later frame phase** is
  that region's, as a call there is (`contexts.rs`). So `on:press={go}` does
  not give a view `go`'s effects.
- **A helper that declares no row carries everything its body performs**:
  its calls, the members it reads, and the declarations it names. The fixed
  point that fills in its row uses the same inference as a declaration's own
  row check.

**(ruling needed)** A function type states no effect row, so a parameter
`f: fn(Int) -> Int` says nothing about what `f` does, and a helper that
calls it is checked as though `f` did nothing. This ADR makes that sound by
counting the effects where the function is named, not where it is called.
The alternative is effect rows on function types, `fn(Int) -> Int !{..}`,
as Koka's are, which would let a helper's own row state what its callback
may do.

## Acceptance

- **`compiler/pw-core/tests/effects_through_values.rs`**, 5 tests, each
  with controls. The three that state a route an effect took fail at
  ca34ab6, the commit before. The two guards hold there too: a function
  named in a handler is the handler's, and a binding named like a function
  is its own value.
- **A generality witness**, `forbidden_effect/named-callback.pw`: a view
  handing a geometry-reading declaration to `List.map` by name, with the
  effect declared. It emits its invariant alone, where it passed at
  ca34ab6.
- **No other rejected, rule or generality fixture's diagnostics change.**
  The store, kiokun and the accepted corpus check clean. The artifacts are
  byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/effect_value_mutations.py`,
  `just e10-effects-through-values`, 4 mutants.
