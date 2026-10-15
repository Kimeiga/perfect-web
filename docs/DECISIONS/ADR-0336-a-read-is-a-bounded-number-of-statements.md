# ADR-0336: a read is a bounded number of statements on PostgreSQL

Status: accepted under the owner's delegation of 2026-10-02 (W9's, NEXT 4b,
track `bounded-statements`, with the integrator's rulings of 2026-10-14;
merged on train-8). Date: 2026-10-14. Builds on ADR-0308
amended (the counted connection), ADR-0328 and ADR-0329 (whose
KNOWN_LIMITATIONS lines this answers), and feeds NEXT 4c's design note (a
read is bounded by construction), whose "Measurements (4b)" are this
ADR's counts.

## Context

- **The store's logic is written once over the layer's rows**, a store's or
  a menu's at a time (`store.rs`' `Rows`), and on PostgreSQL each is a
  statement. The integrator's finding of 2026-10-12 named where that grew
  with the data:
  - the order's reach (`cart_reaches`): each line's store found by reading
    every store's menu, and each store's coverage read alone;
  - a reader's search (`search#find-for`): a coverage read per store found,
    each reading every store;
  - the hours (`hours#for-stores`, `hours#for-cart`): three statements per
    store, and each cart line's store found by every store's menu;
  - the checkout (`checkout#parts`): each line's store, the same way.
- **What others count.** Django's `assertNumQueries` and Rails'
  `assert_queries_count` pin the queries a block runs; a count that grows
  with the data is the N+1.

## Decision

1. **Rows read by set.** The layer's rows gain four set-shaped reads, each
   one statement on PostgreSQL whatever the number of keys (`= ANY($1)`),
   and a scan in memory:
   - **a store by id**, `stores_by_ids(ids)`: the stores among `ids`, which
     `stores#get`, a menu's existence and a coverage read use, not every
     store's row;
   - **the stores of a set of items**, `stores_of_items(items)`: each item's
     store, the first in the stores' order whose menu lists it (`DISTINCT
     ON (i.id) ... ORDER BY i.id, s.position`), the order the scan gives;
   - **the hours of a set of stores**, `hours_of(stores)`: three statements
     whatever their number, the zones and cut-offs, the weekly periods and
     the special dates, each by set, in the orders a store's own read gives
     them (ADR-0328's contract);
   - **the coverage of a set of stores**, `coverage_of(stores, owner)`: the
     owner's addresses once and the stores' rows at once (`store.rs`).
2. **The loops go.** `cart_reaches`, the search's reach, `hours#for-stores`,
   `hours#for-cart` and `checkout#parts` read by set. The scans remain as
   memory's way, named (`scanned_stores_of_items`, `hours_one_by_one`), and
   the mutants put them back.
3. **Every operation counted, per call** (`tests/bounded_statements.rs`):
   each of the 63 operations of the store's data layer, reads and writes
   (`store:data/…` and the host's `store:host/…`), called through the layer
   at the seed and with fifty stores more (listed before the seeded ones, so
   a loop that stops at the store it wants still meets every one), each
   count pinned and equal at both: "operation X makes k statements per
   call", the shape a budget composes from (NEXT 4c). An operation the test
   does not name fails it: every one is counted.
4. **Every page and command counted** on a store served on PostgreSQL: the
   reader adds a line from each seeded store, saves an address, chooses a
   tip, reads the home page, the store's, the cart, a search and the
   address page, places the order and reads it; each step's statements
   pinned and equal at both sizes.
5. **The layer's own tally.** A statement is counted on the thread that runs
   it (`STATEMENTS`, ADR-0308 amended); `store_pg.rs`' `Tally` adds each
   call's to the layer's total, whatever thread ran it, which a page's or a
   command's count is read from (`DataLayer::statements`, a test's); and
   each operation's, and each command's transaction's, apart.
6. **A command's commit is counted with it** (the integrator's ruling of
   2026-10-14): what it hands the outbox (an insert per event and per
   invalidated entry), its interaction's record (two statements, ADR-0325),
   its scheduled runs (ADR-0334: a delete for each, and an insert for each
   run due) and the outbox read back (one), each bounded by what the command
   hands the platform, never by the data's size. The control: `choose_tip`
   emitting one event more makes one statement more.
