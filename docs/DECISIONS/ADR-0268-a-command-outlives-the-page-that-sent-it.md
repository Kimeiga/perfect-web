# ADR-0268: a command outlives the page that sent it

Status: accepted under the owner's delegation of 2026-10-02; found by a
WebKit failure on CI. Date: 2026-10-08. Milestone: E14.

## Context

- **A press was lost on CI.** Verify run 37728182124, at 328c306, failed
  `pages.spec.mjs`'s "a press the server refuses is restored on the cart's
  page" in WebKit alone: the cart's page held no line, and its "Increase
  quantity of Espresso" was waited for 30 seconds. Its setup pressed Add,
  waited for the cart's count to read 1, and read `/cart`. Sixty runs here,
  under load, passed.
- **The count is the speculation's, shown before the request leaves.**
  `speculate` renders a command's transition (ADR-0122), and only then is
  the request made. A test that waits for the count and leaves the page
  races the request; WebKit under load cancelled a request in flight when
  its page was read again, which this file's previous test and ADR-0238's
  like wait for. The run's log does not record the request's fate; its
  trace, which CI kept, is not read here. Each document is its own
  subscriber from its render (ADR-0161), so a commit after the render
  reaches it: a press that reached the server would have shown.
- **A user meets the same race.** A press, and a link followed before its
  request has left, end the page and the request with it: the page showed
  the press taken, and nothing says it was not. The window is short where
  the connection is open, and long where it is not.
- **The Fetch standard** (read 2026-10-08): a request's `keepalive` "can be
  used to allow the request to outlive the environment settings object,
  e.g., `navigator.sendBeacon()` and the HTML `img` element use this".
  Bounded: "If the sum of contentLengthValue and inflightKeepaliveBytes is
  greater than 64 kibibytes, then return a network error", the bodies of a
  page's keepalive requests in flight. WebKit counts them so
  (`KeepaliveRequestTracker`).
- **Playwright intercepts such a request** in each of the three engines:
  every test that holds a command at the network holds it as before.

## Decision

1. **A command's request is kept alive** where its body fits what remains
   of 64 KiB of the page's kept-alive bodies in flight, counted in bytes,
   from its sending until its answer is read or it fails. A press and a
   link followed at once is a press: the server commits it, and the next
   page reads it, in its document or from its subscription.
2. **Past what remains, a request is sent as before.** One the browser
   would refuse is not made.
3. **`?keepalive=` lowers the budget for a test**, as `?transport=` pins an
   adapter, so the bound is reached with the store's few bytes. It never
   raises it, and one it cannot read is 0.
4. **Nothing else is kept alive**: the subscription and a page's reads are
   the page's own, and end with it.
5. **A test that presses and then leaves the page waits for the press's
   answer**, where leaving is not what it tests: `pages.spec.mjs`'s
   `oneEspresso`, the accessibility audit's order, `shared-output`'s
   return to the store, and the feed's follow and unfollow.

## Acceptance

- **`e2e/keepalive.spec.mjs`, 5 tests in each engine**: a command's request
  kept alive, counted by its body's bytes until its answer; past a budget
  of one body, the second press's request sent as before, and both made; a
  press and a link in one task, the press in the cart; `?keepalive=`
  lowering the budget and never raising it; a request that fails no longer
  counted.
- **`e2e/feed.spec.mjs`**: a post's request counted by its bytes, an emoji
  four of them and two of JavaScript's characters.
- **`scripts/keepalive_mutations.py`, 8 mutants**: the request not kept
  alive; the budget not read; the bytes in flight not counted, or not
  uncounted when a request is answered or fails; a body counted in
  characters; `?keepalive=` raising the budget, or an unreadable one
  keeping it whole. Recorded by `just e14-keepalive`.
- **The browser suite in the three engines.**

## Not claimed

- **A press whose handler's code is still loading** when the page is left
  is not sent: its request does not exist yet (a handler's code arrives on
  its first press, E7-L).
- **A page whose other scripts keep bodies alive** (`sendBeacon`) can find
  a command's request refused by the browser's bound, which counts theirs
  and this runtime counts its own. It fails as a network failure does, and
  is sent again as its `retry` clause says.
- **An answer whose page is gone is read by no one**: a refusal or a
  failure the server answers is not shown. The next page reads what is.
- **Why WebKit lost the press is inferred**, not read from the run's trace.
