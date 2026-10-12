# ADR-XXXX: a store is open by its hours, read by the clock

Status: proposed by track `store-hours` (W9, `track/store-hours`), under the
integrator's rulings of 2026-10-11 (docs/PARALLEL.md, "Track
`store-hours`") and the coordinator's of 2026-10-12. The track is based on
`track/store-search`'s head, since a store's status is shown on the home
page and in search results. Date: 2026-10-12. Milestone: E14 (NEXT, the
DoorDash customer app's item 4, step 2). Builds on ADR-0304 (the clock),
ADR-0298 (the store on PostgreSQL), ADR-0318 and ADR-0319 (a user's cart
and order), ADR-0302 (a refusal told where the press was) and ADR-0308 (the
search).

## Context

- **The store had no hours.** `StoreRow` held an `opens_minute` and a
  `closes_minute` nothing read, and store 47's notice said "Open until 7
  pm" as text. An order could be placed at any hour.
- **ADR-0304 gave the platform a clock that a page reads by comparison.**
  A kept value holds until the earliest instant it compared. A page that
  shows it is told then. `clock.at` reads a zone's local time as RFC 5545
  does.

## Research

- **DoorDash's Marketplace store API** (developer.doordash.com, "Store
  Management" reference and "Get Store and Menu Information", read
  2026-10-12, through search excerpts):
  - Hours are "set based on the timezone of the store".
  - `open_hours` is keyed by `day_index`. An end earlier than the start
    runs into the next day. A day the store does not open is left out.
  - DoorDash "deducts 20 minutes from `open_hours.end_time` to determine
    the true ordering hours".
  - `special_hours` close, shorten or extend a date (`closed = true` for a
    holiday).
  - Full-day hours are written 00:00:00 to 23:59:59.
- **schema.org `OpeningHoursSpecification`**:
  - `dayOfWeek`, `opens` and `closes`.
  - A `closes` earlier than `opens` spans into the next day.
  - `validFrom` and `validThrough` mark a special date.
- **RFC 5545 §3.3.5**: a local time a transition skips is read with the
  offset before the gap, and a repeated one is its first occurrence. This
  is what `pw-time`'s `Zone::instant`, and so `clock.at`, does.

## Decision

1. **The hours are the store's data, on both layers** (`hours.rs`):
   - its IANA zone;
   - a cut-off in minutes before a closing, 20 at first, DoorDash's;
   - weekly periods (an ISO weekday, an opening and a closing minute, 0 to
     1440). A closing not after the opening is the next day's. The same
     minute means the whole day round.
   - special dates (days since 1970-01-01 in the store's zone), closed or
     open other hours, each in place of its weekday's periods.

   In memory they live in `State`'s `hours`. On PostgreSQL they are
   `stores.time_zone`, `stores.last_order_minutes`, `store_hours` and
   `store_special_hours`, from migration `0012_store_hours.sql`, numbered
   after part 1's 0011.
   - **The first hours are open every day, the whole day round**, so that
     no other track's test depends on the hour it runs. The hours' own tests
     set theirs.
   - The host's own write is `store:host/hours#set`, refused for a weekday
     past 1 to 7, a minute past 0 to 1440, a cut-off a day long or a zone
     the host's database lacks. A test reaches it at `/bench/hours`, with a
     `StoreChanged` event.
   - **A set is held as PostgreSQL returns it** (the integrator's ruling of
     2026-10-12): weekly periods by weekday and opening, special dates by
     date, and the last of a period or a date listed twice kept, as an
     upsert keeps it (`hours::held` in memory; `ON CONFLICT ... DO UPDATE`
     on PostgreSQL). The read-back is then one value on both layers.
   - **The order is the layers' contract**: a store's hours, as read, are
     one value with its order, said in `OpeningHours`' comment and on
     `Rows::hours`. PostgreSQL upholds it by its `ORDER BY`, memory by
     holding a set so. The program relies on it and sorts nothing. A sort
     in the program would be code no test can observe, an equivalent
     mutant.
2. **Whether a store is open is the program's** (`examples/store/hours.pw`):
   - From yesterday to a week ahead, each day's periods (its special
     date's, or its weekday's) become spans by day number and minute.
     Spans that meet are joined, so a store open every day round is one
     span.
   - **A day's periods are their union; the layers hand them ordered by
     opening, whatever order a set lists them in** (the integrator's
     rulings of 2026-10-12). A span that starts at or before the last one's
     end joins it, keeping the later end, compared as (day, minute). That covers a past-midnight period overlapping the next day's
     first. Until then, dinner listed before lunch made the next opening
     dinner's (in memory only, since PostgreSQL read them ordered), and
     periods that overlapped, not meeting exactly, stayed apart, so an
     order was refused at the first period's last order while the second
     kept the store open.
   - Each span becomes instants with `clock.at` in the store's zone. Its
     last order is the cut-off before its end, a day back where the end is
     just past midnight.
   - `status_of` compares the clock with them through `clock.passed`. The
     result is `Open(Opening { until, last_order })` for the span that has
     opened and not closed. Otherwise it is `Closed(Some(opens))` for the
     next one, or `Closed(None)`.
   - So a kept value holds until its next opening or closing, and a page
     that shows it is told then (ADR-0304).
   - The type is `OpenState`. The ruling's `StoreStatus` names the query
     that answers it, so the two names stay distinct.
