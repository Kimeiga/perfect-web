# ADR-0209: a command computes the entries it invalidates

Status: accepted under the owner's delegation of 2026-10-02. It completes
ADR-0195's ruling 11, after ADR-0207 and ADR-0208. Date: 2026-10-05.
Milestone: E14, the app layer.

## Context

- **Ruling 11 says "the server never evaluates a key's text".** ADR-0208 made
  that true of `emits`. It was still false of `invalidates`.
  - The server read each `invalidates` edge's key off the compiler's graph.
  - `current_session()` was the request's session.
  - Anything else dropped every entry of the query (ADR-0127). That is sound,
    since it drops more than it must, but a feed whose like command
    `invalidates Post(post)` would drop every post's kept answer on every
    like.
- **The same mechanism serves.** An entry to drop is, like an event, staged
  with the command's writes and acted on once they commit, never before: a
  reader that refilled the entry before the commit would keep the old value.

## Decision

- **An entry is a function of the platform's invalidations.**
  - `invalidates Cart(current_session())` calls
    `pw:host/invalidations#store-page-cart`, taking the query's parameters.
  - It is named by the query's whole path, since two modules' queries may
    share a name: the store has `Resources.Cart` and `store.page.Cart`.
- **The command calls it before its body**, with the values it computed. Its
  `emits` and `invalidates` keys are evaluated in the order they are
  written.
- **It is under the outbox's authority, `outbox.write`.** The outbox keeps
  the entry until the writes commit. A command that invalidates performs
  it, as one that emits does.
- **What a key performs is the command's** (ruling 9, for `invalidates`): a
  new context, `Invalidated`, contributes its effects and its imports.
- **The contract's import carries the query's path**, `invalidates:
  "store.page.Cart"`.
- **The server stages it and drops the entry once the writes commit.**
  - The entry is keyed by the values as the command computed them, through
    the query's `key` positions.
  - The `invalidates` edge's text is no longer read. The server now evaluates
    no key of a command.

## Alternatives

- **Invalidations as functions of the outbox's own interface.** An event's
  function is named by the event's name and an entry's by the query's path,
  so the two would share one namespace for no reason. Two interfaces under
  one authority keep each kind's names its own.
- **A key the server cannot compute drops the whole query**, as it did. That
  is sound, but it grows with the data: every like drops every post.

## Acceptance

Recorded by `just e14-command-invalidations` in
`docs/evidence/E14/command-invalidations.txt`:

- **`compiler/pw-conformance/tests/invalidations.rs`**, 2 tests.
  - A command `invalidates Spot(to + 1)` and `emits Moved(to)`. Given 41, it
    calls the invalidations with 42, then the outbox with 41, then the data
    layer.
  - Its contract imports `m-spot` with `outbox.write`, and its world takes
    an `s64`.
- **The dev server's test** `an_invalidated_entry_is_dropped_by_the_key_the_command_computed`:
  store 47's `Menu`, handed over as a command does, is read again, and store
  48's stays kept.
- **The store's commands, against their references** (`oracle.rs`): each
  hands its cart's entry, then `CartChanged`, then runs its body.
- **`scripts/command_invalidations_mutations.py`**: 4 mutants.
- **The store's WIT, contracts and components** are emitted again.

## Not claimed

- **`invalidates Cart(_)`**, every entry, is ruling 10's: a key position
  left open is refused now, as it was.
- **A materialization's `depends_on` keys** are read by the materializer
  from the graph. They name a dependency, and nothing evaluates them.
