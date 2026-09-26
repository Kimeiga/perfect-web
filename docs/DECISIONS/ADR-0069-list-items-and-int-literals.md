# ADR-0069: a list's items share one type, and an `Int` literal fits an `Int`

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1, the
value checker).

## Context

A probe for wrong programs that pass `pw check`, run after ADR-0068, found
two more. On 2026-09-26 each checked:
- **A list whose items are of two types:** `[1, "a"]`, returned where a
  `List<Int>` is declared, and `List.length([1, "a", true])`. A list's items
  were joined, and where two disagreed the element type was a hole, so every
  relation through the list was undecided.
- **An `Int` literal past 64 bits:** `99999999999999999999`. The checker typed
  it an `Int`, and the backend was the first to refuse it ("does not fit in
  the ABI's `s64`").

## Decision

- **A list's items are related to each other** (PW0615, "a list's items must
  share one type"). The first item of a known type is the list's, and an
  item of another type is refused there. An item of any type agrees, so
  `[None, Some(1)]` is a list of `Option<Int>`.
- **An `Int` literal is related to the range an `Int` holds** (PW0616, "an
  `Int` literal must fit in 64 bits"). A literal is a magnitude, so the least
  `Int`, whose magnitude is one past the greatest, is written as an
  operation on it, as the backend already requires.

## Acceptance

- **`compiler/pw-core/tests/lists_and_literals.rs`**, 2 tests, each with
  controls. Both fail at e30d755, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/list_literal_mutations.py`,
  `just e10-lists`, 3 mutants.
