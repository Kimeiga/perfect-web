# ADR-0084: what a string interpolates has a text form

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1).

## Context

ADR-0074 related what a template writes to a type with a text form. A
string's holes in code are the same question, and nothing related them. On
2026-09-26, at 5a874ee, each of these checked:
- `"{[n, 2]}"`, a list;
- `"{p}"`, a record;
- `"{o}"`, an `Option`.

The backend was the first to refuse each: "the backend does not lower an
interpolated value of this type yet: ... has no text form here".

Three fixtures interpolated a value with no text form:
- two that must stay clean logged a `Result`, `log.public("loaded {store}")`
  with `store` a `Result<Store, StoreError>`: the rule fixture
  `privacy-sink/public-value-to-public-log.pw`, and the generality
  neighbour `value_exceeds_sink_level/valid-public-neighbour.pw`;
- the generality witness `private_in_shared_cache/branch-join.pw` showed a
  whole `Cart` and a whole `Store`, records, and would have emitted this
  second defect beside its invariant.

## Decision

- **What a string interpolates has a text form**, the same rule as
  ADR-0074's (PW0609). A `String`, an `Int` or a `Bool` has one, and so does
  an opaque type over one. A `Float` has no format yet.
- **An `Option` or a `Result` is taken apart first** (PW0600), and the
  message names `match`, not the template's `{#match}`.
- **A template attribute's string stays the template's** (ADR-0074), which
  names the attribute. It is reported once.

**(ruling needed)** Whether a record or a list should have a text form of
its own. None has one today, in a template or a string.

## Acceptance

- **`compiler/pw-core/tests/string_holes.rs`**, 3 tests, each with
  controls. The two that state a hole with no text form fail at 5a874ee, the
  commit before. The third, an attribute's string reported once, as the
  template's, holds there too, and guards this change.
- **The three fixtures are corrected.**
  - The two clean ones take the store's name out of the `Result` and log
    it.
  - The witness shows the cart's count and the store's name, and emits its
    invariant alone.
  No other rejected, rule or generality fixture's diagnostics change.
- **ADR-0064's tests logged an `Option` or a list** in five programs, each
  about where a label flows, not what is logged. Each takes the `Option`
  apart, or joins the list with a fold, and still refuses its secret (PW5006)
  alone. `String.join` would have joined the list, and it launders a label,
  which ADR-0085 takes up.
- **The store, kiokun and the accepted corpus check clean.** Their artifacts
  are byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/string_hole_mutations.py`,
  `just e10-string-holes`, 3 mutants.