3. **What the pages show**, each "Open 24 hours", "Open until 7:00 pm",
   "Closed · opens tomorrow at 7:00 am" or "Closed":
   - the store's page (`StoreStatus(id)`, public, shared, dropped by
     `StoreChanged(id)`);
   - the home page (`StoresOpen()`, every store with its status, which
     the home page now binds in place of `StoreList()`);
   - search results (the search reads `hours#for-stores` for the stores
     found);
   - the cart (`user query CartHours(reader)`, the sentence of a cart's
     store that is closed or past its last order, dropped by
     `UserCartChanged` and `StoreChanged`).
4. **An order is refused while closed, and past the last order** (its form
   the integrator's, 2026-10-12).
   - `place_order` reads the hours of its cart's stores in its own
     transaction (`hours#for-cart`). Before `UserOrders.place` it answers
     the declared `OrderError.Closed(Closing)`:
     - `type Closing = | ClosedNow(Option<Opening>) | PastLastOrder(LocalTime)`;
     - `type Opening = Opening { day: OpeningDay, at: LocalTime }`;
     - `type OpeningDay = | Today | Tomorrow | On(Weekday)`.

     The command fills them from the store's hours, in its zone, with its
     clock. The last order is the time it was, in the store's zone.
   - **Errors stay data; the view says them.** The browser has no zone
     database, so it cannot word an instant. It can word these.
     `closing_said` and `opening_said` (`hours.pw`), pure, are the ONE
     wording, shared by:
     - the page's "Closed · opens tomorrow at 7:00 am";
     - the cart's "Harbor Coffee is closed: it opens tomorrow at 7:00 am.";
     - the cart handler's "A store in your cart is closed: it opens
       tomorrow at 7:00 am.", told where the press was (ADR-0302).
   - Adding to a cart while closed is allowed: that is browsing.
5. **The clock module gains four pure functions**: `day_number` and
   `numbered_day` (days since 1970-01-01, ISO 8601's proleptic Gregorian
   calendar), `midnight` and `minute_of_day`.
   - A date's number is how a special date is matched and how "tomorrow"
     is said.
   - `minute_of_day` is how a page words a `LocalTime` a command answered.
   - The integrator accepted the first three, 2026-10-12, as Java's
     `LocalDate.toEpochDay` and `ofEpochDay` are. The fourth came with
     ruling (3).
   - The integrator's file, and a change to the trusted platform
     contract's hash.

## Found

- **`a | b & c` is `(a | b) & c`.** The language's `|` and `&` share one
  precedence, left to right. "Open until" first compared `to_day == today |
  to_day == today + 1 & to == 0`, and said "until today at 7:00 pm". The
  test caught it, and the condition is parenthesized now. The integrator
  rules it refused (its answers, below).

## Alternatives

- **The status computed by the data layer, from the host's clock.** A
  layer's operation reads no `clock.passed`, so nothing would note the
  instant, and no page would be told at the closing.
- **The hours in `StoreRow`.** They are a store's, but a store page, a
  list and a cart each read them alone, and the store's own reads stay
  unchanged.
- **A store seeded with real hours (07:00 to 19:00).** Every test that
  places an order at night, on CI and here, would be refused.

## Acceptance

- `tests/store_hours.rs`, in memory and on PostgreSQL (13 tests):
  - the store's page told at its closing instant: held until 19:00,
    nothing due at 18:59:59, "Closed" sent past it;
  - an order refused past the last order (18:41) and while closed (20:00),
    and taken before it (18:39), the cart saying why;
  - a cut-off across midnight (open to 00:10, refused at 23:55), and a
    store open every day round taking an order at 23:55;
  - hours past midnight (22:00 to 02:00);
  - a special date closed;
  - a day the clocks go forward (2027-03-14): a closing at 03:00 PDT half
    an hour after 01:30 PST, and an opening at 02:30, a time the day skips,
    read as 03:30 PDT;
  - the home page and a search showing each store's hours. The clock
    stands still between the two reads, so only the change's drop rereads
    the home page's stores. A local run on 2026-10-12 found the mutant
    "kept past a change to their hours" surviving: the clock moved days
    between the reads, past the stores' 30 seconds;
  - hours a store cannot have refused;
  - the hours read back alike on the layer, the seed's and a set's;
  - since the integrator's ruling of 2026-10-12, each with its control:
    - dinner listed and written before lunch: at 09:00 "opens today at
      11:00 am", and at 15:00 "at 5:00 pm";
    - 11:00 to 15:00 overlapping 14:00 to 22:00: at 14:30 "Open until
      10:00 pm" and an order at 14:45 taken; at 22:30 one refused;
    - Monday 18:00 to 02:00 overlapping Tuesday 01:00 to 10:00: one span,
      "Open until 10:00 am" at Tuesday 01:30, and closed at 10:30;
    - a set listed out of order, a period and a date twice, read back
      ordered with the last of each kept.
- `e2e/store-search.spec.mjs`, three engines: a store's page told when the
  store closes while it is open, and the home page after.
- `scripts/store_hours_mutations.py`, 17 mutants. Since the rulings of
  2026-10-12 they include the overlap merge back to exact-meet, memory's
  ordering of a set taken away, and PostgreSQL's `ORDER BY` taken away.

## Not claimed

- More than one week ahead: a store that next opens later says "Closed".
- Item-level hours (DoorDash's `item_special_hours`).
- A special date that only extends the day: it replaces the day's hours.
- The minutes shown are the period's as written. On a day a transition
  skips them, the instant is RFC 5545's (02:30 read as 03:30), and the
  page says "2:30 am".
- Store 47's notice still says "Open until 7 pm"; it is the store's text.

## The integrator's answers, 2026-10-12

1. **`|` and `&`**: neither binds tighter. A chain mixing them without
   parentheses will be refused at parse, naming both readings ("write
   `(a | b) & c` or `a | (b & c)`"). A chain of one operator stays
   left-associative, and comparisons still bind tighter. The sources are
   Carbon's logical-operators design and GCC's `-Wparentheses`.
   - Not "& binds tighter": that would silently change any program written
     for the old parse, where a refusal changes nothing and names the fix.
   - The integrator's parser track does this. Store-hours writes every such
     predicate with explicit parentheses, so it reads the same either way.
2. **The clock's functions**: `day_number`, `numbered_day` and `midnight`
   are accepted, their comments saying "days since 1970-01-01, ISO 8601's
   proleptic Gregorian calendar" (decision 5).
3. **The order's refusal is structured, not a sentence**: `Closing`,
   `Opening` and `OpeningDay` (decision 4), worded by the view.

## Merge notes

- Migration `0012_store_hours.sql` comes after part 1's 0011.
- Regenerated: the store's IR, graph, WIT, contracts, `e10-component`, and
  the handlers (`place_order` and the cart's handler).
- Moved: `build.rs`, `data_sources.rs` and `one_checker.rs`' counts and
  lists; the platform contract's hash; `store-values.json`.
- `store_search_mutations.py`'s "a search is kept past a menu's change" is
  re-anchored on the search's `invalidates_on`, which gains `StoreChanged(_)`.
- KNOWN_LIMITATIONS: a week's horizon; the shown minute on a skipped day;
  and (the integrator's N+1 ruling of 2026-10-12) "the hours' reads run a
  statement per store on PostgreSQL (hours#for-stores, hours#for-cart)",
  until the track that bounds every read.
- `hours.rs`' PostgreSQL reads and writes run through part 1's `Counted`
  connection, so every statement they run is counted.

## Report

- **Built**: `examples/store/hours.pw`; `spikes/own-renderer/server/src/hours.rs`
  and its seams (`store.rs`, `store_pg.rs`, `main.rs`, each marked `TRACK
  SEAM (store-hours)`); migration 0012; the pages' status (app.pw,
  search.pw); `place_order`'s refusal; `tests/store_hours.rs`; the spec's
  hours test; `just/store-hours.just`; `scripts/store_hours_mutations.py`.
- **Tests, local** (PostgreSQL 18.6): `tests/store_hours.rs` 9 of 9 in
  memory and on PostgreSQL; the server's suite in memory, 474 passed;
  `pw-core`, `pw-cli`, `pw-host` and `pw-materialize` green;
  store-search, accessibility, cart, stores and pages specs, 152 passed in
  three engines.
- **Mutants**: 14 planted; CI's verify is the record.
- **Verify**: on the push of `track/store-hours`.
