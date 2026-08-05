# ADR-0004 — Preserve standard HTML, CSS, URLs and HTTP

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §11.2 forbids replacing HTML, URLs, CSS or HTTP wholesale, and §8.2
requires semantic elements rather than generic `View`/`Container` nodes. The
temptation to invent a binary document format is explicitly listed as a non-goal
in §2.

## Decision

The platform emits **standard semantic HTML, standard CSS, standard URLs, and
ordinary HTTP responses**. Compile-time checks constrain *how* those are used
(§8.2: nesting, labels, keyboard accessibility, ARIA relationships, list keys,
duplicate ids, dead routes, unsafe raw HTML) but never replace them.

Ten of the 31 rejected corpus examples encode exactly these HTML/accessibility
rules (R-018 through R-024), so the commitment is executable rather than stated.

## Consequences

- Interoperability, accessibility, indexing, caching, streaming and
  debuggability come from the platform instead of being reimplemented.
- The static route can be genuinely zero-JS, which the Marko spike confirmed.
- We inherit HTML's quirks and CSS's cascade. Accepted deliberately: charter §2
  lists "a CSS layout engine" and "a bespoke binary replacement for HTML" as
  things this project must not build.

## Revisit when

Never, as a wholesale replacement. Narrow additions (charter §8.6 declarative
patches, §13 browser primitives) are Milestone 13 experiments and must ship with
a working fallback for current browsers.
