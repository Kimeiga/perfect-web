# ADR-0174: a store delay, a cart delay, and a one-shot database error

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's sixth gap (charter §15.5): "deterministic test
controls", of which a store delay, a cart delay and a one-shot database
error were missing or partial.

## Context

Measured on the development server on 2026-10-04:

- **No store delay and no cart delay.** §15.5 asks for each,
  configurable. The recommendations' and the estimate's delays exist
  (`/bench/recommendations`, `/bench/estimate`).
- **The one-shot database error was an add of the server's own.**
  `/command/add_and_fail` ran `add_to_cart` with the data layer told to
  answer `cart-expired`. No page's press met it, and no read could.
- **The one test that used it proved nothing.**
  `e2e/resource-path.spec.mjs`'s "a rolled-back command produces no browser
  update" posted it through Playwright's `request` fixture, which does not
  share the page's cookies. Its add was another session's, whose changes
  the page could never hear. Since ADR-0172 it also failed on its
  arguments, a string where `add_to_cart` takes the item. The test passed
  either way.
- **Every document read its cart twice.** Before serving a document, the
  session's drain read the cart to bring its materialized entry up to date,
  even when its entry existed and nothing had changed. That read changed
  nothing. It was also the read a one-shot read error would meet, where no
  one sees it.

## Decision

1. **`POST /bench/store?delay=ms`: the store's delay, one for every
   reader.** The store is one value for all of them, kept for its
   freshness, so the delay is the data layer's: each read that reaches it
   waits. What any page kept of a store is dropped, so the next reader
   waits. `delay=0` clears it.
2. **`POST /bench/cart?delay=ms`: the session's cart delay.** Each read of
   this session's cart waits; another session's does not.
3. **`POST /bench/fail?next=write` or `next=read`: a one-shot database
   error.** The session's next cart write, or next cart read, fails as a
   database that is down does: the operation answers no value.
   - A command traps and commits nothing. Its press fails visibly, and the
     line its speculation showed goes (ADR-0122).
   - A page cannot be shown, and is answered 503, as one whose query failed
     is (ADR-0147).

   Once: the next write or read succeeds.
4. **`/command/add_and_fail` is retired.** The resource-path test arms the
   page's own session, and the page's own press meets the failure.
5. **A drain reads nothing when nothing changed.** It reads the cart only
   to make the session's entry, or to regenerate it after an invalidation.

## Alternatives

- **A declared refusal for the database error** (`cart-expired`, as before).
  That is the command's own answer, not the database failing under it.
- **A store delay per session.** The store is kept for every reader: only
  the session whose read happened to fill the cache would wait.
- **A cart delay per server.** It would slow every visitor's page for one
  test's sake, and every engine's tests run against their own servers at
  once.

## Acceptance

Recorded by `just e14-test-controls` in `docs/evidence/E14/test-controls.txt`:

- The development server's tests:
  - a one-shot write error fails the session's next write and commits
    nothing; the next succeeds; another session's write is untouched;
  - a one-shot read error fails the session's next read, and the page
    cannot be shown, once;
  - a cart delay slows its own session's page and no other's; the store's
    delay slows the store's read when it is read, and not once cleared.
- `e2e/controls.spec.mjs`, in Chromium, Firefox and WebKit:
  - a write the database fails: the press fails, its line goes, and the
    next press is one line;
  - a read the database fails: the page is answered 503, once;
  - a cart delay slows its own session's page, and no other's;
  - the store's delay slows every page that reads the store, until it is
    cleared.
- `e2e/resource-path.spec.mjs`: a rolled-back command produces no browser
  update, now of the page's own press.
- `scripts/test_controls_mutations.py`: 10 mutants.

## Not claimed

- **The rest of §15.5.** The network error, the forced reconnect and the
  materializer failure are the next rulings'.
- **A page whose cart cannot be read is not shown at all**, as a page whose
  query failed is not (ADR-0147). Showing its public parts beside a failed
  cart is the last-known-good ruling's (test 18).
