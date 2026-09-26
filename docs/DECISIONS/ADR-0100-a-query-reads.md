# ADR-0100: a query reads

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.5).

## Context

Charter §7.5 gives each primitive one meaning:
- a `query` is a "keyed remote read with cache, freshness, lifecycle,
  cancellation, and dedupe";
- a `command` is the "explicit mutation with authorization, idempotency,
  transaction, optimistic behavior, and invalidation".

PW0401 holds a declaration to the effects its kind may perform: a view may
not reach the database while it renders. Nothing held a query to reading.
On 2026-09-26, at 64140d7, a `session query` whose body was
`Carts.clear(current_session())`, declaring `database.write<Carts>`,
checked. The platform caches, deduplicates and retries a query as a read,
so the write happened once for every reader of a cache entry, and again on
every retry. None of a command's declarations applied: no idempotency key,
and nothing it invalidates.

## Decision

**A query reads** (PW0401). A query or a subscription that performs
`database.write` or `database.transaction` is refused; a transaction exists
to hold writes. A mutation is a command's. Reading, `database.read`, is what
a query is for.

**(ruling needed)** Which other effects mutate what a query reads. The
ontology marks no effect as a write, so the two are named in `forbidden_in`,
as the families a view may not reach are.

## Acceptance

- **`compiler/pw-core/tests/query_reads.rs`**, 1 test, with a read as its
  control. It fails at 64140d7, the commit before.
- **`effects.rs`'s unit test `a_query_may_read_and_not_write`**: a query and
  a subscription may not write or open a transaction, may read, and a
  command may do both.
- **No rejected, rule or generality fixture's diagnostics change.** No query
  in the corpus writes. The store, kiokun and the accepted corpus check
  clean.
- **Mutation controls:** `scripts/query_read_mutations.py`,
  `just e10-query-reads`, 3 mutants.
