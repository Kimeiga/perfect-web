# ADR-0103: a write reaches the fragments built on it

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4).

## Context

ADR-0101 holds a command to the queries that read what it writes. It exempts
a query with a positive `freshness`, whose entries expire (A-026). A
materialization does not expire:
- `regenerate` has one value, `on_invalidation`, so a fragment is rebuilt
  only when an event reaches it;
- the materializer consumes committed events and never reads a command's
  `invalidates`, which drops a query's entries and nothing else.

On 2026-09-26, at 4db3c48, this checked:
- a `public query Shop` with `freshness 5.minutes`, reading `Stores`;
- `materialize ShopFragment` depending on `Shop(id)` and listening for
  `StoreChanged(id)`;
- `rename_shop`, which writes `Stores` and emits nothing.

`Shop` expires in five minutes, and `ShopFragment` never does. It serves the
old store until some other event reaches it. The same holds in two more
cases:
- the command names the query in `invalidates`: the query's entries drop,
  and the fragment built from them stays;
- a fragment reads the store in its own body, since a materialize block may
  hold statements after its policies (`let store = Stores.get(id)`).

## Decision

**A write reaches the fragments built on it** (PW5106, whose invariant
already says so: "each cached reader of the data it changes"). A
materialization reads what its own body reads, and what its dependencies
read whatever their staleness windows. A command that writes a domain a fragment reads must emit an event
that reaches the fragment. Either the fragment listens for the event, or it
depends on something that does (ADR-0102). `invalidates` does not reach a
fragment.

A fragment's dependencies are read one level deep, since `depends_on` is a
materialization's alone (ADR-0092) and a query reads nothing through the
graph.

The diagnostic names the dependency the fragment is built from, or says the
fragment reads the data itself. Its repair names the events that reach the
fragment.

## Acceptance

- **`compiler/pw-core/tests/writes_reach_fragments.rs`**, 5 tests. Each fails
  at 3ae4738, the code at 4db3c48. The cases, each with its control:
  - a fragment over a query with a window, the command emitting nothing,
    against one emitting what the fragment listens for and a command writing
    another domain;
  - `invalidates` of the query without an event;
  - an event reaching the fragment through the query it depends on, against
    an event nothing hears;
  - the underline, related spans and repair, for a fragment built from two
    queries, only one of which reads what the command writes;
  - a fragment reading the store in its own body.
- **ADR-0101's tests pass unchanged**, and its 13 mutants are still killed.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change,
  with the library alone or with the accepted modules the harnesses add. The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/fragment_reach_mutations.py`,
  `just e10-fragments-reached`, 7 mutants.
