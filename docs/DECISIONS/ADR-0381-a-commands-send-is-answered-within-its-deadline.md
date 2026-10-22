# ADR-0381: a command's send is answered within its deadline, or it is no answer

Status: accepted by the integrator under the owner's delegation, 2026-10-21.
Milestone: E14 (charter §7.6, §15.4). Amends ADR-0173's "a request that hangs
waits" and extends ADR-0305's early answer.

## Context

- **A press could stay pending for ever.** `send()` in
  `spikes/own-renderer/public/pw-runtime.mjs` gave `fetch` no signal, and the
  answer's body none. A connection the network held open, an answer that
  never began and a body that never ended rejected nothing, so neither the
  `retry` clause's resend nor the press's failure was ever reached (ADR-0173's
  "not claimed"). A local diagnostic of 2026-10-21 held one request for 1.5 s
  and saw its line stay, with no failure marker.
- **The page's order made one such request every later one's** (found writing
  the tests). A command names the latest command its page sent and has not
  been answered (ADR-0305). Where that one never reached the host, the host
  answers the later one early, and the runtime sent it again only once the
  earlier one was answered or had failed: never. On the unchanged source a
  clear pressed after an order held at the network was answered early and
  never sent again (`e2e/command-deadline.spec.mjs` at `e7664d2e`: its sends
  named the order, and nothing followed). A handler's navigation waits for
  each press made before it (ADR-0280), so the page could not leave by its own
  handlers either.
- **The charter asks for it.** §7.6 requires explicit semantics for
  cancellation, timeout, retry, backoff and jitter, and "No fire-and-forget
  promise by default". A query's `timeout` is a budget its host enforces
  (ADR-0109); a command had no bound at all. The failure census's RT07 says
  what a deadline can and cannot do: cancellation changes observation and
  lifetime, not an unobserved remote commit outcome
  ([obligations](../../research/failures/OBLIGATIONS.md)).
- **The development host** (read 2026-10-21, `spikes/own-renderer/server/src/main.rs`):
  one thread and one connection per request (`connection: close`). It never
  notices that a client gave up, and a command runs to its end. A request sent
  again with its interaction is answered with the first's outcome, waiting
  while the first runs (ADR-0121, `try_command`). It holds a command up to two
  seconds for one it names that has not arrived, and without bound for one it
  names that is running (ADR-0305).
- **What others do** (read 2026-10-21):
  - `fetch` has no deadline of its own. `AbortSignal.timeout()` aborts a
    request with a `TimeoutError`, by active time: paused while the document
    is in the back-forward cache (MDN, Baseline 2024).
  - ky gives each attempt a `timeout`, 10 s by default, "applied independently
    to each retry", and does not retry a timed-out request unless asked
    (`retryOnTimeout: false`): a timed-out request may have been applied.
  - .NET's standard resilience handler wraps a 10 s attempt timeout and three
    jittered retries in a 30 s total timeout.

## Decision

