# ADR-0175: a network error and a forced reconnect, made by the server

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's sixth gap (charter §15.5): a one-shot network
error and a forced reconnect, which were partial.

## Context

- **A network error existed only inside browser tests.** A test aborted a
  request with Playwright's routing (`route.abort`). The server had no
  control a deterministic test could arm, and nothing let a test drop a
  connection after the server had committed.
- **A forced reconnect was the browser's alone too.** `transport.spec`
  aborted the page's subscription request in the browser. The server could
  not end a subscription it held open, and nothing showed that a change made
  while a page was cut off still reached it.
- **A held subscription already ends every two seconds** (80 polls of
  25 ms) or one (the long poll), and the page asks again. A reconnect the
  server forces must end it now, and refuse the next for a while.

## Decision

1. **`POST /bench/drop?next=command&at=before|after`: a one-shot network
   error.** The session's next command connection is closed with no answer:
   - `before`: the request read, and nothing run;
   - `after`: the command run, and committed if it did, and its answer not
     sent.

   Once. The page sends the request again (ADR-0173), with the same
   interaction, and the host answers it with what happened (ADR-0121).
2. **`POST /bench/reconnect?for=ms`: a forced reconnect.** The session's
   open subscriptions, a stream or a long poll, end now with no more bytes.
   Each new one is closed at once, unanswered, for `ms` milliseconds, a
   stream before its head as a long poll is. With `for=0` it is a blip: what
   is open ends, and the page that asks again is served. What is queued
   meanwhile is the page's when it subscribes again, from its cursor
   (ADR-0139).

Both are the session's own, so a suite that uses them shares its host.

## Alternatives

- **Abort in the browser, as before.** It cannot drop a connection after
  the server committed, which is the case that matters: the answer lost.
- **A server-wide outage.** Every engine's tests run against one host at
  once; one test's outage would be every test's.
- **Close the stream with an error frame.** A frame is an answer. A cut-off
  connection is no answer, which is what a page meets when a network drops.

## Acceptance

Recorded by `just e14-connection-faults` in
`docs/evidence/E14/connection-faults.txt`:

- The development server's tests, over real connections:
  - a dropped command connection is answered by nothing: before, and
    nothing ran; after, and the line was committed. Sent again with the
    same interaction, each is answered, and the line is there once;
  - a forced reconnect, armed through its control, ends an open stream and
    an open long poll at once. A new subscription inside the window, a
    stream or a long poll, is closed unanswered, and one after it is sent
    what was queued. With no window, an open stream ends and the next is
    served at once.
- `e2e/connections.spec.mjs`, in Chromium, Firefox and WebKit:
  - a command whose connection is dropped before it runs is sent again, and
    is one line;
  - one dropped after it commits is sent again, and is one line;
  - a page cut off from its subscription hears, once it is back, what
    changed meanwhile.
- `scripts/connection_faults_mutations.py`: 10 mutants. Their first run, at
  `40d0b85`, killed 8. The server's test had refused only a long poll inside
  the window, which its own loop ends anyway; a stream is sent its head
  first. And the browser's test of the control passed by chance: a page's
  held stream ends within two seconds of itself, mostly inside the window.
  The server's test now refuses a stream, and arms the control while a
  stream is held, with a window and without one.

## Not claimed

- **A connection dropped mid-answer.** A drop here comes before any byte of
  the answer; one cut after the head and part of the body is untested.
- **The materializer failure**, §15.5's last control, is the next ruling's.
