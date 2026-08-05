# ADR-0007 — Explicit typed invalidation events before any automatic dependency inference

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §9.4 says to start with explicit typed invalidation events and a
transactional outbox, and explicitly not to attempt automatic SQL dependency
inference first. §20 lists "automatic invalidation inference is unsound or
opaque" as a live risk.

## Decision

Invalidation is driven by **explicit typed events** declared in the resource
declaration (`invalidates_on StoreChanged(id)`) and emitted by commands **inside
the same database transaction as the state change**.

Automatic dependency tracking may later be added only as an **auditable
optimization** over this ground truth, never as a replacement for it.

## Consequences

- Invalidation is inspectable and testable: charter §14 M6's gate ("`MenuChanged(store_47)`
  invalidates only store 47") is directly checkable.
- Corpus examples A-009 and R-017 already encode this shape, and R-005 encodes
  the cache-key partition rule that goes with it.
- Developers must declare invalidation sources by hand, and a missing declaration
  produces stale data. Mitigation: a shared materialization with neither a
  freshness policy nor an invalidation source is a warning today (`PW0200` in the
  diagnostic spike) and becomes an error at Milestone 6.

## Revisit when

Milestone 6 completes and the dependency graph is stable enough that deterministic
query execution could record logical reads. Any such change needs its own ADR and
must keep the explicit path as the fallback.
