# ADR-XXXX: an order is placed for a time its reader chooses, and sent to its stores then

Status: proposed by W9 (track `scheduled-orders`), building the integrator's
design of 2026-10-14 (NEXT, the DoorDash customer app's item 4, its last
step), under the owner's delegation of 2026-10-02. Date: 2026-10-14.
Milestone: E14. Builds on ADR-0180 (a delivery estimate is a range),
ADR-0304 (the clock, read by comparison), ADR-0328 (a store's hours),
ADR-0329 (an order priced again in its transaction), ADR-0332 (a payment
is the provider's word), ADR-0334 (a command run when the clock passes its
time) and NEXT 4b (a read is a bounded number of statements, its ADR-XXXX).

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
   ahead whose start every store of the cart is open at, by ADR-0328's hours
   in each store's own zone.
   - The candidates are the quarter hours stepped along the clock from
     midnight today in the first store's zone, nine days of them, so a day
     the clocks change has its own number of windows, each its own instant.
   - Within reach is `!passed(start - 1 h) & passed(start - 168 h)`: compared
     with the clock (ADR-0304), so what shows them holds until the hour
     before the first window, and a page showing them is told then.
   - A store is open at a window where one of its spans (ADR-0328's, built
     from yesterday to eight days ahead) has opened by the window's start and
     not closed: open at its start, as the design says, whatever the
     store's last-order cut-off (which holds an order for as soon as
     possible).
   - Said in the first store's zone: "Today, 11:15 AM", "Tomorrow", a
     weekday, "Next Monday" a week on.
3. **`place_order` holds the time to them again, in its transaction**
   (`held_to`): for as soon as possible, ADR-0328's refusal while a store is
   closed or past its last order, as before; for a time, `NotOffered(when)`
   where it is no longer offered (its hour came, a store's hours changed), a
   store closed now taking one for a time it is open. The cart says
   "Thu 6:30 PM is no longer offered: choose another time." as soon as the
   hour comes, the checkout having compared it.
4. **Paid now, sent then.** The payment is begun and settled as ADR-0332's
   (or at once where the deployment takes none). Settled, an order for a
   time is `Scheduled(ScheduledFor)`, and its stores are told nothing:
   `schedule send_to_store(order) at s.sends`, where `sends` is the window's
   start less the longest of its stores' estimates at their most (ADR-0180)
   and five minutes more, DoorDash's lead. `send_to_store`, `invoked_by
   Clock`, moves the order to `Placed` as an order for as soon as possible is
   placed at once: its stores have it, and the reader's pages are told, once.
   - `ScheduledFor { at, sends, day, time }`: the design's `Scheduled(at)`
     with what both settling paths need to schedule the send, computed once
     in `place_order` from the estimate the reader saw, and the window's day
     and time in the first store's zone, which a page words with no zone
     database (ADR-0328's "errors stay data").
   - An estimate that cannot be made counts for nothing beyond the five
     minutes.
5. **The sweep still applies** (ADR-0334): an order for a time awaiting its
   payment is taken back thirty minutes on, and no send was scheduled for it.
6. **The order page says when**: "Scheduled for Thu 6:30 PM." until it is
   placed, then as before, its total charged.
7. **The clock's arithmetic** (`packages/pw-platform-web/clock.pw`):
   `later(at, by)`, `earlier(at, by)` and `is_before(a, b)`, pure, reading no
   clock. The clock itself is compared by `passed` alone.
8. **A second order waits for a scheduled one to be sent**:
   `OrderError.Scheduled`. Both layers hold one current order a reader (the
   in-memory map would replace the paid order; PostgreSQL's latest order
   would be the second, and the first never sent).
9. **The times are a page of their own**, `/cart/when{?day}` (ADR-0309's
   query parameter names the day shown): a store open the day round offers
   668 windows, a resumable button each, which the cart page, read again at
   every change of the cart, should not carry. The cart says what is chosen
   and links there; choosing goes back to the cart.
10. **The data**: the time chosen with the checkout's choices; an order's
    time (`order_times`, migration `0016_scheduled_orders.sql`; `times` in
    memory), one a reader, written with the order where it awaits its
    payment, read with its order's reference only, so one kept past its
    order is no order's and is replaced by the reader's next. New
    operations: `checkout#set-time`, `user-orders#send`;
    `user-orders#await-payment` takes the order's time; `user-orders#settle`
    makes an order for a time `scheduled`.

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
- **The store's hours say "7:00 am"; the design's example "Thu 6:30 PM"**.
  Built as the design says (`clock_time_said`); one wording is the
  integrator's to choose.

## Alternatives

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

## Acceptance

- The checker (`compiler/pw-core/tests/scheduled_orders.rs`, 2 tests):
  moving and ordering instants reads no clock (control: comparing it is
  PW0400); a run is scheduled at an instant moved along the clock
  (control: an `Int` is PW6503).
- The server (`tests/scheduled_orders.rs`, 23 tests; `scheduled_orders.rs`,
  1), in memory and on PostgreSQL: the seed's hours offer every quarter hour
  from an hour to a week ahead (668), the first gone a quarter hour on; a
  page of times held until the first window's hour; a store's hours offer
  the windows it is open at; hours past midnight; two stores in two zones,
  said in the first's; a time not offered refused when chosen (within the
  hour, between quarter hours, past a week, while closed) and when placed
  (its hour come, its hours changed); an order for a time taken while the
  store is closed; sent at its preparation and not a second before, told to
  the open page once; the estimate's most and five minutes; the longest of
  two stores'; a send before payment, or of another order, placing nothing;
  the sweep; a second order refused; the time kept with the choices; the
  page of times and the order page's words.
