# ADR-0010 — `.pw` is the provisional source extension

**Status:** Proposed
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §14 M2 task 1 requires choosing a provisional file extension and grammar
in an ADR, and suggests `.pw` "unless a better non-conflicting choice is found".
Milestone 0's corpus needed an extension before Milestone 2 could decide.

## Decision

Use **`.pw`** provisionally for all corpus and example files.

This is deliberately recorded as **Proposed**, not Accepted: Milestone 2 task 1
owns the real decision. Milestone 0 needed a filename, not a commitment.

## Consequences

- 45 corpus files use `.pw`; changing it is a rename plus a one-line change in
  `tools/corpus-check`.
- **Not verified:** `.pw` has not been checked exhaustively against editor and
  language-tooling registries for conflicts. Tracked as `docs/ASSUMPTIONS.md`
  A-007.

## Revisit when

Milestone 2 task 1. Supersede this ADR with an Accepted one at that point.
