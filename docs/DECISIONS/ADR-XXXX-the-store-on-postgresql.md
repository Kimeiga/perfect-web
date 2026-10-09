# ADR-XXXX: the store's data behind the seam, in memory and on PostgreSQL

Status: proposed by track `store-pg` (W5, `track/store-pg`), under the
integrator's rulings of 2026-10-08 (docs/PARALLEL.md, "the store on
PostgreSQL (W5's plan)" and "W5's five design points"); numbered at its
merge. Date: 2026-10-08. Milestone: E14, the app layer. Builds ADR-0246's
"Not claimed": the store on PostgreSQL.

## Context

- **The feed runs on PostgreSQL (ADR-0246); the store did not.** Each
  DoorDash gap after this one is to be built and tested on both layers, so
  the store's data must sit behind the same seam as the feed's.
- **Much of the store's state bypassed the seam.** Store 47's menu was
  written by `broadcast_menu` and by `/StorePage.html?items=`; store 48's
  menu and the catalogue were code; the stock, the recommender, the
  estimates, the notice and the kitchen's preparation time were written by
  `/bench` routes straight into `StoreData`'s fields. None of it was
  staged, committed or given its events in a transaction.
- **ADR-0193 rules that an order is the cart's lines and a status.**
  `orders#place` staged the status alone; the lines went with the emptied
  cart.
- **Delivery.** The host redraws a session's cart from the materializer's
  SQLite outbox (`drain_held`). A layer whose commit keeps its own outbox
  wrote nothing there, so a cart changed on PostgreSQL would never reach
  an open page.
- **The tests read the in-memory layer's internals** (`self.store.carts`,
  `s.store.orders`, `s.store.sold_out`, ...), so none could run on another
  layer.

## Research

- **PostgreSQL 18** (postgresql.org/docs/18, read 2026-10-08):
  - "`pg_catalog` is always effectively part of the search path": a
    connection whose `search_path` is the store's schema alone still finds
    the built-in functions (`pg_current_xact_id`, `hashtext`).
  - An unqualified name follows the search path, and "If there is no match
    in the search path, an error is reported, even if matching table names
    exist in other schemas in the database." So the store's connections
    cannot reach the feed's `outbox` or `pw_migrations` by name.
  - A constraint trigger may fire "at the end of the containing
    transaction; in the latter case they are said to be deferred": the
    refused-commit test's trigger.
  - Isolation, hot standbys and serialization failures: as ADR-0246 read
    them.
- **The client**: `postgres` 0.19.14, already in the lock (ADR-0246). The
  calls used are ADR-0246's (`Client::connect`, `batch_execute`, `query`,
  `query_one`, `query_opt`, `execute`, `transaction`, `is_closed`), with
  `Option<T>` and `i32` parameters and columns. No crate was added.

## Decision

### 1. One set of operations over two stores of rows

- **`store.rs` builds every operation once**, over a trait `Rows`: the
  stores, a store's menu, the stock, the catalogue's entry for an item, a
  session's cart and order, the notice, the kitchen, the curated
  recommendations and a session's estimate, each read and written.
- **Each layer supplies its rows and its transaction**: `StoreData`'s
  in-memory rows (`MemRows`), and `StorePg`'s (`store_pg.rs`, `PgRows` on
  the command's connection, or on a pooled one in a read-only repeatable
  read transaction for a query). What an operation answers and how a fault
  meets it is `store.rs`'s alone, so the two layers cannot answer
  differently.
- **The first data is the same rows**: `State::seed` in memory, and
  `migrations/store/0002_seed.sql` on PostgreSQL. A test holds them alike.
- **An item a menu gains** is described, priced and listed as the
  catalogue's code says of its id (`Item::new`), on both layers.

### 2. An order keeps its lines

- `orders#place` places the cart's lines with the status `placed` and
  empties the cart, in the command's transaction. Moved along, an order
  keeps its lines; one the kitchen sets with none placed has none.
- On PostgreSQL an order is a row of `orders` and its lines rows of
  `order_lines`; a session's order is its latest.
