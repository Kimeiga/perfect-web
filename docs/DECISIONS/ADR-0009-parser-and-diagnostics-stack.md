# ADR-0009 — Hand-written parser with annotate-snippets diagnostics

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §14 M0 task 11 required validating "development ergonomics before
choosing parser libraries", and §16.3 sets a specific diagnostic quality bar:
name the rule, the value's origin, the boundary that rejected it, the inferred
label, and at least one legal alternative.

## Decision

Diagnostics render through **`annotate-snippets` 0.12.16**. Source positions are
plain `Range<usize>` byte spans produced by a hand-written lexer.

Every diagnostic carries a **stable `PW####` code**, at least one **primary**
span, and — where a rule involves a boundary — a **context** span naming where
the offending label originated. `Renderer::plain()` provides ANSI-free
byte-stable output for the Milestone 2 compile-fail snapshot tests.

## Consequences

- The charter's §16.3 target diagnostic was reproduced from real parsed spans.
- `AnnotationKind::Primary` / `::Context` maps directly onto the
  boundary/origin distinction the charter requires, so the shape is native to the
  renderer rather than bolted on.
- Two rules discovered by running the spike:
  1. **Semantic checks must be suppressed when parsing produced errors** —
     otherwise the checker reports on parser recovery rather than on user code.
  2. **`pw explain` must be *tested* for generated-file leakage**, not merely
     intended to avoid it. Milestone 2's gate requires it; it is now an assertion.
- **Not yet decided:** the lossless tree representation for `pw fmt`. Milestone 2.

## Revisit when

Milestone 2, when the real grammar and the lossless tree are designed; or if
`annotate-snippets` cannot express a needed diagnostic shape.
