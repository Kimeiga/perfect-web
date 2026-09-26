# ADR-0092: a dependency-graph clause belongs to a declaration that can mean it

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4;
ADR-0007).

## Context

ADR-0007 made invalidation explicit. A command emits typed events "inside
the same database transaction as the state change", and declares which cache
entries it invalidates. A resource or a materialization declares the events
it listens for, and a materialization the resources it depends on. ADR-0088
and ADR-0091 check what each clause names and gives. Nothing checked where
it is written. On 2026-09-26, at a969e2b, each of these checked:
- a query that `emits` and `invalidates`;
- a command that `invalidates_on` and `depends_on`;
- a `fn` that `emits`;
- a page that `invalidates`.

The graph drew the query's and the command's edges, which nothing reads: the
materializer consumes the events commands commit, and the entries resources
and materializations are keyed by. A `fn` and a page are not in the graph,
so what they wrote was dropped. The architect ruled on 2026-08-20, for an
effect's `host` clause, that "semantically dead syntax survives for a long
time if it still parses"; a known head on a declaration that cannot mean it
is a diagnostic, not a silent ignore (PW0332).

## Decision

**A dependency-graph clause belongs to a declaration that can mean it**
(PW5105). `crate::policy::declared_by` says which:
- `emits` and `invalidates` belong to a command;
- `invalidates_on` belongs to a query, a subscription, a resource or a
  materialization;
- `depends_on` belongs to a materialization.

Every clause in the corpus is already in its place.

**(ruling needed)**
- Whether a query may `depends_on` another resource: a derived resource.
  None does, and it is refused.
- Which declarations every other policy head belongs to. This table covers
  the four clauses the graph reads; the rest are unchecked.

## Acceptance

- **`compiler/pw-core/tests/clause_places.rs`**, 3 tests, each with
  controls. All 3 fail at a969e2b, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/clause_place_mutations.py`,
  `just e10-clause-places`, 5 mutants.
