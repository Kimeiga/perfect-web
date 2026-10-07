# ADR-0252: a value returned early carries its label to the caller

Status: accepted under the owner's delegation of 2026-10-02; a correction,
found holding `e10-lexical`'s two survivors. Date: 2026-10-07. Milestone:
E14.

## Context

- **A body's label was its last statement's**, joined with each `?`'s
  value (ADR-0129, `labels::value_of_body`): what calling its declaration
  carries. A `return` in a branch or a loop was in neither.
- **So a secret returned early was public to the caller.** With `token()`
  reading a secret into a `String`, `fn g() -> String { if c { return
  token() } "none" }` was public, and `log.public(g())` passed; so did `for
  t in [token()] { return t }`. A `return` that is also the last statement
  was caught, being the block's value.
- **Which value comes back may be what leaks.** `if secretly() { return
  "yes" } "no"` returns two public values, and the one the caller gets says
  what the secret was. ADR-0129 holds a sink to the conditions it runs
  under and an assignment to the conditions it is made under; a return was
  held to nothing.

## Decision

1. **A body's label joins each `return`'s value**, at any depth, with the
   conditions the `return` runs under (`labels::returns`). A `return` in a
   function value leaves the function value, and is its own (ADR-0051).
2. **Each `?` joins its conditions too**: the failure it returns, and the
   conditions under which it may.

The last statement's value keeps its label as before: an `if`'s label
already joins its condition's.

## Acceptance

- **`compiler/pw-core/tests/returned_labels.rs`, 5 tests**, each with its
  control: a secret returned from a branch, and a public value; from a loop,
  and from a loop over public values; a public value returned where a
  secret decides, and where a public condition does; a `return` in a
  function value, which is not the body's; a `?` whose failure returns
  where a secret decides, and where a public condition does.
- **The whole workspace's tests**, the corpus among them: nothing else
  moved.
- **`scripts/returned_labels_mutations.py`: 4 mutants**, recorded by `just
  e14-returned-labels`: 4 of 4 killed.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **A `?` in a function value** still counts toward the body's label, as
  ADR-0129 had it: conservative, where a `return` there is the function
  value's own.
- **A loop that ends early**: the language has no `break` (ADR-0051).
