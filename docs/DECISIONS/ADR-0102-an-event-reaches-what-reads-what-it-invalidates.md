# ADR-0102: an event reaches what reads what it invalidates

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4, §14 M6).

## Context

Charter §9.4 treats incremental materialization as "a materialized view over
resources, not a route timer". It says `MenuChanged(store_47)` "should
invalidate only affected resource snapshots and page fragments". Charter §14
M6 task 5 has the materializer consume committed events and find "affected
resources/fragments". The compiler's `Graph::affected_by` gives an event's
reach as every listener, "plus anything reading what those produce", and
follows reads transitively.

The materializer did not. Its drain invalidated an event's direct listeners
and no entry that reads them. A-009's `MenuFragment` depends on
`Resources.Store(id)`, which listens for `StoreChanged(id)`, and does not
listen for `StoreChanged` itself. A store's change dropped the store's entry
and kept the fragment built from it, with no error. The store's own fragment
(`examples/store/menu.pw`) listens for all three events itself, which is why
the demo never showed this. ADR-0101 recorded it on 2026-09-26.

## Decision

**An event reaches what reads what it invalidates.** `Graph::reaches(node,
event, values, key)` answers for one entry. The event reaches it in either
of two cases:
- the node listens for the event and binds the entry (`listens`, ADR-0091);
- the node reads a node the event reaches, at the key its read supplies.

The drain invalidates each entry the event reaches. The `NoSubscriber` case
is unchanged: an event nothing listens for reaches nothing.

**A read's key** is the reader's arguments as written. `Store(id)` passes
the reader's `id` as the store's first parameter, so store 47's change
reaches fragment 47 and not fragment 48. A position that the read does not
fill from a parameter is not known, and matches any value. That covers
three cases:
- an expression, such as `Banner(current_region())`;
- a position left out, such as `depends_on Banner`;
- `_`.

An entry invalidated needlessly costs a regeneration; one missed is a stale
page.

**Reads are followed along each path.** A node already on the path is not
followed again, so a cycle of reads ends. The same node reached by another
path is followed at that path's key.

The compiler's side is unchanged. `depends_on` is a materialization's alone
(ADR-0092), so a query has no reads. ADR-0101's reach, which asks whether a
command's event reaches a query, would give the same answer through
`affected_by`, and no test could tell the two apart.

**(ruling needed)**
- **Unknown positions match every value.** The alternative is to refuse a
  read whose key the reader does not bind.
- **What a page reads.** A page's reads are in the graph and would be
  followed like a fragment's, but no caller materializes a page. The dev
  server's entries are the cart and the menu fragment. A page already
  rendered in a browser learns of a change from its bindings, not from here.

## Acceptance

- **`runtime/pw-materialize/tests/reads.rs`**, 5 tests. Each fails at
  7cd2904, the code at fa80d50. The cases:
  - an event reaching a fragment through two reads, store 47's and not store
    48's;
  - a read that supplies no key, reaching every entry;
  - a reader of what the event does not reach, and an event nothing hears;
  - a cycle of reads;
  - one node read at two keys by two paths.
- **Every other materializer test passes unchanged:** the M6 gate (`gate.rs`,
  including store 47 of 1000), `listeners.rs`, `scale.rs`,
  `concurrent_drain.rs` and `graph_contract.rs`. `listeners.rs`'s header
  named ADR-0089 for ADR-0091, and says ADR-0091 now.
- **ADR-0091's mutation controls:** two of them anchored in the code this
  replaced. They are re-anchored, and all three runtime mutants are still
  killed.
- **Mutation controls:** `scripts/read_through_mutations.py`,
  `just e10-read-through`, 7 mutants.
