# ADR-0287: a handler is held where it runs

Status: accepted under the owner's delegation of 2026-10-02, on a finding of
the integrator's of 2026-10-09 (probing ADR-0283). Date: 2026-10-09.
Milestone: E14, the DoorDash customer side.

## Context

- **A page placed at `build` whose button sends a command was refused**:
  "`database.write<Thing>` is not available at placement Build" (PW5005),
  for `on:press={() => SaveAll()}` and for `on:press={SaveAll}` alike.
- **Its contract said otherwise.** The page's contract defers its handlers'
  work (E8-0, ADR-0283) and allows `build`.
- **The handler's own rule says otherwise** (ADR-0113): a handler runs in
  the browser when its element is pressed, and "a call to [a command]
  performs nothing here": the command is its own component, at its own
  placement. That rule holds a handler to the browser, and refuses one that
  writes the database itself.
- **The row check says otherwise**: a view declared `!{}` with such a button
  is not refused for the write.
- **The declared-placement check alone counted it.** `effect_rows` held
  every source in the body to the declaration's declared world, a handler's
  among them.
- **Why it matters now**: the DoorDash menu is a page every customer sees
  alike, built ahead or cached at the edge, with an Add button on each dish.

## Decision

- **A page's declared placement grants what the page does where it is
  placed**: what it renders. Its handlers' work (an `on:` attribute's
  lambda, or the declaration it names) is held where handlers run, in the
  browser, by ADR-0113's rule.
- Nothing else moves: a handler that writes the database itself is refused
  as before, by the handler's rule; what a page renders is held to its
  placement as before.

## Alternatives

- **Check a handler's work against the browser here too**: that is the
  handler rule's work already; a second report of one defect is the
  duplicate R-026 taught the check to avoid.

## Acceptance

- **`compiler/pw-core/tests/handlers_where_they_run.rs`**, three tests: a
  page built ahead may have a button that sends a command (a lambda, and
  the command named); a handler that writes itself is refused once, where
  it runs; what a page renders is still held to its placement.
- **`scripts/handlers_where_they_run_mutations.py`**: the skip removed fails
  the first two.

## Not claimed

- **A frame phase's work** (`post_paint`, `frame`) is still held to a page's
  declared placement, though it too runs in the browser. No program places
  a page with frame phases anywhere but the browser; queued.
