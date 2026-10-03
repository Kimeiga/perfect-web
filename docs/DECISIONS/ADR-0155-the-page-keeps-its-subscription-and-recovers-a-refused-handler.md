# ADR-0155: the page keeps its subscription, and recovers a refused handler

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14, from an audit of the canonical store against charter §15
(`docs/research/charter-15-store-audit.md`). Three runtime defects.

## Context

The audit compared the store with charter §15, requirement by requirement. It
found three defects in the browser runtime:

1. **A page whose subscription failed once stopped hearing changes.** On a
   failed request, `subscribe()` meant to fall back from the stream to the
   long poll. It did that only when `chosenTransport()` answered
   differently, which it never does. So it returned: a dropped connection, a
   server restarting or a network that blinked left the page showing what it
   last heard, with no sign that it had stopped. Charter §15.5 lists
   "forced reconnect" among the controls a store must survive.
2. **A press on a refused handler did nothing.** The resume decision
   (`pw-resume`) refuses a handler a document names that this build does not
   have, as a document a cache kept from an earlier build does, and it says
   how to recover. The runtime logged the recovery and bound nothing, so the
   button stayed dead. Charter §15.6 test 16: "Handler version mismatch
   recovers through safe reload or refetch."
3. **Every recovery was read one place off.** `pw-resume-wasm` reports
   recoveries from 0, `RefetchRegion` first. The runtime's table began with a
   "none" the ABI does not have, so:
   - a region to refetch read as "none";
   - a document to reload read as "rerender-private-slot".

   Nothing acted on a recovery then, so only the log was wrong. Fixing the
   second defect without this one would have made a reloadable page do
   nothing.

## Decision

1. **A failed subscription request is asked again**, from the cursor the
   page holds, after a pause that grows from 250 ms to five seconds.
   - A stream that fails twice in a row falls back to the long poll, and
     says so, unless a test pinned the adapter.
   - A page that is leaving (`pagehide`) asks nothing again.
2. **A press on a refused handler performs its recovery.**
   - A recovery a document read again satisfies reads the page again, and
     does not replay the press: a reload, a region to refetch, a private
     slot to render again, or an interaction to retry. A mutation the user
     did not ask for again is how a version mismatch becomes a double charge.
   - A recovery that needs the user's consent asks first.
   - An irrecoverable one leaves the button inert, and says so on it
     (`data-pw-handler-error`).
   - A page read again within ten seconds for the same address is not read
     again: a server still sending the stale document would otherwise reload
     it for ever.
3. **The recovery table is in `pw-resume-wasm`'s order.**

## Acceptance

Browser tests, in Chromium, Firefox and WebKit:
- `e2e/transport.spec.mjs`, for both adapters: a page whose first
  subscription request is dropped hears a menu renamed at the server.
- `e2e/recovery.spec.mjs`:
  - a press on a handler from another build reads the page again once, and
    the fresh page's press works;
  - a page still stale after that is not read again, and its button says
    why.

Also:
- `scripts/runtime_recovery_mutations.py`: each defect put back fails a test,
  three of three (`just e14-runtime-recovery`).
- The browser suite.

## Not claimed

- **A command retried on a transport failure** (§15.4). ADR-0154 makes it
  safe, and it is not done.
- **A region refetched in place.** A reload stands in for it, which the
  charter allows ("reload or refetch").
