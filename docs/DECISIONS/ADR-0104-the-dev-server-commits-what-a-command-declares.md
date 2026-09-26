# ADR-0104: the dev server commits the events a command declares

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4,
ADR-0007, ADR-0019).

## Context

ADR-0007 has a command emit typed events "inside the same database
transaction as the state change". ADR-0019 is the outbox that commits the
two together. The dev server runs each command as the component the compiler
built (E10-I), and says of its command path that "no code here knows which
command it is running".

The events it committed were not the command's. After any cart write, it
committed `Events.CartChanged` carrying the session, whatever the command
declared. ADR-0101 found what that hid: a command that declared neither
`invalidates` nor `emits` updated the page here, and would update it
nowhere else. A command that declared another event would have committed
`CartChanged` as well.

## Decision

**The dev server commits the events a command declares.** Before the
command runs, the server reads the command's `emits` edges from the
compiler's graph, which it already holds for the materializer. Each edge
names an event, and its key gives the values: `current_session()` is the
session the request carries. The events are committed with the staged state
in one transaction, as before. A command that declares no event commits
none, and no entry hears of it.

A value the server cannot compute (any key but `current_session()`) is
refused before the command runs, and nothing is written. It is not guessed.

The graph is read once, into the server, where a test can replace it. It
used to be parsed at each drain.

**(ruling needed)** Computing a key is the command's work. An event
carrying the command's own arguments (`MenuChanged(store)`) needs the
compiled component to return its events, or to call an outbox host
function, rather than the server evaluating a key's text. The store
declares only `current_session()`.

## Acceptance

- **`spikes/own-renderer/server`, 2 new tests:**
  - `a_command_commits_the_events_it_declares`. `add_to_cart` commits
    exactly `CartChanged` for its session, and its cart's entry moves. With
    its `emits` edge removed, the write commits and the entry does not move.
  - `an_event_value_the_server_cannot_compute_is_refused`. A key of `item`
    is refused before the command runs, and the cart is unchanged.
- **The defect, shown at the commit before.** Both tests replace the
  server's graph, which the server did not hold before this change, so at
  e41e852 they do not compile. In a scratch worktree at e41e852, only the
  graph was made a field (the first two edits of this change). There
  `a_command_commits_the_events_it_declares` fails at its no-`emits` half:
  the entry moved, because the server committed `CartChanged` anyway.
- **The structural test** `the_commands_and_handlers_are_the_compilers` now
  also refuses an event named in the command path.
- **The store in a browser.** The Chromium browser suite, three full runs,
  passes as before: 101 tests each
  ([committed-events-browser.txt](../evidence/E10/committed-events-browser.txt),
  `just e10-browser chromium docs/evidence/E10/committed-events-browser.txt`).
  The store's commands declare `emits CartChanged(current_session())`, so
  the page sees the same frames.
- **Mutation controls:** `scripts/committed_events_mutations.py`,
  `just e10-committed-events`, 5 mutants.
