# ADR-0271: a reader is told once per burst, and served in turn

Status: accepted under the owner's delegation of 2026-10-02; found by
WebKit's "Load more" failures on CI. Date: 2026-10-08. Milestone: E14.

## Context

- **"Load more" failed in WebKit on CI six times** (runs 37663299969,
  37681083688, 37707617400, 37726447712, 37732376131, 37748154579): after
  the press, the page kept its twenty rows for five seconds. The runtime's
  record (run 37707617400) said the read was answered later, and only one
  stream was asked.
- **Reproduced here** under parallel load, 3 and 1 of 40 runs, with the
  runtime's record and the network's: the page received about seventy
  patch sets for the shared feed in a few seconds, each rendering its
  timeline at the twenty rows its document had, while its own read for
  forty went unanswered past the five seconds. Six workers post to one
  feed host, as CI's do. A second symptom had the same cause: the post the
  test held and released kept its speculated author, "You", past five
  seconds, its commit's frame waiting behind the others.
- **Two causes in the development server:**
  - **Each commit told every other reader** (ADR-0219): `tell_waiting`
    told each waiting commit in turn, and each read every other session's
    open documents again: a burst of N posts was N renders for every
    reader.
  - **The session's hold was taken in no order.** `std::sync::Mutex`
    promises none, and a session told again and again took its hold back
    before the session's own read, its keyed read (ADR-0152, ADR-0224),
    and its stream's first request, which waited seconds.

## Decision

1. **A reader is told once for every commit waiting**: `tell_waiting`
   merges what it takes into the set of sessions to tell, and tells each
   once, reading the latest.
2. **One telling of a session at a time**: a commit that reaches a session
   being told asks for one more after it and returns; the one more reads
   the latest. A telling that ends lets go of the session where it sees no
   commit came, in the same hold, so none is lost between the two; one
   that panics lets go of it too.
3. **A session's hold is taken in turn** (`Turns`): a ticket each, served
   in the order asked. A session's read waits for the change in progress,
   and comes before a telling asked after it. A turn that panics serves
   the next, where a mutex would be poisoned.

## Acceptance

- **The server's tests**: a burst of five posts gives another reader one
  patch set, showing each; turns are served in the order asked, and a turn
  that panics serves the next; and the server's whole suite.
- **The browser, before and after**: "Load more" in WebKit, six workers,
  forty runs a round, the server before and after in turn, four rounds
  each: 13 of 160 failed before (both symptoms), 2 of 160 after (Fisher's
  exact test, p = 0.006). The two after came in the first round; the last
  two rounds, at load averages of 10 to 36, passed all 80.
- **`scripts/telling_mutations.py`, 7 mutants**: the commits a connection
  takes told one by one; a commit reaching a reader being told dropped, or
  not told after; a telling that never lets go, or that panics and
  silences the reader; the session's hold taken in no order; and a turn
  that panics serving no one. Each run bounded, so a turn served to no one
  fails rather than waits. Recorded by `just e14-telling`.
- **`scripts/cross_session_mutations.py`**, run whole, one mutant
  re-anchored to `tell_waiting`'s set.

## Not claimed

- **A reader's own read is not put before a telling in progress**: it waits
  for that render, then is served.
- **Two of 160 still failed after**, under load past what CI's runners
  carry: the tests' five seconds bound a machine, not only the server.
- **Delivery across hosts**: one host's sessions, as before (ADR-0246).