- A new read, `store:data/orders#placed`, answers the lines as a cart's
  value. No program reads it yet; the tests read an order through it.

### 3. All of the store's state through the seam

- **A route that changes data stages one of the host's own operations**
  (`store:host/menus#change`, `menus#stock`, `menus#replace`,
  `orders#set-status`, `notices#post`, `kitchen#set-prep`,
  `recommendations#curate`, `estimates#set`) through `data.begin`, and the
  host commits it with the route's declared events (`MenuChanged`,
  `InventoryChanged`, `OrderChanged`) in the same transaction
  (`Server::store_write`). A refusal (a duplicate key, an item not on the
  menu) commits nothing.
- **No program is linked to them.** The host strips `store:host/` from a
  component's operations, and refuses at start a program that names one,
  whatever capability it declares for it.
- **The fault hooks stay test controls** (`store::Faults`): `fail_next`,
  the store's delay and one-shot origin failure, each cart's delay and
  one-shot failures, the recommender's delay, failure and gate, each
  estimator's delay and failure, the slow category. Both layers apply them.
  What the recommender suggests and what an estimator estimates are data.
- **`MenuOp::Stock` carries whether the item can be ordered**, written with
  its `InventoryChanged`.
- **The categories** are rows: a store's sections (`categories`), and how
  each item is served, hot or cold (`menu_items.served`), which E14's T07
  reads by category. `/bench/category` sets a fault, the slow category, and
  no data.

### 4. One commit path, and delivery on PostgreSQL

- **`Server::commit_staged`** is the commit of a command and of a route:
  `rows`, then the layer's own commit, else the materializer's
  transaction, then `publish`.
- **A layer that keeps its own outbox and the session's entry** (the store
  on PostgreSQL) has its commit recorded in the materializer after the
  database committed: the rows and the events it read back, in one
  materializer transaction, the shape the in-memory path writes, so
  `drain_held`, `committed_basis` and the cart's entry behave as in memory.
  The database stays the authority: a record that fails after the commit
  is said, and the command is answered committed. A feed layer
  (`session_entry` false) is unchanged.
- **Every event a commit stages is consumed once delivered.**
  `Materializer::delivered(ids)` consumes a commit's events after the host
  has drained the session's entry and told its readers.

### 5. Isolation

