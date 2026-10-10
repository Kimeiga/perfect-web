# ADR-XXXX: a delivery goes to the reader's chosen address

Status: proposed by track `store-accounts` (W8, `track/store-accounts`),
under the integrator's rulings of 2026-10-09 (docs/PARALLEL.md, "the
DoorDash track (W8's plan)", milestones 2 and 3) and its answers below;
numbered at the merge. Date: 2026-10-09. Milestone: E14, the DoorDash
store's customer side, milestones 2 and 3. Builds on "a cart and an order
are a user's" (this track's first ADR) and ADR-0298 (the store's data
layers).

## Context

- **A store's estimate was its kitchen's alone.** `session query
  Estimate(session)` answered how long the store takes, the same wherever
  the order was going, and any store took any order.
- **The rulings** (PARALLEL.md): a reader keeps delivery addresses (add,
  rename, remove, one chosen; a guest's chosen is its session's); each has
  coordinates from a fixed table, never a geocoding service; whether a store
  reaches one is a pure function of its zone, a radius; the estimate and the
  store's availability are keyed by the chosen address, and the estimate
  gains the courier's travel; a store out of reach says so where its menu is,
  and its Add is refused.
- **Milestone 3 is the owner's Next.js bug, as the acceptance**: an address
  saved, the handler's `Ok` arm `navigate StorePage(id)`, and the store's
  page shows the new address's estimate as served, with no parameter to bust
  a cache and no second load of the document, in three engines.

## Decision

### 1. The places are a fixed table, and a store's zone a radius

`places.rs` holds eleven places (Union Square, the Ferry Building,
Chinatown, Civic Center, Dolores Park, the Castro, Golden Gate Park, Ocean
Beach, Lake Merritt, Berkeley, Palo Alto), each with an id, a name and
coordinates in millionths of a degree. An address keeps a place's id and a label of the reader's (1 to
40 code points, `AddressLabel`). Nothing resolves a place the table does not
hold, and no request leaves the machine.

A store has a zone: where it is and a radius in metres. Store 47 is at Union
Square and reaches 4 km; store 48 at the Ferry Building, 2 km. In memory
(`store::zone_of`) and on PostgreSQL (migration `0004_addresses`, three
columns on `stores`), a test holding the two alike. Civic Center is within
store 47's radius and past store 48's: without a place between the two
radii, a layer that held store 48's radius as store 47's answered as the rule
does (Found).

### 2. One rule, read by the estimate, the page and the commands

