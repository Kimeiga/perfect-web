# ADR-0215: a query's `retry` reaches its runtime as declared, and `fixed` is no strategy

Status: accepted under the owner's delegation of 2026-10-02. It builds
ruling 0089-b of ADR-0210, urgent defect 4. Date: 2026-10-05. Milestone: E14.

## Context

- **A page's plan carried a query's attempts and nothing else**
  (`page_values.rs`).
  - Its cache doubled each delay and added a jitter drawn from the key,
    always. So `retry bounded_exponential(max = 3, jitter = false)` ran with
    jitter.
  - `retry fixed(max = 3)` ran as exponential backoff, and nothing said so.
- **A command's `retry` was honoured** (ADR-0173). The browser sends a
  command again with the clause's bound, its jitter, and `fixed`'s constant
  delay.
- **What a strategy means for a query.** A declared error is an answer:
  only a read that failed is tried again. So `bounded_exponential` and
  `transport_only` retry a query alike.
- **Fixed delays synchronise retries.** Clients that failed together retry
  together, which is why the standard advice is exponential backoff with
  jitter (AWS, "Exponential Backoff And Jitter").

## Decision

- **`fixed` leaves `retry`'s operators** (PW0335). The fixtures that wrote it
  write `bounded_exponential`. The browser's branch for it goes, and so does
  the handler's `backoff` field, which only told the two apart.
- **A query's plan carries whether its delays vary**, `jitter`, absent where
  they do not. The server gives it to the cache, which jitters only then.
- **Every strategy the language has is honoured where it runs:**
  - for a query, in the cache: a bound, doubling, and jitter if declared;
  - for a command, in the browser: the same (ADR-0173).

## Acceptance

Recorded by `just e14-query-retry` in `docs/evidence/E14/query-retry.txt`:

- **`compiler/pw-core/tests/query_retry.rs`**: a query's attempts and jitter
  reach its plan, and none is written where there is none.
- **`runtime/pw-resource/tests/gate.rs`**: without jitter the delays are
  exactly 100, 200, 400; with it, they vary by key.
- **The dev server's test** `a_querys_retry_reaches_its_cache_as_declared`.
- **`compiler/pw-core/tests/policy_values.rs`**: `fixed(max = 3)` is none of
  `retry`'s operators. `handlers.rs` sends `{ retry: { max, jitter } }`, as
  the committed handler modules (`just e10-handlers`) and
  `e2e/compiled-handler.spec.mjs` do. The retry spec passes as before.
- **`scripts/query_retry_mutations.py`**: 4 mutants.
  `scripts/command_retry_mutations.py`'s "`fixed` is read as `exponential`"
  is retired with `fixed`.
