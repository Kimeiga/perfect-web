# ADR-XXXX: an order is placed for a time its reader chooses, and sent to its stores then

Status: proposed by W9 (track `scheduled-orders`), building the integrator's
design of 2026-10-14 (NEXT, the DoorDash customer app's item 4, its last
step), and as the integrator ruled on it on 2026-10-15 (Rulings, below), under
the owner's delegation of 2026-10-02. Date: 2026-10-15. Milestone: E14. Builds
on ADR-0180 (a delivery estimate is a range), ADR-0304 (the clock, read by
comparison), ADR-0328 (a store's hours), ADR-0329 (an order priced again in
its transaction), ADR-0332 (a payment is the provider's word), ADR-0334 (a
command run when the clock passes its time) and NEXT 4b (a read is a bounded
number of statements, ADR-0336).

## Context

- **The store took orders for as soon as possible alone.** ADR-0328 refused
  one while a store of the cart is closed; ADR-0334 gave the platform a
  command run at a time (`invoked_by Clock`, `schedule .. at ..`), its first
  use ADR-0332's sweep.
- **Practice** (the integrator's, read 2026-10-14): DoorDash's merchant help
  ("Manage Orders") keeps a scheduled order in a Scheduled tab until its
  preparation is due, and makes it active about five minutes before; Uber
  Eats ("Scheduled Orders") takes one up to about a week ahead and no later
  than an hour before, while the restaurant is closed, for a time it is open.
- **What a program could not say.** An `Instant` and a `Duration` are opaque
  outside the platform's `clock` module, which offered comparison with the
  clock alone (`passed`). "An hour before this window", "a week before",
  "the window less its preparation" and "this window inside that opening"
  each need an instant moved, or two ordered, without reading the clock.

## Decision

1. **The reader chooses when, at checkout**, as they choose a tip:
   `choose_time(when: Option<Instant>) -> Result<Option<Instant>, OrderError>`,
   none for as soon as possible, kept with the cart's other choices
   (`Chosen.when`; `checkout_choices.scheduled_for`), one time for the whole
   cart. A time not offered now is refused by name, `OrderError.NotOffered`,
   as `apply_promo` refuses a code: a page can send any instant.
2. **The times offered are the program's** (`examples/store/scheduled.pw`),
   never the page's: fifteen-minute windows from more than an hour to a week
   ahead, **offered only where every store of the cart would take an order for
   as soon as possible when its stores are told** (ruled by the integrator,
   2026-10-15: to a store a scheduled order is an order for as soon as possible
   placed then).
   - The candidates are the quarter hours stepped along the clock from
     midnight today in the first store's zone, nine days of them, so a day
     the clocks change has its own number of windows, each its own instant.
   - Within reach is `!passed(start - 1 h) & passed(start - 168 h)`: compared
     with the clock (ADR-0304), so what shows them holds until the hour
     before the first window, and a page showing them is told then.
   - **One rule for both paths**: `takes_order(spans, when)` in `store.hours`.
     `when` is none for the clock, where an order for as soon as possible is
     held to the hours now (`closing_now` decides with it, and words its
     refusal only where it says no), or an instant, where a window is held to
     them at `sends`. A store takes an order as of an instant where a span of
     its (ADR-0328's, in its own zone) has opened and not closed, and the
     span's last order is not past: its opening inclusive, its last order
     exclusive, as `clock.passed` (now at or after) has them.
   - **A window's `sends` is the window less its lead**: the longest of the
     cart's stores' estimates to the reader at their most (ADR-0180), and five
     minutes more, DoorDash's lead (`lead_of`; an estimate that cannot be made
     counts for nothing beyond the five). So a window just after an opening
     whose `sends` falls before it is not offered; one whose `sends` falls past
     a last order is not, though the store be open then; and one whose `sends`
     falls while the store takes orders is, though it be closed at the window
     itself (19:15 where the store closes at 19:00 and its last order is
     18:40: told at 18:35).
   - **The windows wait for the estimate, an ASAP cart does not**: `lead_for`
     reads it only where a time is chosen or the page of times is shown. A
     cart for as soon as possible asks none, so its statements and its latency
     are as they were.
   - A store's spans run from yesterday to eight days ahead (they were
     seven): a window a week ahead, on a day the clocks go forward, is on the
     eighth.
   - Said in the first store's zone, **in the words the store's hours say a
     time in** (`time_said`, ruling 5): "Today, 11:15 am", "Tomorrow", a
     weekday, "Next Monday" a week on; an order "Thu 6:30 pm".
