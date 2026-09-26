# ADR-0109: a timeout is a budget above zero

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.2, §9.5).

## Context

Charter §9.2 lists `timeout` among a resource's policies. The resource
runtime gives each flight its timeout as a budget, including retries, and
expires the flight once `now >= started + timeout`
(`runtime/pw-resource`). ADR-0089 checked `timeout` for being a duration, as
it does `freshness`.

On 2026-09-26, at 5fdc297, `timeout 0.seconds` checked. A budget
of zero expires every flight at the moment it starts, so the query can never
answer.

A freshness of zero is a different thing: a promise never to serve a stale
value, which the store's cart makes.

## Decision

**A timeout is a budget above zero** (PW0335). `timeout`'s domain is a
duration greater than zero; `freshness`'s is any duration. The message says
why zero is refused: every request would end before it starts.

## Acceptance

- **`compiler/pw-core/tests/timeouts.rs`**, 1 test, failing at 5fdc297:
  `0.seconds` and `0.minutes` refused, against a two-second budget and a
  freshness of zero.
- **Corpus.** No fixture's diagnostics change; no timeout in the corpus is
  zero.
- **Mutation controls:** `scripts/timeout_mutations.py`,
  `just e10-timeouts`, 3 mutants.