`places::delivers_to(zone, place)`: the great-circle distance (haversine, the
Earth's mean radius) within the radius, the boundary included.
`places::travel_minutes`: four minutes a kilometre, rounded up, at least one.
`store::coverage` reads the reader's chosen address and the store's zone
through the layer's rows and calls both. Every reader of the rule goes
through it:

- the estimate, `store:data/user-estimates#for-store`: `no-coverage` past
  the radius, else the kitchen's minutes plus the travel;
- the page, `store:data/coverage#for-store` (`Coverage`: chosen, delivers,
  the address's label, the travel);
- `add_to_cart`, `store:data/coverage#reaches`, in its own transaction;
- `place_order`, in `user-orders#place`'s transaction, every line's item
  from a store that lists it and reaches the address.

`tests/addresses.rs`'s `the_page_the_estimate_and_the_commands_read_one_rule`
holds it: for each store and each of the eleven places, the page's `delivers`,
the command's answer and the estimate's refusal each equal `delivers_to`, on
both layers.

### 3. Out of reach is the program's declared error, not a predicate (Q-M2a)

`CartError.OutOfRange(StoreId)` and `OrderError.NoCoverage`, answered by the
commands and told in each handler's `Err` arm in the page's words ("This
store doesn't deliver to your address.", "A store in your cart doesn't
deliver to your address."), as "That item just sold out." is. The store's
page reads `Reach(id, current_user())` and, where the store does not reach
the address, says so where the menu is: "Harbor Coffee doesn't deliver to
Home." `place_order` refuses on its own, whatever the Add did: a cart can
outlive a change of address.

### 4. Addresses are the reader's

`examples/lib/Addresses.pw`: `Address`, `Place`, `Coverage`,
`AddressError` (`UnknownPlace`, `TooMany`, `NoSuchAddress`) and the library's
functions over `store:data/addresses`, `places` and `coverage`. The store's
`user query SavedAddresses(reader)`, `Places()` (public, an hour),
`Reach(id, reader)`, and the commands `add_address` (the new one chosen),
`rename_address`, `remove_address` (the chosen one removed leaves none) and
`choose_address`, each invalidating `SavedAddresses(current_user())` and
emitting `AddressesChanged(current_user())`, which `Estimate`, `Reach` and
`SavedAddresses` listen to by their reader. At most ten (`TooMany`). The rows
are keyed by the reader's opaque owner, as the cart's are: a user's id, or a
guest's `u-<session>`.

### 5. The estimate is the reader's, kept for no time (Q-M2b)

`user query Estimate(id, reader)`, keyed by both, `freshness 0.seconds`,
`cache private`, streamed as before. The estimator's test controls
(`/bench/estimate`, `Faults.estimators`) stay keyed by session, and the
travel goes on top. Kept for no time and private, each document's read
computes with its own session's fault, so a fault injected for one session
never reaches the user's other sessions through a user-keyed entry, and no
test depends on which session read first. The faults are test controls; the
travel is the model.

The host's streamed runs (`stream_runs`) took `current_session()` alone as a
key's argument; they now take `current_user()` too, as the page's own reads
do.

### 6. The address page, and milestone 3

`user page AddressPage(id)` at `/stores/{id}/address`, linked from the
store's page ("Change", `#change-address`): the reader's addresses, each
with Deliver here (`choose_address`, then `navigate StorePage(id)`), Rename
and Remove; and the places, each with Save and deliver here (`add_address`,
then `navigate StorePage(id)`). The label is checked by `address_label`
before the command: a label outside 1 to 40 is told on the page and sends
nothing.

The navigation is a document load today; the store's page is served with the
new address's estimate already in it, because `Estimate` is kept for no time
and keyed by the reader. The header's remount assertion waits for soft
navigation (track/layouts).

## Questions to the integrator, and its answers

All 2026-10-09, relayed through the coordinating session.

1. *Q-M2a: may the host evaluate `DeliversTo` at a `TRACK SEAM` in
   `identity.rs`'s holds, beside `OwnsPost` and `MayMessage`?* **No.** A
   `requires` predicate answers who may act, the principal's relation to a
   resource, and `policy.rs` keeps them deployment vocabulary so that no
   application code runs before authorization. Whether a store delivers to
   an address is the store's own rule about its own data, so it belongs in
   the program, as `ItemUnavailable` does: the command reads the address and
   the store in its own transaction and answers a declared error, told in
   the handler's `Err` arm. The estimate's `NoCoverage` and the command's
   refusal come from one function, held by a test (2). `place_order` refuses
   whatever `add_to_cart` does (3).
2. *Q-M2b: keep the estimator's test controls keyed by session, travel on
   top?* **Yes**, on the condition that the `Estimate` entry is never kept or
   shared across a user's sessions: freshness 0 and cache private (5).
3. *Places as a fixed table with stated coordinates, no geocoding?*
   **Accepted**; stated below as not claimed.

## Found

- **A `<ready as={x}>` binding is resolved as a module's function of the
  same name.** The store's page bound `as={estimate}` while `Addresses` held
  a function `estimate`, and the build refused (`PW0401`) a name the page
  had bound. A name a template binds (`<ready as>`, `<failed as>`, `{#each
  … as}`, a match arm's binders) should shadow module members and imports,
  as a `let` does. The library's function would have been
  `Addresses.estimate`; it is `estimate_to` here, which the fix will not
  change. The integrator's ruling: a resolver defect, the integrator's,
  queued on NEXT with a test for each binding form, after soft navigation.
- **The table first told the two stores' radii apart nowhere.** With ten
  places, none lay between 2 and 4 km of the Ferry Building, so the mutant
  "on PostgreSQL, store 48 delivers as far as store 47" survived the first
  recording. Civic Center is the place between the two radii, and the rule's
  test asserts that the table holds one.
- **`!reach.delivers` in a condition made a derived component** the
  committed component evidence did not list; the page reads
  `{#if reach.delivers}{:else}…{/if}`.
- **A wildcard key in `invalidates` (`Reach(_, current_user())`)** is
  undecided in the value relations; the commands invalidate
  `SavedAddresses(current_user())` and let `AddressesChanged` tell the rest.

## Alternatives

- **A host predicate for reach**: refused (Q-M2a).
- **Coordinates from a geocoding service**: a download and an account, and
  refused by the ruling.
- **Polygons for a zone**: a radius is what the ruling names, and a pure
  function of two points.
- **The estimate kept per user**: refused (Q-M2b).

## Consequences

- A store's estimate depends on where the order goes, and a store out of
  reach takes no order.
- The store's artifacts move again: contracts, components (`store.page.Reach`
  new), WIT, graph, plan, handlers (the address page's five), and the
  platform contract's hash (`domain.pw`'s two new cases).

## Acceptance

Recorded by `just e14-store-accounts` in
`docs/evidence/E14/store-accounts.txt`, against PostgreSQL 18.6:

- **`places.rs`'s own**: the distance is the great-circle one (Union Square
  to the Ferry Building about 1.5 km, to Palo Alto about 44); a store
  delivers within its radius, at its boundary, and not a metre short; four
  minutes a kilometre, rounded up, at least one; each place one, and in the
  Bay Area.
- **`tests/addresses.rs`, in memory and on PostgreSQL**: a reader's
  addresses are theirs to add, rename, remove and choose (another user's
  and a guest's apart; a place outside the table refused; a label past 40
  refused at the boundary); at most ten; one rule everywhere it is read; the
  estimate gains the travel to the chosen address; a store out of reach says
  so, refuses the Add, and refuses the order of a cart filled before the
  address changed; an address saved tells the user's other sessions and no
  one else's; the places are the table's.
- **The browser** (`e2e/store-accounts.spec.mjs`, Chromium, Firefox and
  WebKit, development accounts): milestone 3, an address saved goes back to
  the store, whose estimate is the new address's as served (Union Square to
  Dolores Park, 3.58 km: "Delivery in 40 to 50 min"), the URL without a
  search, and one load of the store's document; a store out of reach says so
  and refuses its Add; an address chosen goes back to the store, its
  estimate the chosen one's.
- **`pw-conformance`'s oracle**: `add_to_cart`'s reference reads the reach
  after the item's availability, and is run both ways.
- **`scripts/store_accounts_mutations.py`**: see the report.

## Not claimed

- **The places are a fixture, not a geocoder.** Eleven places, approximate
  coordinates, no address outside them.
- **A courier's minutes are a stated model**, four a kilometre in a straight
  line; no road, no traffic.
- **The header's remount assertion**: when soft navigation lands.
- **An address's label is not checked for meaning**, only its length.

## Report

### What was built

- `places.rs`; the store's zones and addresses in both layers (migration
  `0004_addresses`); the operations `user-estimates#for-store`,
  `coverage#for-store`, `coverage#reaches`, `addresses#list|add|rename|
  remove|choose` and `places#list`; `user-orders#place`'s refusal.