3. **`place_order` holds the time to them again, in its transaction**
   (`held_to`): for as soon as possible, ADR-0328's refusal while a store is
   closed or past its last order, as before; for a time, `NotOffered(when)`
   where it is no longer offered (its hour came, a store's hours or the
   estimate changed), a store closed now taking one for a time it would take
   one. The cart says "Thu 6:30 pm is no longer offered: choose another
   time." as soon as the hour comes, the checkout having compared it.
4. **Paid now, sent then.** The payment is begun and settled as ADR-0332's
   (or at once where the deployment takes none). Settled, an order for a
   time is `Scheduled(ScheduledFor)`, and its stores are told nothing:
   `schedule send_to_store(order) at s.sends`. `send_to_store`, `invoked_by
   Clock`, moves the order to `Placed` as an order for as soon as possible is
   placed at once: its stores have it, and the reader's pages are told, once.
   - `ScheduledFor { at, sends, day, time }` (ruling 3): what both settling
     paths need to schedule the send, computed once in `place_order` from the
     estimate the reader saw, and the window's day and time in the first
     store's zone, which a page words with no zone database (ADR-0328's
     "errors stay data").
5. **The sweep still applies** (ADR-0334): an order for a time awaiting its
   payment is taken back thirty minutes on, and no send was scheduled for it.
6. **The order page says when**: "Scheduled for Thu 6:30 pm." until it is
   placed, then as before, its total charged.
7. **The clock's arithmetic** (`packages/pw-platform-web/clock.pw`, ruling 1):
   `later(at, by)`, `earlier(at, by)` and `is_before(a, b)`, pure, reading no
   clock. The clock itself is compared by `passed` alone.
