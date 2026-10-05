# ADR-0206: a case has one name where values are rendered, its WIT case's

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-05. Milestone: E14. Found building ADR-0205.

## Context

- **Two names for one case.**
  - The browser's wire names a case as its WIT case is: `some`, `none`,
    `ok`, `err`, and `circle` for a declared `Circle` (ADR-0061, ADR-0130).
    The build writes a signal's first value so, and a handler sets one so.
  - The template named a declared case so, and the language's own four as
    Pleris writes them, `Some`, `None`, `Ok`, `Err`. A host's query value,
    and the renderer's stream outcome, named the four so too. The values
    files ADR-0193 introduced wrote `"$case": "Some"`.
- **So a page matching on an `Option` or a `Result` a signal holds found no
  arm.** `{#match picked}{:Some(n)}..{:None}..{/match}` with `signal picked:
  Option<Int> = None` refused to render: "no arm renders `none`". No page in
  the repository matched on such a signal, so nothing showed it.
- **One name is the browser's wire's.** It is the one the browser's modules
  and the build already write, and the one a declared case already had.

## Decision

1. **Every case is named as its WIT case is, where values are rendered**:
   the template's arms, a host's values, the renderer's stream outcome, and
   a values file. `{:Some(n)}` is the arm `some`.
2. **The language's four have no other name there.** A value or an arm that
   says `Some` names no case of an `Option`.

## Alternatives

- **The renderer reads `some` as `Some`.** It reads values knowing no type,
  so it could not tell the language's `some` from a declared case named
  `Some`, whose WIT name is `some` too. And two names would stay where one
  does.
- **The browser's wire writes `Some`.** It would name the language's four
  apart from every declared case, and the modules, the build and every
  value already written would change.

## Acceptance

Recorded by `just e14-one-case-name` in `docs/evidence/E14/one-case-name.txt`:

- **`compiler/pw-core/tests/one_name_for_a_case.rs`**, 2 tests: a page
  matching on an `Option`, a `Result` and a declared sum type its signals
  hold renders at the first values the build writes and at values as a
  handler sets them; and the build writes each case by its WIT name.
- **The tests that named the four otherwise**, now naming them so: the
  template's (`template_blocks.rs`), the renderer's (`branches.rs`,
  `cases.rs`, `streams.rs`, `plan_lists.rs`), and the development server's.
- **The committed kiokun build** (`docs/evidence/E10/kiokun/`), rebuilt: its
  templates name the arms so. `evidence_is_current.rs` compared four kinds
  of file there, and `pw build` writes a graph and page plans beside them,
  so rebuilding the directory as `just e10-kiokun` does failed it. It
  compares every file now.
- **`scripts/one_case_name_mutations.py`**: 5 mutants. Two scripts
  re-anchored: `streams_mutations.py` and `template_match_mutations.py`.
