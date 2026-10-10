# ADR-XXXX: a page's commands run in the order it sent them

Status: accepted under the owner's delegation of 2026-10-02, on a finding of
the soft navigation's evidence run. Date: 2026-10-10. Milestone: E14.
Completes ADR-0152's "presses run in the order they were made", which
ordered handlers and not what they commit; keeps its "a slow command does
not hold up the press after it": the next press is shown and its handler
runs, and its command commits after.

## Context

- **Found.** `navigate.spec.mjs`'s "a press made before the navigation is
  answered first, its answer first" timed out once in Chromium (`just
  e14-soft-navigation`, 2026-10-10), after its presses had been moved to the
  keyboard for another race. Run 600 times under load it passed every time;
  with the order's request held 400 ms on its way it failed every time, in
  all three engines. "Clear", pressed after "Place order", reached the
  server first, the cart was emptied, and the order was answered
  `NothingToOrder`: "Your cart is empty: there is nothing to order." The
  customer pressed to order and was told they had nothing to order.
- **Why.** The runtime starts each handler after the press before it
  (ADR-0152) and sends the first command first. But each request goes on a
  connection of its own, and the host ran a session's commands in the order
  they arrived (ADR-0271's turns). Under load, the test's own `route.fetch()`
  calls were enough to swap two requests; a phone's network does as much.
- **What others do** (read 2026-10-10):
  - Bayou's session guarantees (Terry et al., PDIS 1994), monotonic writes:
    "If Write W1 precedes Write W2 in a session, then, for any server S2, if
    W2 in DB(S2) then W1 is also in DB(S2) and WriteOrder(W1,W2)"; a server
    accepts a session's write only once its database holds the session's
    earlier writes.
  - Next.js "dispatches Server Actions one at a time per client. If a user
    triggers three actions in quick succession, the second waits for the
    first to finish": its client dispatcher's order (the Server Actions
    guide, next 16.3).
  - Replicache numbers each client's mutations; the server keeps the client's
    `lastMutationID`, applies only the next, in one transaction with it, and
    the client sends a mutation again until the server says it has it.
  - Phoenix LiveView sends a page's events on its one socket, handled in
    order.
- **One command at a time, sent by the page** (Next.js's way) orders them
  too, but a page that leaves keeps only what is in flight (ADR-0268's
  `keepalive`): two presses and a link followed at once would lose the
  second, and every press behind another waits a round trip before it is
  even sent. The order is kept where the commit is made.

## Decision

1. **A command names the one before it.** Each command's request carries,
   with its interaction (ADR-0121), `pw-after`: the interaction of the latest
   command its page sent and has not been answered, where there is one. A
   page names only its own: another tab's commands are another reader's, and
   a page's before it navigated were each answered first (ADR-0280).
2. **The host runs a command once the one it names has run.** One that names
   a command still running, or waiting for its own, waits for it, as long as
   it takes. A command has run once it is answered: committed or not,
   refused by its `requires`, or failed for good.
3. **One not yet arrived is waited for two seconds**: as long as a page's
   first resend of a request that found no server may wait (a second,
   ADR-0173), and as long again for its way. Past that the command is
   answered early, `409 {"committed": false, "early": true, "after": …}`,
   and nothing runs. A command that names one answered early is answered
   early at once; and one whose connection closed before it ran (charter
   §15.5's drop) has not run, so what follows it waits for it to be sent
   again.
4. **An early answer is no outcome.** The runtime sends the command again,
   with its interaction, naming none, once every command its page sent before
   it is answered or has failed; it is not one of its `retry` clause's
   resends.
5. **What is kept**: each session's last 64 commands' fates, as many as its
   outcomes (ADR-0121), never one still in flight; forgotten with the
   session.

## Found

1. **Two presses' commands committed in the order the network brought
   them** (Context).
2. **A test answered a press at the network, and the press after it waited
   for it.** `optimistic.spec.mjs`'s "one press rejected among two" answered
   its first press itself, so the server never saw it: the second waited the
   two seconds, was answered early and sent again, and the test read the
   page again before that. It waits for the second's commit now, as a test
   that reads the page again must.
3. **The refusal's announcer test and the build id met in a merge.** Merging
   master into the refusal's branch was clean as text, and
   `a_signal_page_holds_its_announcer` called `signal_document` without the
   build ADR-0300 added: clippy's test build found it.

## Alternatives

- **One command in flight a page** (Next.js): the page knows each answer, so
  nothing waits on a guess; but a page that leaves loses what it has not
  sent, and each press behind another waits a round trip to be sent.
- **A number per page** (Replicache's `lastMutationID`): the same order, and
  a counter per document kept on the server, with a page's numbering to keep
  across its retries. Naming the one before needs no counter: an interaction
  already names one press's command, and a retry keeps it.
- **Only commands that touch one entry in order**, by their `invalidates`: a
  press may read what the one before it wrote without writing it, an
  address set and then an order placed, and what a command reads is not
  declared.
- **Waiting without end** for one not arrived: a request lost on its way
  would hold the next for ever, a thread each.

## Acceptance

`just e14-command-order` records each (docs/evidence/E14/command-order.txt):

- **The host's order** (`spikes/own-renderer/server/src/order.rs`, 8 tests):
  one whose named command was answered runs; one waits for its named
  command to arrive and run; one whose named command never comes is answered
  early after the wait, watched so that a wait without end fails; one whose
  named command was answered early is answered early at once; a command
  that vanished unrun is waited for again; another session's command of one
  interaction is not the one named; one in flight is kept past the bound; a
  session forgotten takes its commands' fates.
- **The command route over HTTP** (4 tests): "Clear", there before the order
  it names, waits, and the order is placed with the cart's line, the cart
  cleared after; one whose named command never comes is answered 409,
  runs nothing, and what names it is answered early at once, and sent again
  naming none it runs; one after a command whose connection closed before it
  ran waits for it to be sent again; a session forgotten for idleness takes
  its commands' order.
- **`e2e/command-order.spec.mjs`, in Chromium, Firefox and WebKit**: the
  order's request slow on its way, the order is placed and the clear runs
  after it, naming it; held past the wait, the clear is answered early and
  sent again, one interaction, naming none; after an order the network lost,
  the clear is sent again naming none once the order has failed, and it runs;
  three presses, each later one on a quicker way, run in the order pressed,
  each naming the one before, and a press sent once all are answered names
  none.
- **`optimistic.spec.mjs` and `navigate.spec.mjs`**, in three engines.
- **`scripts/command_order_mutations.py`**: 20 mutants. And
  `command_retry_mutations.py`, `keepalive_mutations.py` and
  `connection_faults_mutations.py`, re-anchored, run whole by their recipes.

## Not claimed

- **A page's tabs, and its documents before a navigation**: two tabs'
  presses are two readers'.
- **More than one host**: a session's order is kept by the process that
  serves it; hosts behind a balancer would share it, or keep a session on
  one.
- **A command lost on its way from a page that left**: the one after it is
  answered early, and no page sends it again. Neither runs: the order is
  kept, and the second press is lost with the first.
- **An answer from something between**: a proxy's error answers a press the
  host never saw, and the next press waits the two seconds and is sent again.
- **An upload**, which goes by its own route (the uploads track), is no
  command the runtime sends, and is in no order.