8. **A second order waits for a scheduled one to be sent**
   (`OrderError.Scheduled`, ruling 4, the interim). Both layers hold one
   current order a reader (the in-memory map would replace the paid order;
   PostgreSQL's latest order would be the second, and the first never sent).
9. **The times are a page of their own**, `/cart/when{?day}` (ruling 6;
   ADR-0309's query parameter names the day shown): a store open the day
   round offers 668 windows, a resumable button each, which the cart page,
   read again at every change of the cart, should not carry. The cart says
   what is chosen and links there; choosing goes back to the cart. Each link
   and button stands in a paragraph of its own: WCAG 2.5.8 wants 24 pixels
   or room, which a list of one-line items does not give, and a view cannot
   hold a stylesheet (Found).
10. **The data**: the time chosen with the checkout's choices; an order's
    time (`order_times`, migration `0016_scheduled_orders.sql`; `times` in
    memory), one a reader, written with the order where it awaits its
    payment, read with its order's reference only, so one kept past its
    order is no order's and is replaced by the reader's next. New
    operations: `checkout#set-time`, `user-orders#send`;
    `user-orders#await-payment` takes the order's time; `user-orders#settle`
    makes an order for a time `scheduled`.
11. **The checkout and the page of times listen for the reader's addresses**
    (`AddressesChanged`): they read the estimates, and the checker refuses a
    query that reads the addresses and is not read again when they change
    (PW5106).

## Rulings

Each ruled by the integrator, 2026-10-15, on the questions this ADR was first
proposed with.

1. **`clock.pw`'s `later`, `earlier` and `is_before`**: accepted; the trusted
   platform contract's hash moves with a dated comment (decision 7).
2. **When a window is offered**: changed, to the rule of decision 2. A window
   is offered only where every store of the cart would take an order for as
   soon as possible at its `sends`, by the rule that holds one now (`held_to`
   and what it includes: opening, closing, any cut-off), not merely where the
   stores are open at its start. Edge tests, and a mutant for each, hold it.
3. **`OrderStatus.Scheduled(ScheduledFor { at, sends, day, time })`**:
   accepted (decision 4).
4. **A second order refused while one is scheduled**: accepted as the
   interim; it goes to KNOWN_LIMITATIONS at the merge (Not claimed).
5. **Wording**: one wording and one function, the store hours' words for a
   time of day ("7:00 pm", ADR-0328): "Thu 6:30 pm".
6. **The times on a page of their own**: accepted (decision 9).

## Found

- **A view cannot compute a function of a match arm's binding**: "part 3
  computes a value from `s`, which is no query's value". A function of the
  page's query value is computable (`charge_of(order)`), so the order page
  words the time as `when_scheduled(order)`.
- **A webhook's commit is told to other sessions on its own connection**,
  after it is answered: a test that then calls `run_due()` raced that drain
  of the sessions to tell, and once under load counted the reader's page
  told twice or not at all. The test waits for the page to be told the order
  is scheduled before it moves the clock, as the payments' own test waits.
- **The language has no function-typed parameter** (none appears in the
  corpus), so a rule shared by two ways of asking takes an "as of": the
  `Option<Instant>` of `takes_order`, none for the clock.
- **A view cannot hold a stylesheet**: a `<style>` element's text is a
  template, so its braces are interpolations (PW0001), and PW5023 refuses a
  value inside one. The e2e's own WCAG 2.5.8 audit (`violations`) failed the
  page of times in all three engines, links 17 to 19 pixels tall and buttons
  21 to 23 a line apart; the markup spaces them (decision 9).
- **The windows depend on the estimate, which is slow and can fail.** The
  development server's estimator waits 400 ms a store (charter 15.5) and
  `Estimate` is streamed with a 3 s timeout; `TimesOffered` and `Checkout`
  are not, and have 2 s. Taken the conservative way: the estimate is read
  only where a time is chosen or the page of times is shown, never for a cart
  for as soon as possible (`a_cart_for_as_soon_as_possible_asks_no_estimate`,
  and `/cart`'s 16 statements did not move), and a failed call makes the
  page of times and the checkout of a reader with a time chosen unavailable,
  a refusal (a declared error) counting for nothing. Cheap to reverse; the
  follow-ups are `delivery streamed` for the page of times, and the cart's
  `when_said` split out of `Checkout`.
- **A window after a closing is offered**: told at 18:35, the store takes the
  order, which it prepares and sends past its closing. It is the rule as ruled;
  the last order is the store's own bound.
- **The count tests of the first artifact commit were incomplete**: the
  workspace run's whole list of failures (`data_sources`, `each_typing`,
  `handlers`, `one_checker`, the E10 handlers) is the checklist, not the three
  its message named.

## Alternatives

- **Windows by their start's hours** (the first design): an order sent to a
  store before it opens, or past its last order. Ruled out (ruling 2).
- **Windows from the local times of each day** (`clock.at(day, 18:30)`): a
  day the clocks go back repeats an hour no local time names twice, and one
  they go forward names an hour that is not there.
- **`sends` computed where the payment settles**: the provider's word runs
  on the reader's behalf in a session of its own, whose estimate is not the
  one the reader was shown, and an estimator's delay or failure would hold
  the provider's word.
- **The send scheduled where the order is placed**, before it is paid: an
  order paid after its preparation was due would then need a second path.
- **All the windows on the cart page**: see decision 9.
- **The status `Scheduled(at)` alone**: each settling path would read the
  order's time again, and the page could not word it.
- **Two copies of the hours rule**, one for the clock and one for a window:
  they would drift. One function and an "as of" (decision 2), and a test that
  asks the cart's own words at the instant each window of a day would be told.

## Acceptance

- The checker (`compiler/pw-core/tests/scheduled_orders.rs`, 2 tests):
  moving and ordering instants reads no clock (control: comparing it is
  PW0400); a run is scheduled at an instant moved along the clock
  (control: an `Int` is PW6503).
- The server (`tests/scheduled_orders.rs`, 25 tests; `scheduled_orders.rs`,
  1), in memory and on PostgreSQL: the seed's hours offer every quarter hour
  from an hour to a week ahead (668), the first gone a quarter hour on; a
  page of times held until the first window's hour; a store's hours offer the
  windows its stores are told at while it takes orders (07:45 to 19:15 for a
  store open 07:00 to 19:00, and one after it closes); hours past midnight;
  two stores in two zones, said in the first's; **the integrator's edges**:
  a window whose `sends` falls on an opening is offered and one before it is
  not, one on a last order is not and one before it is, and the cart says at
  each of those instants what it says of an order for as soon as possible;
  **the rule probed at every quarter hour** of a day of two periods with a
  cut-off between quarter hours (a window is offered exactly where the cart
  says an order for as soon as possible is taken when its stores are told);
  the estimate moving the windows, and a failed estimate counting nothing;
  an ASAP cart asking no estimate, with a control; a time not offered refused
  when chosen (within the hour, between quarter hours, past a week, before
  its stores can be told, past a last order) and when placed (its hour come,
  its hours changed, the estimate grown); an order for a time taken while the
  store is closed; sent at its preparation and not a second before, told to
  the open page once; the estimate's most and five minutes; the longest of
  two stores'; a send before payment, or of another order, placing nothing;
  the sweep; a second order refused; the time kept with the choices; the
  page of times and the order page's words.
- NEXT 4b (`tests/bounded_statements.rs`), re-pinned on PostgreSQL, equal at
  the seed and with fifty stores more: the new operations, the page of times
  (14, the estimate from each of the cart's two stores in), `choose_time`
  (13), and an order for a time chosen, placed, paid and sent.
- The store in three engines (`e2e/scheduled-orders.spec.mjs`, hosts of
  its own): a time chosen on the page of times (which passes the WCAG audit),
  paid in the provider's frame, "Scheduled for ..", the clock moved past its
  preparation, "Placed"; and a time whose hour came, refused by name. With
  the cart, accessibility, payments, order-total, store-accounts,
  stable-layout, slots, store, stores, store-search, pages and layouts specs:
  301 passed, 5 skipped (their own engine skips), none failed.
- `scripts/scheduled_orders_mutations.py`, 52 mutants (the results are
  recorded by `just e14-scheduled-orders`); `store_hours_mutations.py`'s
  anchors follow the shared rule; the checker refuses the two that drop
  `AddressesChanged` (PW5106, `CHECKED`).

## Not claimed

- **Cancelling an order once paid** (a refund at the provider, which no
  adapter has); before payment the reader's cancel and the sweep apply.
- **A store declining scheduled orders**; **a time per store** in a cart of
  several; **a window the store sets**.
- **The hours when the order is sent**: an order is held to them where it is
  chosen and where it is placed; `send_to_store` does not hold it again, so a
  store whose hours changed since is told as the order was taken.
- **A window nearer than its lead**: an estimate over about 55 minutes puts a
  window just an hour ahead before now, and its send is due at once.
- **The estimate at the moment of sending**: `sends` is fixed when the order
  is placed.
- **More than one current order a reader** (decision 8, the interim of ruling
  4): a second order is refused while one is scheduled. It goes to
  KNOWN_LIMITATIONS at the merge.
- **A page of times that survives its estimator**: the estimator's delay and
  failure reach the page and a time-chosen checkout (Found).

## Merge notes

- Migration `0016_scheduled_orders.sql`, after 0015.
- Moved: `tests/bounded_statements.rs`' counts (its commit says each); the
  count tests of the store's artifacts (`build`, `data_sources`,
  `each_typing`, `handlers`, `one_checker`); `store_hours_mutations.py`'s
  "an order is placed whatever the hours" re-anchored on `held_to` with a
  well-typed replacement, and its two mutants on the hours' rule re-anchored
  on `takes_order` and `open_span`; `payments_mutations.py`'s "an order
  awaiting its payment does not stop another" on the order's status read
  once; `a_command_that_emits_nothing_tells_nothing`'s patch names
  `TimesOffered`, which reads the cart.
- `store.hours`: `status_of`, `named_span` and `closing_now` find the open
  span with `open_span`, and a store's spans run ten days (`day_spans`).
- Playwright: `SCHEDULED_ORDERS_PORTS`, PORT+183..185, their providers ten
  above.
