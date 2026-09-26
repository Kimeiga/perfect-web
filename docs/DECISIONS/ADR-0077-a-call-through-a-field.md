# ADR-0077: a call through a field holding a function is checked

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1, the
value checker).

## Context

`r.f(x)`, where `f` is a field of `r`'s type holding a function, is a call
through a value, as a call through a binding is (ADR-0068). A probe on
2026-09-26, at 99afe09, found that such a call resolved to nothing: a
member call was read only where a declaration takes the value first. So,
with `f: fn(Int) -> Int`, each of these checked:
- `r.f("a")`, an argument of the wrong type;
- `r.f(1, 2)`, one argument too many;
- `r.f(1)` where a `String` is declared, the wrong use of the result;
- `r.x(1)`, where `x` is an `Int`, a call to a value that is not a
  function.

The backend refuses the call by name ("`r.f` names no declaration this
program contains"), so no program ran wrong. Compiling it is NEXT's
"calling a function held in a record field".

## Decision

A call whose callee is a field of its receiver's type, and not a
declaration taking the receiver, is a call through the value the field
holds. ADR-0068's relations check it: its arity (PW0604), each argument
(PW0605), its result, and that the value is a function (PW0614). A
declaration that takes the value first is its member, as before.

## Acceptance

- **`compiler/pw-core/tests/field_calls.rs`**, 3 tests, each with
  controls. The first two fail at 99afe09, the commit before. The third,
  that a declared member is still a declaration, holds there too, and
  guards this change.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean. Their artifacts are
  byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/field_call_mutations.py`,
  `just e10-field-calls`, 2 mutants.

## Found while building it

An effect travels through a function value unseen. In a view declared
`!{}`, `clock.now()` is refused, and so is `let f = () => clock.now()`
then `f()`. Each of these passed on 2026-09-26:
- `let f = clock.now` then `f()`;
- a named function passed to a helper that calls it;
- a named function held in a record field and called through it;
- `List.map(xs, stamp)`, where `stamp` reads the clock.

The last is R-037's invariant, "a generic helper preserves its callback's
effects; it cannot launder them", which held only for a callback written as
a lambda. This is ADR-0078's.