7. **A command is pinned apart**, and **its scenario named** (the
   integrator's ruling of 2026-10-14), both pinned:
   - (a) **a deployment taking no payments: placed at once** (ADR-0332, the
     development store's): `begin_payment` answers `BeginError.NotTaken`,
     and `place_order` settles its order inline, in its own transaction;
     its operations, its commit and the session's queries read again after
     it, apart;
   - (b) **a deployment taking payments: awaiting** (the production path):
     the development provider configured as `tests/store_payments.rs`'
     `paying` does; `place_order`'s transaction leaves the order awaiting,
     no settle among its operations, and `settle_payment`, on the
     provider's word (a card paid on its page), runs in its own.

## Measurements

On PostgreSQL 18.6, 2026-10-14; each the same with fifty stores more.

Per operation (statements per call):

| Operation | k | Operation | k |
| --- | --- | --- | --- |
| stores#get, stores#list | 1, 1 | hours#for-store | 3 |
| menus#for-store, #sections, #in-category | 2, 2, 2 | hours#for-stores (all stores) | 3 |
| menus#is-available | 1 | hours#for-cart | 6 |
| search#find | 2 | fees#for-stores (all stores) | 1 |
| search#find-for | 4 | checkout#parts | 2 |
| coverage#for-store, #reaches | 2, 2 | checkout#chosen | 1 |
| user-estimates#for-store | 3 | promos#find | 1 |
| recommendations#for-store | 3 | user-orders#await-payment | 14 |
| carts#current, user-carts#current | 1, 1 | user-orders#paying, #settle | 2, 2 |
| user-carts#add | 4 | host hours#set | 13 |

The full table, all 63, is the test's `PINNED`. A write's count grows with
what it writes, a line, a period or an item (`user-orders#await-payment`
writes the cart's two lines, `hours#set` a week's seven periods), never
with the number of stores.

Per page and command, each command with its commit: `add_to_cart` 10 and
11, `add_address` 6, `choose_tip` 6, the home page 5, the store's page 16,
the cart 13, a search 8, the address page 2, the order's page 5.

`place_order`, by scenario:
- (a) taking no payments, placed at once: its operations 33, its commit 7,
  its queries read again 21;
- (b) taking payments, awaiting: its transaction 37 (its sweep scheduled,
  2); then `settle_payment` on the provider's word, its own transaction, 17
  (the sweep cancelled, 1). And the order taken back: `cancel_order` 25,
  then `settle_payment` on the provider's word of the cancel 14; the sweep
  run thirty minutes on (the clock moved, then `run_due`): `expire_order`
  26, then the provider's word of its cancel 12.

`place_order` in (a), by operation (the layer's tally per operation):

| Where | Operation | Calls | Statements |
| --- | --- | --- | --- |
| its transaction | hours#for-cart (`closed_in`) | 1 | 6 |
| its transaction | checkout#parts, fees#for-stores, checkout#chosen (`priced_for`) | 3 | 4 |
| its transaction | user-orders#await-payment | 1 | 16 |
| its transaction: no payments taken, so the order is settled at once (ADR-0332) | user-orders#settle | 1 | 7 |
| its commit | the outbox's inserts, the interaction's record, the outbox read back | | 7 |
| after the commit, the session's invalidated queries read again | user-carts#current (`Cart`) | 6 | 6 |
| after the commit | hours#for-cart (`CartHours`) | 1 | 6 |
| after the commit | checkout#parts, fees#for-stores, checkout#chosen (`Checkout`) | 3 | 4 |
| after the commit | user-orders#current (`Order`), coverage#for-store and addresses#list (the estimate) | 3 | 5 |

NEXT 4c's note takes this as its measured baseline.

## Alternatives

- **Count rows as well as statements.** `stores#get` read every store's row
  in one statement before; a statement count cannot tell. Its set read is
  in, and rows are not counted: NEXT 4c's budget is calls.
- **A read cache in the layer.** It hides the loop rather than removing it,
  and a command's reads must see its own writes.
- **Leave memory's scans unnamed.** Naming them is what lets a mutant put a
  loop back exactly.

## Acceptance

- `tests/bounded_statements.rs`, on PostgreSQL: every operation per call,
  and every page and command, at the seed and with fifty stores more,
  pinned and equal at both; a command's commit counted with it, and its
  control.
- `scripts/bounded_statements_mutations.py`, 7 mutants, each restoring one
  loop, narrowed to `tests::bounded_statements::`: `just
  e14-bounded-statements`.
- The store's suites in memory and on PostgreSQL unchanged.

## Not claimed

- **Rows read**: `stores#list` and the home page read every store, as they
  must; the count is of statements.
- **Loops bounded by a cart**: the order's reach reads the coverage of the
  cart's stores at once, but a cart's lines are written a statement each.
- **What 4c makes impossible**: this measures and pins; a program that
  reads per element is refused only once 4c is built.

## Merge notes

- KNOWN_LIMITATIONS: remove "the hours' reads run a statement per store on
  PostgreSQL (hours#for-stores, hours#for-cart)" (ADR-0328) and "a cart's
  lines are mapped to their stores by a read per store (checkout#parts, as
  hours#for-cart)" (ADR-0329): both are one read by set now.
- `just e14-bounded-statements` joins NEEDS_DATABASE.
