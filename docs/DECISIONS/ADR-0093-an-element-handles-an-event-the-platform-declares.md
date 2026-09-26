# ADR-0093: an element handles an event the platform declares

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §8.2).

## Context

Which event an `on:` attribute delivers is a platform fact, declared in the
platform's `events` module: `events.press(event: PressEvent)`, and likewise
`submit`, `input`, `keydown` and `change`. The module says so: "declaring a
new event is a change to this file rather than to a checker". The one rule
that reads it holds a handler to the event's type (PW0602), and skipped an
attribute whose event it did not find.

On 2026-09-26, at 18b9601, `<button on:clik={go}>` and `on:presss={go}`
checked. The runtime listens for a click whatever the event's name
(KNOWN_LIMITATIONS, ADR-0058), so the handler ran by accident. A runtime
that listens for the named event would never run it.

## Decision

**An element handles an event the platform declares** (PW5022). An `on:`
attribute that names no event in `events` is refused, and the message
lists the ones that are declared. A program with no `events` module, one
checked without a platform, declares no events, and its attributes are not
held to any.

## Acceptance

- **`compiler/pw-core/tests/declared_events.rs`**, 2 tests. The first, with
  the five declared events as its controls, fails at 18b9601, the commit
  before. The second, a program without a platform, holds there too, and
  guards the change.
- **No rejected, rule or generality fixture's diagnostics change.** Every
  `on:` in the corpus names `press`, `submit`, `input` or `keydown`. The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/declared_event_mutations.py`,
  `just e10-declared-events`, 2 mutants.
