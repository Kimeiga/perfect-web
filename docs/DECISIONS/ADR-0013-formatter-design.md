# ADR-0013 — Canonical formatting for `pw fmt`

**Status:** Accepted (design). **Implementation deliberately deferred.**
**Date:** 2026-08-05
**Milestone:** E2

## Context

E2 shipped a lossless lexer, so a formatter is *possible*. It was not shipped,
because canonical formatting becomes part of the language: changing it later
produces repository-wide churn, and reflowing 68 corpus files into a shape
nobody had agreed on would have been worse than having no formatter.

The architect confirmed the deferral and set the constraints below in advance.

## Decision

### Non-negotiable properties

A formatter that violates any of these is a bug, not a preference:

- **idempotent** — `fmt(fmt(x)) == fmt(x)`
- **semantics-preserving**
- **comment-preserving**, with comments attached to their original syntax node
- **no declaration or import reordering** unless separately requested
- **no load-bearing indentation** — structure comes from delimiters
- **spaces only**; no tabs in canonical output
- **explicit braces or delimiters** for structural blocks
- **stable under partially invalid syntax** where possible
- **minimal line changes** outside the reformatted syntax node
- **one canonical output** — no configuration beyond line width, at most
- corpus verified with `pw fmt --check` once adopted

### Defaults

```text
indentation      4 spaces
line width       100
declarations     one top-level declaration per separated block
collections      multiline collections and arguments use trailing commas
short forms      short declarations stay on one line when unambiguous
effect clauses   effect and capability clauses break one per line once multiline
comments         remain attached to their original syntax node
```

### Gate split

Formatter work is split so design can be accepted without implementation:

```text
E2 formatter-design gate
  - this ADR accepted
  - canonical examples approved

E2 formatter-implementation gate
  - pw fmt is idempotent
  - all corpus files round-trip
  - pw fmt --check passes in CI
```

## Consequences

- E2 gate item 3 ("formatting is deterministic and idempotent") reads
  **NOT STARTED** rather than FAIL, and is now split into two gates so the
  distinction is structural rather than a note.
- **Implementation must wait for ADR-0012.** Building a formatter on the current
  hand-rolled tree and then adopting Rowan would mean writing it twice.
- The corpus is the first formatting test set. `A-003`'s aligned policy columns
  and the `// @` header block are the cases most likely to expose a bad rule —
  the header must not be reflowed, and the alignment is currently hand-made.

## Open

Whether aligned policy columns (`freshness      30.seconds`) are canonical or an
artifact of hand-authoring. The corpus currently aligns them; a formatter must
either preserve that deliberately or normalise it deliberately. Decide with the
canonical examples, before implementation.

## Revisit when

The canonical examples are drafted, or ADR-0012's tree lands and makes a
constraint here impractical.