- `examples/lib/Addresses.pw`, `domain.pw`'s `OutOfRange` and `NoCoverage`,
  `Events.AddressesChanged`; the store's `Estimate`, `SavedAddresses`,
  `Places` and `Reach`, the four address commands, the store's page's
  address line and notice, and `AddressPage`.
- `stream_runs` with `current_user()`.
- Tests: `tests/addresses.rs`, `places.rs`'s, the browser spec's three
  milestone 2 and 3 tests, and the expectations of the tests that pin the
  store's text.

### Tests and mutants

- `just e14-store-accounts`, recorded at `ce4c300` against PostgreSQL 18.6:
  every test above is green on both layers, **29 of 29 mutants killed** (17
  from milestone 1, 12 from milestone 2), each by a failing test, and the
  memory bound stopped no process. The 12 are: the radius read in
  kilometres; the estimate without the travel, or answering for a store out
  of reach; an Add, or an order, to a store out of reach let through; a
  saved address not chosen; a choice that leaves the last one chosen; an
  eleventh address kept; a place outside the table saved; on PostgreSQL, a
  reader's addresses read whoever saved them, or store 48 delivering as far
  as store 47; and the reach not told when addresses change. The first
  recording left the PostgreSQL radius mutant alive (Found). The fix is
  `ce4c300`.
- The workspace's tests (311 result lines, none failed); the server's whole
  suite, 379 tests, in memory and on PostgreSQL; the browser suite in three
  engines at `PORT=7300` (901 passed) and the feed's spec (66); after the
  merge of master, at `PORT=7769`, 904 passed, the feed's included; the conformance
  oracle.
- `just fmt-check`, `lint`, `case-check`, `evidence-gates`,
  `mutation-anchors`, `test-compile` and `audit` green.
- **Re-anchored, not run whole here**: `descriptions`, `slots` and
  `estimate_range`, on the store page's new text and the estimate's new
  operation. Also every script anchored in a file this milestone changed
  (`store.rs`, `store_pg.rs`, `main.rs`, `app.pw`). CI's verification plans
  those recipes and runs them whole (ADR-0281).

### Merge notes

- `origin/master` merged at `7a701c2` (build-id, ADR-0300). One conflict,
  in `main.rs`: `Server`'s fields, where both `cart_by_reader` and
  `build_id` are kept. ADR-0300's new recovery test now expects the store
  page's documents to be its user's (`3b1a516`). The recording is from
  `ce4c300`, before that merge; CI's verification records it again.
- **Shared files**: `main.rs` (`mod places`, `TRACK SEAM (store-accounts)`;
  `stream_runs`), `spikes/own-renderer/store-values.json` (the address
  page's signals and the document's `id`, which the static render needs),
  four browser specs that count the store page's anchors, tab stops and
  tree, or found the Add's part by its owner, `store.rs`, `store_pg.rs`, `examples/domain.pw`,
  `examples/store/app.pw`, `examples/lib/Events.pw`, `StoreData.pw`, the
  compiler's and the host's tests that pin the store's text, the
  conformance oracle, `playwright.config.mjs` (`STORE_ACCOUNTS_PORTS`).
- **Ports**: `PORT=7769`, inside the track's range (7741 to 7940). The
  hosts reach `PORT+162`, and 7769 is the first PORT in range whose hosts
  miss 7768, which the owner's Spotify holds. The runs before the merge were
  at 7300, outside the range.
- **Two ADRs, both `ADR-XXXX`**: this one and "a cart and an order are a
  user's". Where milestone 2's files name `ADR-XXXX` (`places.rs`,
  `0004_addresses.sql`, `Addresses.pw`, the store's address queries and
  page), they mean this one; numbered after the first.
- **The four status documents are untouched.**