- **Its own schema**, `pw_store` (a test's `pw_store_test_<pid>_<n>`),
  which its connections search alone (`SET search_path TO "<schema>"`).
- **Its own migrations** (`migrations/store/`), recorded in
  `pw_store_migrations` under the advisory lock
  `hashtext('pw_store_migrations')`, never the feed's.
- **Its own outbox**, read back after the commit with the feed's code
  (`feed_pg.rs`'s `json_of` and `delivered`, made `pub(crate)`).
- **Every command serializable**, set on each transaction, and measured by
  `provides()` as the feed's is. At most eight connections per layer.
- **`PW_STORE_DATABASE_URL`** opens it; `PW_FEED_TRANSACTIONS=connection`
  leaves its isolation to the connection's default, as the feed's.

### 6. The source

- `examples/lib/StoreData.pw`'s comment said "one SQLite database". It now
  says the database the deployment opens, which the host measures.
- **`Notices`, `Kitchen` and `Categories` are stated, not held.** The
  benchmark's tasks (T09, T02, T07) add them as modules of their own, which
  `StoreData` cannot import. They are in the same database, and `held_to`
  holds a resource no source holds to the host database's guarantees
  (ADR-0207): serializable, strong, no feed, what `StoreData` states.

### 7. Tests on both layers

- **Read through the layer**: `cart_value`, `cart_lines`, `cart_priced` and
  `order_of` call the layer's reads. Arrangements write through the host's
  operations, committed with no event where a change is untold
  (`untold`, `stock_untold`, `menu_untold`, `order_set_untold`).
- **The whole server suite runs on either layer.** `store::layer_for_tests`
  serves each test's store in memory, or with `PW_STORE_TEST_LAYER=postgres`
  on PostgreSQL in a schema of its own, dropped with it. One test is
  layer-shaped: a line of 0 written around the program is refused by
  PostgreSQL's `cart_lines_quantity_check` itself, and the test says so.
- **The browser suite stays in memory**: Playwright's `IN_MEMORY` clears
  `PW_STORE_DATABASE_URL` as it clears the feed's.

## Questions to the integrator, and its answers

All of 2026-10-08, relayed through the coordinating session; recorded in
docs/PARALLEL.md, "W5's five design points".

1. *May the commit tail be factored into one `commit_staged`, and may a
   session-entry layer with its own outbox have its commit recorded in the
   materializer?* Accepted, on one condition: the record is the rows and the
   events in one materializer transaction, the in-memory shape. Test a cart
   changed on PostgreSQL reaching an open page.
2. *Is "a command's path" a host operation staged through `data.begin`,
   not a new `command` in `app.pw`?* Accepted: the merchant's side is out of
   scope. Test that a program naming one is not linked to it.
3. *`StoreLayer` beside an unchanged `DataLayer`, and
   `PW_STORE_DATABASE_URL`?* Accepted, with the store's `search_path` its
   schema alone.
4. *`pub(crate)` on `feed_pg.rs`'s helpers?* Accepted, visibility only; a
   later behaviour change runs the feed's mutation scripts whole.
5. *The whole suite on both layers?* Accepted, as the stronger evidence:
   `just ci` in memory, `e14-store-postgres` both ways on CI.

And, unasked: the diagnostic block PW59 (`Owner::StorePg`), registered;
PORT=7141 for browser runs, none needed; never drop a schema this track did
not create.

## Found

- **An order dropped its lines** (ADR-0193), as the rulings found: fixed in
  memory first, with its test, then on PostgreSQL.
- **`OrderChanged` was never consumed** from the materializer's outbox in
  memory. A drain consumes an event only where it reaches an instance its
  caller names, and the host names only the session's cart; the order's
  query listens for it. Each order placed left a row for good. The
  integrator assigned the fix here; `every_event_a_commit_stages_is_consumed_once_delivered`
  holds it on both layers.
- **A menu change's event was committed nowhere.** `broadcast_menu` changed
  the server's own list and invalidated queries with a `MenuChanged` no
  transaction held. It is now committed with the change, and consumed once
  told.
- **`/bench/stock` set a stock the menu did not know.** An untold stock
  change of an item no menu has used to enter a set and wait there; it now
  writes no row.
- **`in-category` answered `not-found` for store 48** though it holds a
  menu. It now answers each store's items by how they are served.

## Alternatives

- **Each layer its own operations**, as the feed's two are. The store's
  operations carry its faults and its answers' shapes; written twice, they
  would drift, and the parity test would be the only thing holding them.
  One set over `Rows` makes the layers differ only in rows.
- **New commands in `app.pw` for the merchant's writes.** The merchant's
  side is out of scope (answer 2); a program command would be granted to
  every page.
- **The test controls as data.** A delay or a held gate is what a source
  does, not what it holds; rows of them would commit a test's fault.
- **The store in the feed's schema, or the public one.** Both have an
  `outbox`; a shared migration table would let one host's migration
  believe the other's applied.
- **Keep the in-memory tests as they were and add PostgreSQL tests
  beside.** The rulings ask the store's tests to run on both layers; the
  internals the tests read were the seam's bypass itself.

## Consequences

- The store can run on PostgreSQL, held to its source, with its carts,
  orders and menus delivered to open pages as in memory.
- The DoorDash gaps after this one are built once, over `Rows`, and tested
  on both layers by the same suite.
- `just ci` runs the suite in memory; `e14-store-postgres` runs it both
  ways, in a database shard of its own on CI (`NEEDS_DATABASE`).
- The in-memory layer clones its small `State` once per command, behind an
  `Arc` for reads.

## Acceptance

Recorded by `just e14-store-postgres` in
`docs/evidence/E14/store-postgres.txt`, against PostgreSQL 18.6:

- **`server/src/tests/store_pg.rs`, 12 tests**: the database provides what
  the store states (its default read committed) and its connections search
  its schema alone; a cart's change commits with its event and its dropped
  entry in one transaction, and the outbox is consumed; a route's change
  (the kitchen's) commits with its `OrderChanged` in one transaction; a cart
  changed on PostgreSQL reaches the session's open page, its rows recorded;
  a commit refused by a deferred constraint keeps neither its line nor its
  event, records nothing, tells no page, and the next add commits; eight
  sessions adding five times each commit all forty; a retried interaction
  runs its command once; an order keeps its lines and both outboxes end
  empty; the two layers answer 22 reads alike, before and after the same
  writes, and serve five pages to the byte; the store's schema is its own
  beside the feed's; negative controls: a stated change feed and
  serializable against a read committed default are refused, and served
  against a serializable default and with serializable set on each.
- **The server's whole suite, 304 tests beside those, with the store on
  PostgreSQL, and again in memory.**
- **`scripts/store_postgres_mutations.py`: 12 of 12 mutants killed**: an
  order without its lines (either layer, and PostgreSQL's), a commit without
  its events, a transaction at the connection's default, the isolation
  answered not measured, a refused commit answered committed, the outbox not
  read back, the commit not recorded in the materializer or recorded without
  its rows, the search path widened, a program let name a host operation,
  and a delivered event left in the outbox.
- **`pw-materialize`**: an event its host delivered is consumed, once, and
  another commit's is kept.

## Not claimed

Inherited from ADR-0246:

- **A second host.** Delivery is the committing host's; another host on the
  same database hears nothing of the first's commits.
- **Idempotency committed with the writes** (PW0348). The interaction is
  remembered by the host in memory; queued before checkout, the
  integrator's.
- **A measured serialization failure.** One host's commands are serialized
  by the layer's lock; serializable is exercised against another writer
  only through a refused commit.
- **The browser suite on PostgreSQL.** It runs in memory.

And this track's own:

- **Several orders a session keeps.** The rows allow it; a session's order
  is still its latest (ADR-0193).
- **The merchant's side.** The host's own operations are its routes',
  tests' and benchmarks'; no merchant page writes them.

## Report

### What was built

- `store.rs` rewritten: the operations once over `Rows`, the faults, the
  in-memory rows; `store_pg.rs`, the PostgreSQL layer;
  `migrations/store/0001_schema.sql` and `0002_seed.sql`.
- `main.rs`: `commit_staged` and `store_write`; the routes through the
  layer; the host's operations refused to programs and stripped from
  components; `from_build_layers` and `PW_STORE_DATABASE_URL`; the tests
  through the layer.
- `pw-materialize`: `Materializer::delivered`.
- `codes.rs`: `Owner::StorePg => "PW59"`, no code yet.
- `StoreData.pw`'s comment; `playwright.config.mjs`'s `IN_MEMORY`.
- The recipe `e14-store-postgres`, in `NEEDS_DATABASE`.

### Tests and mutants

- Locally, PostgreSQL 18.6: the suite 316 of 316 in memory and 316 of 316
  with the store on PostgreSQL; `tests::store_pg::` 12 of 12;
  `store_postgres_mutations.py` 12 of 12 killed.
- Re-anchored, by ADR-0281 run whole on CI: the eleven of the thirteen
  scripts with anchors in `store.rs` whose anchors moved (`availability`,
  `cart_lines`, `command_answers`, `estimate_range`, `home`,
  `last_known_good`, `menu_categories`, `orders`, `query_values`, `slots`,
  `test_controls`; `feed_served` and `whole_fills` were unchanged), and
  `committed_events` and `menu_changed`, whose anchors in `main.rs` moved.
  `just mutation-anchors` holds every anchor to one place.

### Merge notes

- Rebased on `master` at `de64ad7`. ADR-0280 (navigate after commit) had not
  landed; it meets this track at the place-order handler, `store-ir.json`
  and two e2e specs, none of which this track changed.
- `feed_pg.rs`'s change is visibility only.
- The integrator's accounts-in-the-store work can build on `Rows` and the
  host's operations.