- NEXT 4b (`tests/bounded_statements.rs`), re-pinned on PostgreSQL, equal at
  the seed and with fifty stores more: the new operations, the page of times,
  `choose_time`, and an order for a time chosen, placed, paid and sent.
- The store in three engines (`e2e/scheduled-orders.spec.mjs`, hosts of
  its own): a time chosen, paid in the provider's frame, "Scheduled for ..",
  the clock moved past its preparation, "Placed"; and a time whose hour
  came, refused by name.
- `scripts/scheduled_orders_mutations.py`, 41 mutants.

## Not claimed

- **Cancelling an order once paid** (a refund at the provider, which no
  adapter has); before payment the reader's cancel and the sweep apply.
- **A store declining scheduled orders**; **a time per store** in a cart of
  several; **a window the store sets**.
- **A window held to its stores' ordering hours at its preparation**: a
  window just after an opening sends the order before the store opens, and
  one just before a closing may be sent past its last order (question 2).
- **The estimate at the moment of sending**: `sends` is fixed when the
  order is placed.
- **More than one current order a reader** (decision 8).

## Questions for the integrator

1. `clock.pw` gains `later`, `earlier` and `is_before` (decision 7); the
   trusted platform contract's hash moves. Accept, or named otherwise?
2. Should a window also need its stores taking orders at `sends`?
3. `Scheduled(ScheduledFor)` rather than `Scheduled(at)` (decision 4).
4. "Thu 6:30 PM" beside the hours' "7:00 am" (Found).

## Merge notes

- Migration `0016_scheduled_orders.sql`, after 0015.
- Moved: `tests/bounded_statements.rs`' counts (its commit says each);
  `store_hours_mutations.py`'s "an order is placed whatever the hours"
  re-anchored on `held_to` with a well-typed replacement, and
  `payments_mutations.py`'s "an order awaiting its payment does not stop
  another" on the order's status read once;
  `a_command_that_emits_nothing_tells_nothing`'s patch names
  `TimesOffered`, which reads the cart.
- Playwright: `SCHEDULED_ORDERS_PORTS`, PORT+183..185, their providers ten
  above.
