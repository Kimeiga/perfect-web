# ADR-0071: what a template's blocks and events take

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1, the
value checker).

## Context

A probe for wrong programs that pass `pw check`, run after ADR-0070, found
three in a template. On 2026-09-26 each checked and built at 3555fa4:
- **`{#each n as x}` over an `Int`.** An `{#each}`'s collection was read only
  to type its name, and a collection that is not a list gave the name no type
  and was related to nothing. The renderer refuses it when it renders
  (`MissingValue`).
- **`{#if s}` over a sum type the program declares.** `{#if}` over an
  `Option` or a `Result` is PW0600 (ADR-0042). A declared sum type is taken
  apart with `{#match}` as they are (ADR-0061), and the renderer refuses a
  case as a condition, but the checker said nothing.
- **`on:press={n}` over an `Int`.** An event attribute's value was related to
  nothing.

## Decision

- **An `{#each}`'s collection is an operand** (PW0609, "the list an
  `{#each}` runs over must be `List<_>`"). A value of another known type is
  refused.
- **A template condition has a truth** (PW0609). `{#if c}` and
  `{:else if c}` over a declared sum type are refused: "`{#if}` tests a
  `Shape`, which is taken apart with `{#match}`". An `Option` and a `Result`
  stay PW0600's alone. No relation is recorded for either, so neither is
  reported twice, and neither is counted as a condition that agrees.
- **The renderer's truth is kept for the rest**: a `Bool`, an `Int`, a
  string, a list and a record. ADR-0042 relies on it for a list, with
  `{#if xs}` around an `{#each}`. kiokun's search page tests a list,
  `{#if hits}`, and a string, `{:else if q}`.
- **An event attribute is given a function** (PW0614, "`on:press` is given
  `Int`, which is not a function to call"). A lambda, a function value and a
  declared function agree.

**(ruling needed)** Whether a template condition must be a `Bool`, as a code
`if`'s is (ADR-0043). This ADR keeps the renderer's truth, which kiokun and
ADR-0042's own example rely on.

## Acceptance

- **`compiler/pw-core/tests/template_operands.rs`**, 4 tests, each with
  controls. Three fail at 3555fa4, the commit before, with "nothing
  reported". The fourth, `an_option_condition_is_reported_once`, holds there
  too, where no condition was related. It guards this change against
  reporting an `Option` condition twice or counting it as agreeing.
- **One rule fixture's diagnostics change.**
  `examples/rules/resume/loop-capture-untyped.pw` runs `{#each}` over an
  `Int` on purpose, to pin PW5016, and now reports PW0609 as well. PW5016
  still fires: the capture schema refuses a type it does not know, whether
  or not another rule refuses the program. No rejected or generality
  fixture's diagnostics change.
- **The store, kiokun and the accepted corpus check clean.** Every new
  relation in the store is decided, as `the_store_program_is_decided_not_merely_silent`
  requires.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/template_operand_mutations.py`,
  `just e10-template-operands`, 11 mutants.

## Checked while building it

`on:press={go}`, a declared function named directly, builds an event part
whose `name` is empty, where `on:press={() => go()}` names `go`. That is not
a new defect: the browser runtime attaches only a resumable handler, which
is always a lambda (ADR-0033). A named `fn` bound to an event is checked and
not compiled, as KNOWN_LIMITATIONS says.
