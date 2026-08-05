# ADR-0005 — SQLite first, PostgreSQL when nodes separate

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §10.4 says use SQLite first, then PostgreSQL when separating edge, origin
and database nodes, and §2 forbids building a database engine.

## Decision

Milestones 4 through 6 use **SQLite** for store, menu, cart and the
transactional outbox. PostgreSQL arrives at Milestone 11 when edge, origin and
database become separate nodes.

Invalidation uses **explicit typed events written in the same transaction as the
state change** (a transactional outbox), never inferred SQL dependencies —
see ADR-0007.

## Consequences

- No database work blocks the semantic model, which is the actual research question.
- A single-file database keeps the whole system reproducible on one laptop —
  more important than usual here, since the host has 16 GiB (see A-001).
- Both engines must sit behind the same generated interface so the swap at
  Milestone 11 is not a rewrite.
- SQLite's concurrency model differs from PostgreSQL's; Milestone 6's cache
  stampede and concurrent-reader tests must be re-run after the switch.

## Revisit when

Milestone 11 (multi-node lab), or earlier if a Milestone 6 materialization test
cannot be expressed on SQLite.