1. **Each send of a command has a deadline for its whole answer**: ten
   seconds from the send to the answer's last byte, a success's body
   included. Past it the request is aborted (an `AbortController` and a
   timer, which every browser the runtime runs in has; `AbortSignal.timeout`
   is Safari 16's), and that is no answer, as a request the network dropped
   is (ADR-0173). It is sent again with its interaction while the command's
   `retry` clause allows; after the last, the press fails as unreachable,
   its speculation taken back and its control told (ADR-0302). A refusal's
   or an early answer's body (403, 409) is read under the same deadline, and
   one that stalls is a refusal, as one cut off was.
2. **Sending a timed-out request again is safe here.** Every command a page
   sends is `idempotent_by InteractionId` (PW0338), and the host answers a
   repeated interaction with the first's outcome (ADR-0121). A send given up
   after its command committed is answered, sent again, with that commit.
   This is what ky cannot assume of a request.
3. **A send held for the one before it is waiting its turn.** Given up while
   it named a command before it, whether that one is still unanswered or has
   since failed, it is no outcome and no resend of the `retry` clause's: as
   an early answer is (ADR-0305), it is sent again once each command its page
   sent before it is answered or has failed, naming none. (Amended after the
   review below: first written as "while the command it names is
   unanswered", which a predecessor's own earlier deadline could end first.)
4. **So every press settles in bounded time**: within its sends' deadlines and
   the waits between them (ADR-0173's), and, where its command waits its
   turn, the bound of the commands before it. Ten seconds with no `retry`;
   about 33 s for the store's `transport_only(max = 2, jitter = true)`.
5. **The deadline is the platform's.** A command declares none. `?deadline=`
   lowers it, so that a test reaches it in a second or two, as `?keepalive=`
   lowers the keepalive budget (ADR-0268): never above ten seconds, never
   below half of one, so that a link cannot make a page's presses fail at
   once, and what is no time above zero asks for nothing. A module's load has
   its own deadline and `?load-deadline=` (ADR-0382).
6. **The host's test fault** `POST /bench/hold?next=command&at=before|after|body`
   holds the session's next command connection open, with no answer before
   the command runs or after it has committed, or with its answer's head and
   half its body, until the page closes it, and at most 30 s. It sits beside
   ADR-0175's `/bench/drop`, which closes the connection instead.

## Alternatives

- **One budget for the whole press, shared by its sends**, as a query's
  `timeout` is a budget (ADR-0109). A first send that hangs spends it, and the
  resend that settles an unknown outcome by its interaction is never made.
  Divided among the sends, a short budget gives each less than the host's
  two-second wait for a command not yet arrived. Per-send deadlines are ky's
  and .NET's attempt timeouts.
- **A `timeout` declared on every command**, as PW0338 requires
  `idempotent_by`. Every program, the guide and the benchmark's stores would
  change for a number most would copy, and the declaration needs a rule of
  its own: above zero, within what a browser's timer holds (2^31 - 1 ms), and
  longer than the host's wait for a command's turn. The platform's bound comes
  first; a declared one can follow with its own decision.
- **No resend after a timeout** (ky's default): a press whose command committed
  would be told it failed, though its interaction can find out.
- **Thirty seconds a send**: a press of the store's would settle only after
  about 93 s, long after its reader has left or reloaded.
- **Waiting on**: the failure this decision removes.

## Acceptance

`just e14-command-deadline` records `docs/evidence/E14/command-deadline.txt`:

- `e2e/command-deadline.spec.mjs`, 10 tests, in Chromium, Firefox and WebKit:
  - `?deadline=` and `?load-deadline=` each lower their own, never raise it
    and never below half a second;
  - a send held at the network is given up at its deadline, timed from its
    own send to its abort, sent again with its interaction, its line shown
    meanwhile, and settles as the server's;
  - a connection the host holds before the command runs, after it committed,
    or partway through its answer's body: sent again, the resend answered
    whole, the press settled, and one line;
  - a press none of whose three sends is answered fails as unreachable, each
    send given up at its deadline, its line taken away;
  - a clear held for an order that never reached the host waits its turn and
    runs once the order is given up, and no order is placed;
  - a send held for the order is no retry of its own: the clear's request
    and both resends follow it;
  - a pick made after one with no `retry`, held by the host for it, runs
    once the first has failed;
  - a command with no `retry` fails at its one send's deadline.

  The first eight failed at `e7664d2e`, and the two added after the review
  at the runtime before its repairs, each at an assertion; all pass here.
- The host's fault: `tests::a_held_command_connection_is_answered_by_nothing_until_it_is_given_up`,
  which also requires that a hold ends with its connection.
- `scripts/command_deadline_mutations.py`: 18 controls, strict native
  receipts; a build failure, a timeout or an incomplete inventory earns no
  kill.
- `scripts/command_order_mutations.py`'s two early-answer mutants are
  re-anchored, the turn's lines now standing in two places; the campaigns the
  change reaches run whole.

**Recorded at `bfda9ae5`** (2026-10-21): CI
[39658029334](https://github.com/studiokimeiga/perfect-web/actions/runs/39658029334)
passed x64, ARM and licenses; verify
[39658074296](https://github.com/studiokimeiga/perfect-web/actions/runs/39658074296)
ran the 141 recipes the change reaches, with `e14-pleris-controls`, and the
three full browser jobs, and its 142 records were fetched whole. The spec
passed 24 of 24 (eight tests in three engines); the strict campaign killed
14 of 14, none refused; `e10-browser`'s three runs each passed 1,282 with 20
skipped; every reached campaign killed all its controls, command-order's 20
and keepalive's 8 among them; the benchmark's twelve Pleris controls hold.
The local red baseline at `e7664d2e` failed all eight tests, each at an
assertion. Not landed: landing is the owner's to approve.

**Reviewed 2026-10-21** by an independent read of the code after that
record. Repaired: a held send's race with its predecessor's own deadline
(decision 3), a deadline that `AbortSignal.timeout` would have left out of
older browsers, a link able to lower the deadline to nothing (decision 5),
and test assertions that measured the resend's jittered wait rather than the
deadline, or asserted no failure before the press had settled. The record at
`bfda9ae5` predates the repairs; the stack is verified again at its head.

## Not claimed

- **An outcome told as unknown.** A press whose sends are all given up is told
  "This could not be sent", ADR-0302's words for an unreachable press, though
  it may have reached the host and committed. The page then shows what its
  subscription brings. Nothing tells the reader the outcome is unknown, and a
  second press is a new interaction.
- **A command given up may still run after those behind it.** The host never
  hears that a page gave up: a command still running, or arriving late, can
  run after the page has sent the next one naming none, as ADR-0305's early
  answer already allows.
- **The host's side is unbounded.** A command's run, the session's turn, the
  database pool and the wait for a running predecessor have no deadline, and
  each resend of a request the host is still running parks another thread
  behind it, at most the `retry` clause's `max`.
- **A page gone stale** (ADR-0318): a resend that arrives while its first is
  still running, from a document the host no longer holds, is refused rather
  than answered with the first's outcome, which the host reads only once it is
  finished. The deadline makes that window likelier.
- **A hidden page's timers fire late** (browsers throttle them, Chrome to once
  a minute under intensive throttling), so the bound is the browser's timer's.
- **A module a press needs** is ADR-0382's: before it, one that never loaded
  held its press, and through ADR-0152 the presses after it.
- **A refusal's or an early answer's body that stalls** (403, 409) is told as
  a refusal, as one cut off was: the status came, and the body that says
  what it means did not.
- **A link can lower the deadline** to half a second with `?deadline=`, and
  so fail a command slower than that on that page.
- **A `timeout` declared on a command** is still refused (PW5105).
