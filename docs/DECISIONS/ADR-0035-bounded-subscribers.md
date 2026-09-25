# ADR-0035: bounded subscribers — a page that falls behind is told to reload

Status: accepted under the owner's instruction of 2026-09-24 ("finish the rest
of E10"). Date: 2026-09-25. Milestone: E10 gate item 3, "Memory leaks and
unbounded resource growth are tested under sustained load."

## Context

Measured at `41ccaf6` ([evidence](../evidence/E10/load-2026-09-25.md)):

- 3,000 commands left 3,000 consumed events in the materializer's outbox.
- 1,000 visitors who left, followed by 300 menu changes, left 600,000 queued
  frames and 1,000 materialized cart entries.

A subscriber's frames are dropped only when it acknowledges them, which a
closed tab never does. The frame queue was designed that way deliberately: an
in-flight poll from a reloading page must not take frames the new page still
needs.

## Decision

1. **A consumed event is deleted**, in `pw-materialize`. `consumed_at` was
   written and never read.
2. **A subscriber holds at most `MAX_WAITING` = 256 frames.** Past that, its
   queue is one `Recovery::Reload`, and further frames are dropped until it is
   served a new document. The protocol already had this answer; nothing
   produced it.
3. **A subscriber that has not asked for `IDLE` = 120 s is forgotten**, with its
   session's materialized cart entry. A missing entry is regenerated from state
   by `drain`, so nothing is lost. A live page asks at least every 2 s: the
   stream holds for 2 s and the long poll for 1 s.
4. **Serving a document takes a sequence number**, so a served page's cursor
   is at least 1. A poll with a nonzero cursor for a session the server does
   not hold is a page that missed frames, and it is answered with a reload
   batch. A cursor of zero is still a new subscriber.
5. **The runtime reloads on `Reload`**, once per document. Every other
   recovery is still recorded and not acted on, as the protocol requires.

The constants are engineering choices, not measured optima. 256 is far above
anything the suite queues: a burst of keyed-list operations queues tens of
frames.

## Acceptance

- `pw-dev-server` tests:
  - bounded state under steady commands and under churn;
  - an overflowing queue becomes one reload, with the next sequence number;
  - a forgotten page's poll through the real HTTP route, with its control.
- `pw-host`: 20,000 compiled calls with resident memory flat.
- `e2e/recovery.spec.mjs`, in three engines: a delivered reload reloads, other
  recoveries do not, and a forgotten session's poll is told to reload.

## Consequences

A visitor away for more than two minutes gets a fresh document on their next
poll, and they lose nothing: the document is rendered from current state.
Per-session data (a cart) is still kept. It is the store's data, not what the
server holds on a subscriber's behalf.
