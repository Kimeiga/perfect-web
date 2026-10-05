# ADR-0218: a host serves any program, and its data is the deployment's

Status: accepted under the owner's delegation of 2026-10-02. It is the first
step of the feed reference app, which the owner moved ahead of the remaining
rulings on 2026-10-05 (NEXT 22, 24). Date: 2026-10-05. Milestone: E14.

## Context

- **The development server served one program, the store.** About 7,000
  lines, not counting tests, mixed the host's machinery with the store's
  data:
  - The host's machinery: documents from page plans, queries through compiled
    components and the cache, commands through compiled components and the
    outbox, patches, speculations and handlers.
  - The store's data: carts, the menu, orders, the recommender, estimators,
    the notice board, faults.
- **A second program failed at once.**
  - `from_build` loaded `store.page.StorePage`'s plan unconditionally.
  - The command path staged the store's cart lines and committed the cart's
    total.
  - A session's entry was the cart's, and the shared fragment was the menu's.
  - The feed's draft checks and builds, and could not be served.
- **This is the gap the feed exists to find** (ADR-0210's order: the feed
  right after the soundness defects). Every program's data layer was written
  in the host's language, against the store's staging.

## Acceptance (the first commit)

- **The server's tests, 122** with the start check's. One build imports
  `store:data/stores#nowhere` and is refused at start.
- **The workspace, and the browser suite**, 728 in three engines, unchanged.
- **The mutation controls of the moved code**, recorded again: availability,
  cart lines, command answers, estimate range, home, last known good, menu
  categories, orders, query values, slots, test controls.

## Decision

- **A host serves any built program.** Its machinery reads the build:
  - pages from their plans;
  - queries and commands from their components and contracts;
  - events and invalidations from the outbox (ADR-0208, ADR-0209).
- **A program's data is the deployment's: a `DataLayer`.** It supplies:
  - the host functions the program's contracts import, for reads, and for a
    command's writes, which stage until its transaction commits;
  - what the commit writes in the materializer's transaction;
  - the grants a node gives beyond the platform's;
  - the store's own hooks, such as menu broadcasts and faults, which are
    test controls and not the host's.
- **The host refuses a build that imports a host function no layer
  implements**, at start, as it refuses a build whose handlers were not
  compiled.
- **The split is made in place, in steps, each keeping every test green.**
  The store's tests, about 120 of the server's and 741 in browsers, are what
  says the host did not change. The first step moves the store's state into
  `StoreData`, with no change in behaviour.

## Done in this ADR's first commit

1. **The store's state** moved into `StoreData` (`server/src/store.rs`),
   thirteen fields.
2. **The store's reads**: the data layer, its faults, the order and the
   estimate, and the catalogue (`StoreData::reads`). A query's path only adds
   the platform's session and counts the calls.
3. **A command's staging** (`StoreData::begin`, `Staging`).
   - `command_answered` runs the component with the layer's operations and
     the platform's.
   - It commits the layer's rows and the outbox's events in one transaction,
     then publishes what was staged.
   - The cart's lock is held, as before, from the call to the commit.
4. **The grants and the start check.**
   - A node grants the platform's `session.read` and `outbox.write`, and the
     layer's own.
   - `from_build` refuses a build that imports an operation the layer does
     not supply. The layer's operations are read from the functions it
     builds, not from a list.

Twenty-five mutants anchored in the moved code now name `store.rs`
(`STORE_DATA`); `slots_mutations.py`'s anchors read `self.store.menu`.

## Not claimed yet

- **The steps after these**: the store's test routes, the session's entry,
  the default page and its presentation, the shared fragment. Then the trait
  the feed's data layer implements, and the feed.
- **Live public data across sessions.** A commit tells the committing
  session's documents. Another session's open timeline is not told of a post
  yet. The feed needs it.
