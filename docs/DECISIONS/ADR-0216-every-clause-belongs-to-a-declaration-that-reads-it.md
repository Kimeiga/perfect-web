# ADR-0216: every clause belongs to a declaration that reads it, and a code body admits none

Status: accepted under the owner's delegation of 2026-10-02. It builds
ruling 0092-b and the interim of ruling 0047-a, both from ADR-0210. Together
they fix urgent defect 5. Date: 2026-10-05. Milestone: E14.

## Context

- **PW5105 placed only the dependency graph's four clauses** (ADR-0092):
  `emits`, `invalidates`, `invalidates_on`, `depends_on`. The other 61
  heads checked wherever they were written. The sweep probed:
  - `freshness`, `on_key_change` and `cache` on a command checked;
  - so did `retry` on a function;
  - nothing read any of them.
- **A clause word in a code body was a clause nothing read.** `cache
  nothing_y` in a function's body checked, and `nothing_y` was never
  resolved: the name walker took any policy word with a value on its line as
  a clause, in any body.
- **The audit**, read from every reader of every head:
  - Where each head is read is known.
  - Some heads written in accepted fixtures are read by no runtime yet.
    `transport` on a subscription (A-006), `affine` on a resource (A-007),
    and `revision` on a page (A-013) are each written where they belong by
    design.
  - Three forms share `DeclKind::Other`: a painter, a replicated value and a
    handler policy.
  - Four heads are placed by their own rules: `requires` (PW0335),
    `not_found_on` (PW0342), `rollback` (PW0327) and `host` on an effect
    (PW0332).

- **Found by the second:** R-026 wrote `placement edge` inside its
  function's body, where nothing read it. It was caught by PW5002 ("cannot
  run in any world") for want of the placement. The refusal it was written
  for, "`secret<Payments>` is not granted to the edge world", with its
  expected repair, never spoke.

## Decision

- **Every head has an entry in `declared_by`**, the declarations it belongs
  to by design. A head on any other kind is PW5105, and the table fails
  closed:
  - a cached read's heads (`freshness`, `consistency`, `concurrency`, `key`,
    `dedupe_by`, `timeout`) belong to a query, a subscription or a resource;
  - `cache` to those and a page;
  - `retry` to a query, a resource or a command;
  - a command's to a command;
  - a subscription's connection to a subscription;
  - a resource's life to a resource;
  - and so on for each.
- **A head another rule places is listed for every kind it covers**, so a
  defect is reported once (ADR-0112).
- **A head written only in statements is listed for no declaration**, by the
  statement it belongs to: `intrinsic_height` by a `subtree` block, `because`
  by an `unsafe` statement.
- **A code body admits no clause** (0047-a's interim). In a function's,
  query's or command's body a policy word is an ordinary name, resolved or
  refused (PW0021). Its clauses are written before its body. The full split,
  where the parser makes a clause only in a block that admits it, is after
  the app layer.
- **The table is complete**: a test holds every head the grammar knows to
  an entry, so a head added without a place is caught.
- **R-026 writes its placement before its body.** Read now, the edge's
  refusal speaks: PW5005, its expected text. PW5002 defers to it, since one
  defect is reported once (ADR-0112). R-026 declares PW5005 and
  `declared_placement_cannot_grant`.

## Acceptance

Recorded by `just e14-clause-heads` in `docs/evidence/E14/clause-heads.txt`:

- **`compiler/pw-core/tests/clause_places.rs`**, 3 tests more:
  - the sweep's four probes are each PW5105, and the same head where it is
    read is not;
  - every head the grammar knows has a place;
  - `cache nothing_y` in a function's body refuses `nothing_y` (PW0021).
- **The store, kiokun and the accepted corpus check clean** under the full
  table.
- **R-026 emits only its declared defect** (`checking_source.rs`).
- **`scripts/clause_heads_mutations.py`**: 5 mutants.

## Not claimed

- **Heads written where they belong and run by nothing**, such as `revision`
  on a page and `transport` on a subscription, are placed, not executed.
  Each is the work of the feature it names.
