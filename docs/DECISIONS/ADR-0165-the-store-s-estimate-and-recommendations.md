# ADR-0165: the store's delivery estimate and recommendations

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The audit's third gap: charter §15.3's "delivery estimate
slot" and "recommendation slot", and §15.6 tests 3 and 17, in the canonical
store.

## Context

- **The store had neither slot.** T05 and T10 add them to the benchmark's
  copy of the store, as tasks. The development server already answers
  `store:data/recommendations#for-store` and `store:data/estimates#current`,
  with §15.5's delays of 1200 ms and 400 ms, and the page plan, the server
  and the runtime stream a region since ADR-0148.
- **The benchmark's store is its own copy since ADR-0156**, so the canonical
  store grows without re-basing T05 or T10.
- **§15.2 sets each one's policy.**
  - Recommendations are "public or experiment-partitioned", "streamed,
    bounded cache".
  - The delivery estimate is "Session/User", "private edge/origin result".
- **The accepted corpus already writes the recommendations' form** (A-008):
  `freshness 10.minutes`, `cache shared`, `invalidates_on MenuChanged(id)`,
  `fallback empty`, `delivery streamed`, `timeout 3.seconds`.

## Decision

1. **The store reads `Recommendations(id)`** in A-008's form, but for
   `fallback`, which a stream's query does not run (ADR-0148):
   - public, so shared;
   - kept ten minutes, which is the "bounded cache";
   - dropped when the store's menu changes, since what it suggests is the
     menu's (ADR-0164's path);
   - streamed, with a three-second budget.
2. **The store reads `Estimate(current_session())`** in T10's form: private,
   kept for no time, streamed, with a three-second budget.
3. **The page shows each as a `<stream>`**, in a named region.
   - The estimate comes under the store's name, where a person looks for
     it, and its region is `aria-live="polite"`. A screen reader is told the
     estimate when it comes (WCAG 2.2's status messages, 4.1.3).
   - The recommendations come after the cart.
   - Each has a placeholder and a failure of its own.
4. **An event reaches a stream's kept answer**, as it reaches a `let`'s. The
   development server found a query's policy only among a page's `let`
   bindings, so `MenuChanged(47)` could not drop the kept recommendations.
5. **The development server's recommender suggests from each store's menu**
   as it is now: the items after the first, two of them. Store 48's are its
   own, and a renamed item is suggested by its new name once the change has
   dropped the kept answer. A test that names the items still names them for
   every store.

## Found on the way

- **A `//` line inside markup is text.** The first version of this change
  explained the slots in a comment between two elements, and the page showed
  the comment. JSX does the same, and `eslint-plugin-react` has a rule for it
  (`jsx-no-comment-textnodes`). Nothing in Pleris refuses it yet; the next
  ruling after the §15.1 fields.
- **WebKit does not paint the store until its slots are filled.** WebKit
  paints a page once it holds about 200 characters of text, or has loaded
  (ADR-0148's measurement). The store holds about 90. Measured on this host
  in WebKit 26:
  - the runtime was ready 47 ms after the page was asked for;
  - its first contentful paint came at 2527 ms, when the response ended.

  So a Safari user sees nothing until the recommendations come, and cannot
  press Add before then: tests 3 and 17 hold in Chromium and Firefox, and
  not in WebKit. The store holds so little text because it says nothing
  about itself or its items: §15.1's `Store.description`,
  `MenuItem.description` and `price` are missing (the audit's eighth gap).
  That is the next ruling. `e2e/slots.spec.mjs` records both WebKit gaps as
  `fixme` tests, which the next ruling must turn on.
- **Playwright waits on animation frames**, which WebKit does not run before
  its first paint. A page's readiness polled on frames looked late in WebKit
  for that reason alone. The slots spec polls.
- **Three browser tests counted the whole page's markers.**
  - Two compared every keyed instance's token across two sessions, to show
    the menu's fragment is shared. A stream's region is rendered for its
    document (ADR-0148), so its tokens are the document's; they read the
    menu's now.
  - One counted every identity marker on the page; it counts the slots'
    too, item by item.

## Alternatives

- **The estimate after the cart**, as T10 placed it. A delivery time is read
  with the store's name, before the menu. The position does not change when
  WebKit paints; measured both ways.
- **Recommendations kept for no time**, T05's setup. It is not the bounded
  cache §15.2 asks for. And each page would wait 1.2 seconds for a list that
  changes when the menu does, which the event now says.
- **A paragraph of fixed text on the store page**, as the streamed demo has,
  to get past WebKit's 200 characters. It would be wrong for every store but
  one. The store's own description is the right text, and it is the next
  ruling.

## Acceptance

- `compiler/pw-core/tests/stream_plan.rs`:
  - `the_store_streams_its_estimate_and_its_recommendations` (new);
  - the stream rules' tests, moved to the benchmark's store, which streams
    nothing.
- The development server's tests:
  - the store sends its own content and both slots pending at once, then
    the estimate, then the recommendations, in the same response (test 3);
  - an estimator that is down fills its slot with the failure, and the page
    is served;
  - each store's recommendations are its own, kept, and dropped by a change
    to that store's menu only.
- `e2e/slots.spec.mjs`, three engines:
  - test 3;
  - test 17;
  - a store's recommendations;
  - a failing estimate;
  - the store painted before its slots are filled.

  Test 17 and the paint test are `fixme` in WebKit.
- The browser suite.
- The committed artifacts regenerated: the WIT, the page plan, the
  contracts, the store graph.
- `scripts/slots_mutations.py`: 7 mutants, recorded by `just e14-slots`.

## Not claimed

- **The store painted early in WebKit** (above).
- **Adding a recommended item from its slot.** The slot names what it
  recommends. Every Add button is named "Add" (the audit's test 14), and
  more of them would make that worse.
- **An estimate per store and place.** The estimate is the session's, as
  T10's estimator answers it. §15.1's `DeliveryEstimate` has a range and a
  time it was made, which the domain's does not.
- **A region's change after it settles** (ADR-0148). A change to the menu
  reaches the recommendations of the next page, not of a page already
  showing them.
