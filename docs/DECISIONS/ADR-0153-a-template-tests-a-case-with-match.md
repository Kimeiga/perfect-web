# ADR-0153: a template shows each of a value's cases by name

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14, gate item 5's first open form (T04).

## Context

Gate item 5 found that PW0305 refuses a `{#match}` leaving out a case, and
that one template form leaves a case out without it: `{#if status ==
Placed}` blocks. Nothing holds a chain of them to cover the cases. Checked on
2026-10-03, an `{#if}` chain that forgets `Delivered` is clean. T04's class is
"a state the page does not show", and this is its wrong fix in another form.

The other template form a catch-all would open is closed already. A template
`{#match}`'s arm is a case (PW5019): `{:_}` and `{:other}` are refused, and
PW0305 holds the arms to every case.

Prior art. Rust's clippy has `wildcard_enum_match_arm`, in its restriction
group. Its motivation is that new variants can be missed behind a wildcard,
and its known cost is enums that never change paying for the listing
(rust-clippy issues 8540, 17588). An elm-review discussion (#131) proposes
forbidding a wildcard that matches some, but not all, of a custom type's
variants. TypeScript reaches exhaustiveness with a `switch` and an
`assertNever(x: never)`. Each is opt-in, because in general code a catch-all
is often meant.

## Decision

**In a template, a case is tested with `{#match}`** (PW0337). An `{#if}` or
`{:else if}` whose condition compares a value with one of its type's cases,
with `==` or `!=`, either way round, is refused. That covers an `Option`'s, a
`Result`'s and a declared sum type's cases. The repair is `{#match}`, with an
empty arm where nothing shows, which PW0305 holds to every case.

So in a template every case of a value is named where the value is shown. A
case added later is a page that does not check until it is shown or
declared empty. Outside templates nothing changes: a function's `match` may
keep a catch-all.

## Acceptance

- `compiler/pw-core/tests/case_and_delivery.rs`: each form refused, `==` and
  `!=`, either way round, a qualified case, an `Option`'s, an `{:else if}`'s;
  and the controls, a `{#match}` naming each case and a comparison with a
  computed value.
- `scripts/case_and_delivery_mutations.py`.
- The corpus and the twelve tasks check as before: none tests a case with
  `{#if}`.

## Not claimed

- **A function a template calls can still map a state to nothing**:
  `fn label(s) = match s { Placed => "placed", _ => "" }`. That is a
  function's catch-all, not a template's.
- **A value compared with a computed one** (`status == chosen`) is not a case
  test, and is not refused.
