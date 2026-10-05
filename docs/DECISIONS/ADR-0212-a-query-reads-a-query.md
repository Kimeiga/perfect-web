# ADR-0212: a `query` reads a query or a resource, and a `subscription` a subscription

Status: accepted under the owner's delegation of 2026-10-02. It builds
ruling 0108-a of ADR-0210, urgent defect 6. Date: 2026-10-05. Milestone: E14.

## Context

- **`let v = query helper(n)`, `helper` a `fn`, checked** (reproduced with
  `pw check`). The graph drew a read of it, and the page would keep the
  function's answer as an entry. Its key, freshness and cache are what a
  query declares, and a function declares none of them.
- **The name was resolved, and its kind never asked.** A `query` of a
  subscription and a `subscription` of a query checked too.

## Decision

- **In `query R(..)`, `R` names a query or a resource; in `subscription
  R(..)`, a subscription.** Anything else is refused where it is read,
  PW5108, by its kind: "`query helper` reads a function".
- **A function's repair is to call it**: `helper(n)`, without `query`.

## Acceptance

Recorded by `just e14-query-reads` in `docs/evidence/E14/query-reads.txt`:

- **`compiler/pw-core/tests/reads_name_their_kind.rs`**, 3 tests:
  - a `query` of a function is refused;
  - so are a `query` of a subscription and a `subscription` of a query;
  - each reads its own kind, as a control.
- **The corpus checks as before**: every read names its kind.
- **`scripts/query_reads_mutations.py`**: 3 mutants.
