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

---

## Amendment, 2026-08-05 — the open question, resolved; implementation landed

### Aligned policy columns are canonical

The corpus was measured rather than argued about:

| construct | runs in corpus | aligned | not aligned |
|---|---|---|---|
| policy lists | 8 | **8** | 0 |
| match arms | 2 | 2 | 0 |
| labelled statements (`animate` properties) | 4 | 2 | **2** |

**Policy lists align; nothing else does.** The convention is unanimous only for
policies, and the reason it holds there is structural rather than aesthetic: a
policy list is a table of keywords from a small closed vocabulary, so the column
is stable. Match arms and labelled statements hold arbitrary expressions, whose
lengths vary enough that aligning them turns one edit into a whole-block reflow.
Two of four labelled-statement runs already disagree with each other, which is
what an unstable convention looks like.

The rule: **pad each policy name to the longest name in its own list, plus one
space.** Two corpus files used different hand-chosen widths; the formatter
normalises both. The reflow stays inside a single `PolicyList` node, which is
what keeps it compatible with the minimal-line-changes rule above.

### What the implementation does not do

**It does not re-break lines.** Where the author put a line ending, one stays.
Width-driven reflowing is the part of a formatter most likely to be wrong in a
way that is expensive to undo, and the non-negotiable list above already
constrains line changes more than it constrains width. The honest consequence:
this is a canonical **spacing**, not yet a canonical **layout** — two files
differing only in where lines break format to two different outputs.

**It does not touch markup.** A `TemplateRegion` is copied verbatim, because
whitespace inside markup is significant in ways the grammar does not model.

### Formatter-implementation gate

| criterion | result |
|---|---|
| `pw fmt` is idempotent | **PASS** — asserted on all 68 corpus files |
| all corpus files round-trip | **PASS** — significant token streams identical before and after |
| comments preserved | **PASS** — asserted per file, with a negative control |
| formatted output still parses | **PASS** — re-parsed and checked for errors |
| `pw fmt --check` passes in CI | **PASS** — `just ci` runs it over the corpus |

### What implementing it found

Two defects that produced output which parsed, was idempotent, and was wrong:

- **Indentation cannot come from a brace counter.** A wrapped return type, a
  policy list and a pipeline continuation all indent without opening a brace.
  The first version got 33 of 68 files wrong. Indentation is now counted from
  the tree: how many constructs began on an earlier line and have not closed.
- **Policy alignment never fired.** It looked for a `Name` node, but the grammar
  keeps a policy keyword as a bare token, so every column came out as one space.
  The output still parsed and was still idempotent — the check that caught it
  was comparing against the corpus, not any property of the formatter.
