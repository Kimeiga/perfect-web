# ADR-0256: an entry written `_` is every entry at the rest

Status: accepted under the owner's delegation of 2026-10-02; the first half
of ADR-0195's ruling 10, and a correction it found. Date: 2026-10-07.
Milestone: E14.

## Context

- **Ruling 10** (ADR-0195): "A bare `invalidates Cart` stays refused. Every
  entry is written out as `Cart(_)`, as `InventoryChanged(id, _)` is."
- **`_` in an `invalidates` key was a name**, and resolved to nothing
  (PW0021). A command dropped one entry of a query, the one its key
  computed (ADR-0209), and no more. The follows timeline needs more: a
  follow changes the follower's timeline at every limit it was read.
- **A bare key was refused as a call passing no argument** (PW0604), and
  its repair said to pass the missing arguments.
- **An invalidation dropped a private entry in the committing session's
  partition alone.** The host keyed what it dropped as it keys a read, with
  the committing session's partition. A private query keyed by something
  other than the session (a thread cached per reader, a price list by
  store) kept every other session's copy, and no other session was told.
  No program of the repository's has such a query, so nothing showed it.
- **An event leaving a position unbound dropped every entry of the query**
  (ADR-0091): sound, and more than it must.

## Decision

1. **In an `invalidates` key, an argument written `_` is every value at
   its position**: `invalidates Timeline(current_session(), _)` drops the
   session's entries at every limit. It is a whole argument, by position or
   by name (`limit = _`); `_` inside one, `_ + 1`, is a name as it was.
2. **In an `emits` key it is refused**, PW0021, by what it leaves out:
   "`emits Followed` is given no value at `follower`: `_` is none". A
   listener matches an event by each value it carries.
3. **A bare key stays refused**, PW0604, and its first repair says how
   every entry is written: "write `Counted(_, _)` to drop every entry, or
   the key of the one the command changes".
4. **The command computes the rest** (ADR-0209): it calls the
   invalidations' function that takes the values it gives,
   `pw:host/invalidations#m-grid-EVERY-1`. A WIT name's later words may be
   uppercase, and `wit::ident` writes none, so no query's own name is
   another's shape. The contract records the positions, `every: [1]`, and
   the host restores each, `None` at those.
5. **The host drops the entries at the values given, whatever the rest,
   in every session's partition** (`names_entry`), from the cache's
   `invalidate_where`. A PostgreSQL feed keeps a `_` in its outbox as
   `null`. An event's unbound position is the same: the entries at the
   values it binds are dropped, and no others.
6. **Another session is told** (ADR-0219) of a shared entry dropped, and
   of a private one no key position pins to the committing session by its
   id; an entry keyed by the session is its alone, as before.

## Acceptance

- **`compiler/pw-core/tests/every_entry.rs`**: an entry written with a
  wildcard checks, by position and by name, and a name in a key resolves as
  it did; `_ + 1` is a name; `emits Followed(_, user)` is refused at
  `follower`; a bare key is PW0604, with the repair.
- **`compiler/pw-conformance/tests/invalidations.rs`**: a command leaving
  a position to every value calls `m-grid-EVERY-1` with the value it gives,
  and its contract and its WIT say so.
- **The development server's tests** (`src/tests/every_entry.rs`): a
  session's timeline dropped at every limit and another's kept;
  `Timeline(_, 20)` dropping each session's; a like reaching another
  reader's private thread, told; an entry named by the values given,
  whatever the rest, in any partition. And on PostgreSQL
  (`on_postgres_an_entry_written_with_a_wildcard_is_every_entry`), through
  the outbox's `null`.
- **`runtime/pw-resource`**: every entry a predicate names is dropped, and
  no other.
- **`scripts/every_entry_mutations.py`: 11 mutants**, recorded by `just
  e14-every-entry`: 11 of 11 killed.
- **The scripts this changed run whole**: `command_invalidations` (4),
  `cross_session` (8) and `feed_postgres` (4), each re-anchored to the new
  drop, and those beside it, `committed_events`, `optimistic_posts`,
  `shared_output`, `slots` and `query_blocks`: every mutant killed. One
  test pinned the old reading, `value_relations.rs`'s `_` in an
  `invalidates` key naming nothing; it holds the new one, and `emits`'s.
- **The whole workspace's tests**, PostgreSQL's among them, and the chain on
  the push (ADR-0245).

## Not claimed

- **`_` in a `depends_on` key**, a materialization reading every entry of
  another, is a name, and resolves to nothing.
- **A query keyed by a session's user**, its other sessions told by the
  user and not by the session: the identity track's.
