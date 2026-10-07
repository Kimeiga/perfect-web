# ADR-0255: a materialization may read another

Status: accepted under the owner's delegation of 2026-10-02; the compiler
half of ADR-0195's ruling 10. Date: 2026-10-07. Milestone: E14.

## Context

- **Ruling 10** (ADR-0195): "A materialization may read another. The build
  refuses a cycle, and invalidation propagates transitively. A feed needs
  this." A timeline is built from what others post: an entry per reader,
  read from entries per author.
- **A `depends_on` naming a materialization was refused**: PW5103, a clause
  naming another kind (ADR-0088). The clause was looked up among queries,
  subscriptions and resources, which are terms; a materialization is
  declared in the namespace views share.
- **A clause naming no node of the graph was taken as it was.** A name that
  is a declaration the graph has no node for, a function or a view, made an
  edge to nothing, and no check said so: `depends_on helper(session)`
  checked, and no write reached the fragment through it. So did `emits`,
  `invalidates` and `invalidates_on` naming one.
- **The write check looked one level down** (ADR-0101, ADR-0102): what a
  fragment reads through its `depends_on`, and nothing read through a
  fragment, since only a materialization has a `depends_on` (ADR-0092).
- **The runtime already follows reads** (ADR-0102): `Graph::reaches` walks
  each read edge at the key it supplies, and ends a cycle. So does the
  compiler's `Graph::affected_by`.

## Decision

1. **`depends_on` names a resource or a materialization**
   (`policy::keyed`): looked up among the terms' queries, subscriptions and
   resources, then among the materializations. Its arguments are checked
   against the one named's parameters (PW06), as a resource's are.
   `invalidates` names a resource only, and `emits` and `invalidates_on` an
   event, as before. PW5103's invariant is revised (revision 2).
2. **A cycle is refused**: PW5109 `materialization_cycle`, "a
   materialization depends on nothing that depends on it". Around a cycle
   nothing is rebuilt first: each would wait for the other, or be rebuilt
   from the other's stale entry. It is reported once, by the cycle's first
   member in path order, naming the shortest way back: "`Alpha` depends on
   itself, through `Beta`".
3. **A shared materialization reads only shared ones**: a `partition public`
   materialization depending on one that is not is PW5101, as one depending
   on a private resource is. What that one reads is checked at it in turn,
   and no cycle exists, so a shared entry is built from shared ones only.
4. **A clause naming no node of the graph is refused**: PW5103, reason
   `clause_names_no_node`, "`CartSummary` depends on `m.helper`, which is not
   a resource or a materialization".
5. **What one reads, the one reading it reads too.** Each
   materialization's reads are gathered in rounds, through what it depends
   on and through its own statements, until a round adds none; there are at
   most as many rounds as materializations, since no cycle is accepted. A
   write reaches each materialization of a chain (PW5106), whatever order
   they are declared in.
6. **The key audit counts what a materialization read separates**: its
   parameters, which the reader may pass, and the dimensions it varies by,
   which the reader must vary by too. `Top` reading `Base(store)`, where
   `Base` varies by locale and `Top` does not, would serve one locale's
   document to every locale.
7. **Invalidation propagates at run time as it did**: an event reaching a
   materialization reaches what reads it, at the key its read supplies. No
   runtime code changes; a test holds it.

## Acceptance

- **`compiler/pw-core/tests/materialization_chains.rs`, 9 tests**, each
  with its control: a materialization read by another; its arguments
  checked; a cycle through another and a materialization reading itself,
  each reported once; a shared one reading a private one, PW5101; a
  `depends_on` naming a function, PW5103; the key audit's gap at a
  materialization read's dimension, and none at the parameter passed; a
  write reaching each materialization of a chain, and the control with the
  query invalidated and an event both listen for; what a materialization
  reads in its own statements passing to its reader; and a reader declared
  before what it reads.
- **`runtime/pw-materialize/tests/reads.rs`**, one more test: an event
  reaching a materialization reaches the one reading it, at its key, and
  no other key.
- **`scripts/materialization_chains_mutations.py`: 10 mutants**, recorded by
  `just e14-materialization-chains`: 10 of 10 killed. The scripts with
  mutants within 30 lines of a change, run whole: `clause_key` (two
  re-anchored), `fragment_reach` (one re-anchored to the rounds, where a
  fragment's own reads are now gathered), `listener` (one re-anchored),
  `reads_through_calls`, `speculated_arms`, `stdlib`, `transition_values`
  and `write_invalidation`.
- **The whole workspace's tests**, the corpus among them.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **A materialization is inert at run time.** It has no body, type or
  generator, and no page reads one: none of the store's or the feed's pages
  does. What one materialization reads of another is checked, and its
  invalidation propagates; what it builds from it does not exist yet.
  "Materializations made real" is queued in NEXT, and the follows timeline
  (ADR-0256) is built on queries meanwhile.
- **Regeneration order.** A chain's entries are invalidated together. Which
  is rebuilt first, so the reader is rebuilt from the fresh entry and not
  the stale one, waits for materializations made real.
- **One cycle through a member is named.** A second cycle through the same
  first member is reported once the first is broken.
