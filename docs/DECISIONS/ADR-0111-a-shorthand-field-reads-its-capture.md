# ADR-0111: a shorthand field reads its capture

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (ADR-0033, ADR-0058).

## Context

A resumable handler reads what it captured. Three places decide what that
is, and each read a capture only through a name or a field path, `item` or
`item.id`:
- `resume::capture_paths` decides what the document carries for the handler,
  and what the compiled handler is given;
- the handler artifact's mentioned captures are what PW5017 compares against
  the manifest's capture schema;
- the backend reads a record's shorthand field from bindings alone.

A record's shorthand field, `Pick { n: 1, item }`, reads `item` and has no
expression of its own, so none of the three saw it. On 2026-09-26, at
6c08b34, a handler capturing `item` and building
`Pick { n: 1, item }` was refused by PW5017: the artifact read nothing of
`item`, and the manifest captured it. The program had nothing wrong in it.
ADR-0110, which counts a shorthand read as a read, found it.

## Decision

**A shorthand field reads its capture.** In a resumable handler,
`Pick { n: 1, item }` reads the whole of `item`, as `item` alone does:
- the capture paths carry `item`;
- the artifact mentions it;
- the backend gives the field the captured value, unless a binding of the
  handler's shadows it, as for a path.

## Acceptance

- **`compiler/pw-core/tests/capture_shorthand.rs`**, 2 tests. Each fails at
  6c08b34:
  - the program checks, against the store's field read;
  - its handler compiles, reading `context.captures["item"]` whole.
- **ADR-0110's shorthand test** now asserts that its captured control is
  clean, where it asserted only that PW5025 was absent.
- **Corpus.** No fixture's diagnostics change; no handler in the corpus
  builds a record from a capture.
- **Mutation controls:** `scripts/capture_shorthand_mutations.py`,
  `just e10-capture-shorthand`, 3 mutants.
