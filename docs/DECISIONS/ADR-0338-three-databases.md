# ADR-0338: one app on three kinds of database, each held to what it gives

Status: accepted under the owner's delegation of 2026-10-02, by the
integrator's ruling of 2026-10-15 (below), the owner's decisions below
still the owner's; a design note by track `threedb-design` (the
coordinator's brief of 2026-10-14), merged on train-8. Docs only: nothing
here is built yet. Date: 2026-10-14. Milestone: E14, the app layer: the
sixth reference app of `docs/vision/capabilities.md`, "the app on three
kinds of database" (NEXT, item 22's third). Builds on ADR-0005 (SQLite
first), ADR-0207 (a data source states what it guarantees), ADR-0218 (a
host serves any program; its data is the deployment's), ADR-0246 and
ADR-0298 (the feed and the store on PostgreSQL, the reference), ADR-0325
(an interaction's answer commits with its writes), ADR-0334 (a scheduled
run commits with its writes), NEXT 4b (every operation counted, track
`bounded-statements`) and the 4c note (`sets`, a set read's cost, track
`bounded-reads`, ADR-0337, ruled 2026-10-14).

The coordinator's addition of 2026-10-14 is the central question, and
decision 1: an app's data declared in Pleris, with one generic adapter
per database implementing every app's, so that no app needs Rust.

## Ruling (the integrator, 2026-10-15)

Accepted under the owner's delegation of 2026-10-02, with the amendments
below; every decision stands as written where no amendment names it.

1. **Decision 1 is the direction for every app's data**: declared in
   Pleris, served by one generic adapter per database, no app-specific
   Rust (the owner's steering of 2026-10-15, its second item; the
   architect's E10-S10, generalized from the carts to every operation).
   Built in decision 6's order, D1 first.
2. **Names (amends 1.2).** A set form ends in `_many`, as its host
   primitive does (`get_many(keys)`, `of_owner_many(owners)`,
   `by_store_many(stores)`), so that no set read is read as `all()`, the
   bounded table's whole read. A read by a key's prefix is named by the
   prefix's fields (`CartLines.of_owner(owner)`; a two-field prefix of a
   three-field key, `of_owner_store(owner, store)`), so a read says what
   it reads and no name is told apart by its arity. The writes:
   `insert(row)` (refused where the key exists), `put(row)` (the whole
   row), `delete(key)`, `delete_of_<fields>(prefix)`.
3. **The word and its clauses** (question 1): `table`, with `row`, `key`,
   `order`, `index`, `unique`, `refers` and `bounded`, and the imports'
   namespace `pw:tables/<table>#<primitive>`: accepted.
4. **Codes** (question 2): the table refusals are declaration rules,
   PW0354 to PW0366 in `DeclarationRules` beside ADR-0207's (free on master
   and on every pushed track branch at 2026-10-15 02:30Z); D1 checks again
   at its start and takes the next free code for any that is gone.
   PW0347's and PW0349's revisions come with T1.
5. **TodoMVC is D2's first app with no Rust data layer** (question 1):
   track `todomvc`'s milestone-1 layer (`server/src/todomvc.rs`) is the
   "before" that D2 deletes.
6. **The 4c note's `set` pairing is generated for declared tables**
   (question 1); a hand-written layer keeps its written pairing until its
   app's data is declared (D4, D5).
7. **One conformance suite** (amends 1.5 and 5.5): the 4c note's suite
   (per layer, two data sizes) and decision 5.5's are one suite, written
   once against the `DataLayer` seam, run first against today's
   hand-written layers (4c's track) and generated for every declared table
   from D2 on.
8. **The seam** (question 3), `sources.json`'s `sets` and `tells`, and
   `tables.json` (question 4): accepted.
9. **The name** (question 5): `StoreData` stays until the track that
   splits the store's data (T1 with T3) names the parts it splits; no
   rename while there is one source.
10. **The order** (question 6): accepted: D5's composites in the 4c note's
    order; T3 after the search, a line's price and a cart's stores are
    Pleris.
11. **`transactions partition`** (question 7): a word of ADR-0207's
    `transactions`, as proposed.
12. **The owner's decisions** (1 to 7) stay the owner's. None blocks D1 to
    D5, T1 or T2, which download nothing; the coordinator asks them before
    T3, T4 or T5 starts.

## Context

- **The owner's ask** (relayed 2026-10-04 and 2026-10-08): apps use
  different databases, and whatever the database, the part Pleris owns
  must be perfect: taking data, using it, displaying it, and letting the
  user interact with it.
- **Where that part ends.** Charter §2: do not build "a database engine"
  or "a distributed database". §10.4: "Do not build a database. Use
  ordinary transactions and a transactional outbox for invalidation
  events." ADR-0005: SQLite first, PostgreSQL when nodes separate, both
  behind one interface. So the boundary is the data-source contract: a
  source states what its database guarantees (ADR-0207), the compiler
  holds the program to the statement, and the host holds the database to
  it before it serves (ADR-0246).
- **The owner's databases** (NEXT, relayed 2026-10-08), chosen to differ
  most from PostgreSQL, the reference (ADR-0246, ADR-0298): SQLite, libSQL
  or Cloudflare D1; MongoDB or Firestore; Cassandra or ScyllaDB, against
  which "the compiler refuses the cart's `transaction serializable` and
  `read_your_writes`", while the parts of the app it can hold build and
  run; and, optionally, CockroachDB. Each guarantee is verified in its
  database's own documentation, each layer measures what its database
  gives, and every download, and every server run on the owner's machine,
  waits for the owner's approval.
- **What is built** (`master` at `5da9c055`):
  - Four sources are declared: `StoreData` and `FeedData`
    (`transactions serializable`, `reads strong`, `changes none`), and
    kiokun's `KiokunEntries` and `KiokunSearch` (`transactions none`,
    `reads strong`, `changes none`).
  - The checker holds a program to its sources (PW0344 to PW0349).
  - The host serves a program over one data layer, picked by its
    contracts' import prefix (`from_build_layers`), and `held_to`
    compares every declared source with that one layer's `provides()`.
  - A command's writes, events, interaction's record and scheduled runs
    commit in its layer's transaction (ADR-0246, ADR-0325, ADR-0334). The
    events reach the committing host's sessions only: "A second host on
    one database hears nothing of the first's commits" (capabilities.md).
  - The store's operations are written once over `Rows`, for two layers
    (memory and PostgreSQL, ADR-0298). 4b counted each one on PostgreSQL;
    the 4c note proposes `sets`, what a set read costs.
- **Found reading the store for this note:**
  - Its commands decide on `Stores` (hours, fees, promotions, zones) and
    on `Menus` (whether an item can be ordered). Where those are held
    decides which commands read across databases.
  - Four operations read the menus below the seam, which their effect
    rows do not say: `user-carts#add` (`catalog_item`, the line's name
    and price), `user-orders#await-payment` (`cart_reaches`, each line's
    store found in every store's menu), and `checkout#parts` and
    `hours#for-cart` (`store_of`, the same way). With the menus in
    another database, the layer holding the cart cannot read them, which
    is right, so these change first; the 4c note's decision 3 already
    moves them above the seam.
  - `search#find` and `search#find-for` declare `database.read<Stores>`
    and `database.read<Menus>`: one operation over what would be two
    databases.
  - A command that schedules commits its run with its writes (ADR-0334),
    and nothing holds that to its source's transactions, as PW0347 and
    PW0348 hold its events and its interaction's record.
  - kiokun.com runs on Cloudflare D1 in production, read only (ADR-0314);
    and `rusqlite` 0.40.1, with SQLite bundled, has been in `Cargo.lock`
    since ADR-0019, so a SQLite layer downloads nothing.

### What each database gives

Read in each database's own documentation on 2026-10-14, each page's
version or date given; download sizes from each file's HTTP headers that
day; crates from crates.io's API.

**PostgreSQL 18**, the reference (postgresql.org/docs/18, "PostgreSQL 18.6
Documentation"):
- Transactions and reads as ADR-0246 read them (2026-10-07): Read
  Committed by default; Repeatable Read is snapshot isolation;
  Serializable is Serializable Snapshot Isolation; a hot standby is read
  only.
- Changes: `NOTIFY` is delivered only once its transaction commits, with a
  payload under 8000 bytes, to the sessions listening (ADR-0246). Logical
  decoding needs `wal_level = logical`, set at the server's start;
  "Concurrent transactions are decoded in commit order", and rolled back
  ones "never get decoded" (§47.6.4); a slot holds its WAL until it is
  consumed, even with no consumer connected, and after a crash may send a
  change again (§47.2). `postgres` 0.19.14 and `tokio-postgres` 0.7.18
  have no replication protocol (rust-postgres issue #116, open since
  2015), so a layer would read a slot by SQL (`pg_logical_slot_get_changes`,
  §9.28.6).
- Sets: `= ANY(array)`, one statement for any number of keys (§9.25.3).

**SQLite** (sqlite.org):
- Transactions: "Transactions in SQLite are SERIALIZABLE", and "There can
  only be a single writer at a time" (isolation.html, 2022-04-18). In WAL
  mode a reader sees a snapshot, and "readers do not block writers"
  (wal.html, 2026-08-25). A transaction begins `DEFERRED` by default, and
  one that read first fails with `SQLITE_BUSY` when it writes after
  another writer committed; `BEGIN IMMEDIATE` takes the write lock first
  (lang_transaction.html, 2026-02-18). `BEGIN CONCURRENT` lives on a
  branch, not in a release (its branch's document; a forum reply of
  2024-10-25: "no concrete plans to merge").
- Reads: a new read transaction sees the latest commit; WAL "does not work
  over a network filesystem", so every reader is on the one machine.
- Changes: none across connections. `sqlite3_update_hook` is registered
  on one connection (c3ref/update_hook.html), and the session extension
  watches changes "via a single sqlite3* database handle"
  (sessionintro.html); another connection's commit is seen only by
  polling `PRAGMA data_version` (pragma.html).
- Sets: 32766 host parameters since 3.32.0 (limits.html, 2026-07-10); a
  list as one JSON parameter read by `json_each` (JSON built in since
  3.38.0, json1.html), or rusqlite's `rarray` (its `array` and `vtab`
  features): one statement.
- 3.54.0 (2026-10-09) is the newest; `rusqlite` 0.40.1 bundles 3.53.2, as
  0.40.2 (2026-08-08) does; SQLite is public domain.

**libSQL** (github.com/tursodatabase/libsql, docs.turso.tech):
- A fork of SQLite, "actively maintained, but new features are being
  developed in Turso" (README); Turso's docs recommend Turso Database for
  a new project, which has "not yet reached 1.0", its MVCC "not production
  ready" and its CDC a table per connection, with nothing pushed.
- Remote: an interactive transaction locks the database for writing, with
  a five-second timeout (docs.turso.tech/sdk/ts/reference). Embedded
  replicas send writes to the primary and promise to read one's own
  writes (embedded replicas, introduction).
- No change feed found. `libsql` 0.10.0 (2026-10-13) is asynchronous;
  `sqld`'s last release was 2025-02-14.

**Cloudflare D1** (developers.cloudflare.com/d1):
- Transactions: "D1 operates in auto-commit"; a `batch()` is one
  transaction, a failure rolling back the sequence (Worker API,
  2026-06-22). There is no interactive transaction: `BEGIN TRANSACTION`
  is refused (Cloudflare's blog, 2022-09-27; the import guide,
  2026-04-21, says to remove it). A database processes "queries one at a
  time" (limits, 2026-04-21).
- Reads: without a session every query goes to the primary. Read
  replication needs the Sessions API, which "ensures sequential
  consistency for all queries in a session", reading one's own writes and
  never older than the last read; a replica alone "may be arbitrarily out
  of date" (read replication, 2026-08-10; public beta in its 2025-04-10
  changelog).
- Changes: none found (no D1 event among Cloudflare's event
  subscriptions, 2026-09-25).
- Sets: 100 bound parameters a query; a statement at most 100 KB; 1,000
  queries a Worker invocation on the paid plan, 50 free; 10 GB a
  database (limits).
- Outside a Worker, its REST query API is "best suited for administrative
  use", under the API's 1,200 requests per five minutes. Locally, `wrangler
  dev` keeps a SQLite file under `.wrangler/state`.

**MongoDB** (mongodb.com/docs/manual, "9.0 (Current)"):
- Transactions: a standalone server has none; a deployment "must be a
  multiple node replica set". Multi-document transactions read at read concern
  `snapshot`; one that meets a write outside it aborts, retried whole on
  `TransientTransactionError`; one runs "less than one minute" by default
  (transactions, production considerations). A single document's write is
  atomic (write-operations-atomicity).
- Reads: only `majority` reads with `majority` writes "can guarantee all
  four causal consistency guarantees", reading one's own writes among them
  (causal-consistency-read-write-concerns). Read concern `snapshot` works
  outside a transaction since 5.0 for `find`, `aggregate` and
  `distinct`; `linearizable` only on the primary, one document at a time
  (read-concern).
- Changes: "Change streams are available for replica sets and sharded
  clusters", and notify only of changes "persisted to a majority of
  data-bearing members"; a stream resumes from a token while "The oplog
  must have enough history" (changeStreams).
- Sets: `$in` states no maximum; a document is at most 16 MiB; a `find`'s
  first batch is 101 documents, a batch at most 16 MiB, the rest by
  `getMore` (limits, cursors).
- 9.0.4 is the newest (release notes, 2026-10-13); its Ubuntu 22.04
  x86_64 tarball is 152,030,619 bytes; the licence is SSPL v1. The
  `mongodb` crate 3.9.1 (2026-09-10) has transactions, change streams, a
  blocking API behind `sync`, and command monitoring
  (`command_event_handler`).

**Firestore** (firebase.google.com/docs/firestore, cloud.google.com;
pages updated 2026-10-13 unless said):
- Transactions are serializable, pessimistic in the Standard edition and
  optimistic in Enterprise, 270 s at most and 60 s idle
  (transaction-data-contention, quotas). Reads are strongly consistent by
  default, stale by a `read_time` (understand-reads-writes-scale).
- Changes: realtime listeners, through `Listen` over gRPC or WebChannel
  only, not REST (RPC reference, 2026-04-25).
- Sets: `batchGet` by name, streamed, no maximum stated; a query's `in`
  takes up to 30 values (the 4c note).
- With MongoDB compatibility (Enterprise edition, generally available
  2025-08-26): MongoDB's drivers; snapshot isolation by default,
  serializable supported; change streams in Preview since 2026-04-20.
- The emulator needs Java 21 since firebase-tools 15.0.0 (2025-12-10) and
  does not implement all of production's transactions
  (connect_firestore). Google has no Rust client for its data
  (`google-cloud-firestore` 0.0.0, unpublished); a community crate,
  `firestore` 0.58.0, speaks gRPC.

**Apache Cassandra** (cassandra.apache.org/doc/5.0):
- Transactions: "Cassandra explicitly chooses not to implement operations
  that require cross-partition coordination", "Cross-partition
  transactions" first among them (overview). A logged batch eventually
  applies all its mutations or none, and "All updates in a BATCH belonging
  to a given partition key are performed in isolation" (dml).
- Reads: a consistency level per operation, `ONE` "Only a single replica
  must respond", `QUORUM` a majority; a read meets a write where "`W + R >
  RF`" (dynamo).
- Conditional writes: "single partition compare and swap functionality via
  the lightweight transaction CQL API" (dynamo), Paxos, linearizable
  (guarantees). `IF NOT EXISTS` "will incur a non-negligible performance
  cost" (dml): four round trips a write with Paxos v1, the default, two
  with v2 (cassandra.yaml). A batch with conditions cannot span
  partitions or tables (`BatchStatement.java`, branch cassandra-5.0).
- Changes: CDC hard-links commit log segments into `cdc_raw` on each node,
  for a consumer on that node to parse with Cassandra's own Java reader;
  writes to a CDC table "will be rejected unless some consumption process
  is in place" (cdc).
- Sets: "The IN relationship is only allowed on the last column of the
  partition key or on the last column of the full primary key" (dml): a
  set read across partitions is a request a key (the 4c note).
- 5.0.9 is the newest generally available release (2026-08-07), its
  tarball 73,341,539 bytes, on "Java 11 or Java 17"; Apache-2.0. Accord's
  multi-partition transactions (CEP-15, strict serializable) ship only in
  6.0's alphas (6.0-alpha2, 2026-08-07), "intended only for development
  and testing purposes", enabled per cluster (`accord.enabled`) and per
  table (`transactional_mode`).

**ScyllaDB** (docs.scylladb.com, 2026.3):
- Consistency levels and lightweight transactions as Cassandra's (Paxos,
  linearizable, conditional batches within one partition); Raft "strongly
  consistent tables" experimental since 2026.2; no multi-partition
  transaction.
- Changes: CDC writes a log table beside the base table
  (`<table>_scylla_cdc_log`), read with plain CQL, ordered within a stream
  only, kept a day by default, consumed with ScyllaDB's libraries
  (`scylla-cdc` for Rust, Apache-2.0, its tablets support with open
  issues).
- 2026.3.3 is the newest (2026-10-07), from apt packages for Ubuntu
  22.04, in a developer mode on a machine without XFS; WSL is not in its
  documentation. Since 2025.1 its licence is source-available (the
  ScyllaDB Software License Agreement 1.1: free within 10 TB and 50
  vCPUs, never as a service); 6.2 was the last AGPL release.
- The `scylla` crate 1.9.0 (MIT OR Apache-2.0) is asynchronous on tokio,
  speaks CQL's protocol v4, and is "compatible with Apache Cassandra".

**CockroachDB** (docs.cockroachlabs.com, stable v26.3):
- Transactions: "By default, CockroachDB executes all transactions at
  `SERIALIZABLE` isolation"; Read Committed by choice; a retry error
  (`40001`) is the client's to handle (transactions).
- Reads: strongly consistent from the leaseholder by default; follower
  reads by `AS OF SYSTEM TIME`, stale and consistent (transaction layer,
  follower reads).
- Changes: a changefeed, sinkless to the SQL session or into a sink, with
  "an at-least-once delivery guarantee", ordered per key for a message's
  first emission, duplicates "under some infrequent conditions"
  (changefeed messages); rangefeeds must be on (`kv.rangefeed.enabled`,
  off by default when self-hosted).
- PostgreSQL's wire protocol; rust-postgres listed with partial support
  (third-party tools); session-scoped advisory locks unsupported,
  transaction-scoped ones supported (postgresql-compatibility).
- v26.3.2 (2026-09-18) is the newest, its tarball 149,960,594 bytes;
  v26.2.7 the long-term release. Since 24.3 its licence is the
  CockroachDB Software License: "No license key is required for
  developers running single-node clusters", and a larger cluster without
  one is throttled after seven days (licensing FAQ).

**GitHub Actions** (workflow syntax): a service container's `command`
"Overrides the Docker image's default command (`CMD`)", so a MongoDB
service can start `--replSet`. Docker Hub's newest images: `mongo:9.0.4`,
`cassandra:5.0.9`, `scylladb/scylla:2026.3.3`,
`cockroachdb/cockroach:v26.3.2`.

**So** each database refuses some of what the store asks, except the SQL
ones; what each gives depends on its deployment (a replica set or one
server, a consistency level, a table's CDC, Accord enabled); and every one
has a set read, at a cost of its own. The program must state what it
needs, the source what the database gives, and the layer measure it.

## Decision

### 1. An app's data is declared in Pleris, and one adapter per database implements every app's

- **Today every app's data layer is Rust in the host**: each operation a
  program imports (`store:data/carts#add`, `feed:data/posts#publish`) is
  written by hand, once in memory and once on PostgreSQL (`store.rs` and
  `store_pg.rs`, `feed.rs` and `feed_pg.rs`); capabilities.md says so:
  "each data layer is written by hand in Rust". The architect's ruling of
  2026-08-20 already asks for the store's to become "compiled Pleris over
  narrower primitives" (`docs/EVIDENCE_LEDGER.md`, E10-S10, open). A
  framework for any web app cannot ask each app for Rust, and three
  databases would ask for it three times more.
- **Decided: a program declares its data, and one generic adapter per
  database implements every program's declarations**: the statements,
  the set reads, the migrations, the feed and the counted conformance
  suite. The host keeps narrow generic primitives; the app's logic,
  every composite the 4c note moves above the seam, is Pleris.

#### 1.1 The declaration

```
type CartLine = CartLine { owner: Owner, item: MenuItemId, store: StoreId,
    name: String, price: Money<USD>, quantity: PositiveInt, added: Int }

table CartLines
    row     CartLine
    key     owner, item
    order   added
    index   by_store on store
    refers  item to MenuItems
```

- **`row`**: a declared record type; its fields are the table's columns, a
  document's fields.
- **`key`**: the fields that identify a row. Its first is the partition
  where a database partitions (Cassandra's partition key, a MongoDB
  shard key), so a read by a key's prefix is a read of one partition.
- **`order`**: the order of the rows of one key prefix or index value (a
  clustering column; an index's sort).
- **`index <name> on <fields>`**: another way to find rows; `unique` for
  one whose values are unique.
- **`refers <field> to <Table>`**: a relation; a foreign key where both
  tables are in one SQL source.
- **`bounded <n>`**: a table read whole (the home page's stores), at most
  `n` rows; a table not bounded has no read of all its rows.
- **A source `holds` tables**: `database.read<CartLines>` names
  `CartLines`, as ADR-0207's resources are named.

#### 1.2 Its reads and writes, and no others

Each table gives typed functions, the program's only way to its data:

| Function | Shape | Calls |
| --- | --- | --- |
| `CartLines.get(owner, item)` | by key: one row or none | 1 |
| `CartLines.get_all(keys)` | by keys, the set form | the source's `sets` |
| `CartLines.of(owner)` | by a key prefix: an owner's lines, in `order` | 1 |
| `CartLines.of_all(owners)` | its set form | the source's `sets` |
| `CartLines.by_store(store)`, `by_store_all(stores)` | by an index, and its set form | 1; the source's `sets` |
| `Stores.all()` | every row, only where `bounded` | 1 |
| `insert(row)`, `put(row)`, `delete(key)`, `delete_of(prefix)` | writes, in the command's transaction | 1 each |

- **Each binds to a generic host primitive, imported per table**
  (`pw:tables/cart-lines#get`, capability `database.read<CartLines>`), so
  a component's capabilities still name its tables, and one adapter
  function serves every table.
- **Every keyed read has its set form**, generated: the 4c note's `set`
  pairing is no longer written by hand, and its PW0405 has a repair for
  every read.
- **A read the declaration does not give does not exist.** A filter on a
  field no key or index leads with is the program's own `List.filter`
  over a declared read, which the budget counts (4c).

#### 1.3 What stays a host operation, and what becomes declared

- **Stays the host's**:
  - the table primitives: `get`, `get-many`, `prefix`, `prefix-many`,
    `index`, `index-many`, `all`, `insert`, `put`, `delete`,
    `delete-prefix`, one adapter per database implementing them for any
    table;
  - the transaction: a command is one, begun and committed by the
    platform with its outbox, its interaction's record and its runs
    (ADR-0208, ADR-0325, ADR-0334), in platform tables every adapter
    creates beside the program's;
  - the platform's services: the session and its principal, the clock,
    payments, uploads and blobs, identity, outbound requests
    (`network.fetch`, the recommender);
  - a source the program never writes and no database holds: kiokun's
    build output (raw DEFLATE files), a read-only host layer, said so;
  - test controls: a delay, a failure or a gate set on a table's
    primitive by the test harness, for any adapter, where `store::Faults`
    names the store's own fields today.
- **Becomes Pleris**: every `store:data/…` and `feed:data/…` operation,
  over the generated reads and writes: a cart's line merged with what the
  cart holds, the hours' periods, the checkout's parts and fees, an order
  placed with its lines and its reach, a promotion's use, the search's
  match, the timeline, a thread, a like. The seed is declared fixtures
  per table, where `0002_seed.sql` and `State::seed` hold it twice today.

#### 1.4 Migrations come from the declarations

- `pw build` writes `tables.json`: each table's fields and their types,
  key, order, indexes, references and source.
- The schema a database has is recorded in it (`pw_schema`, with its
  hash), and the last one released is committed beside the build
  (`schema.lock`).
- The compiler diffs the declarations against it. An additive change is
  generated: a new table, a new field that is an `Option` or has a
  default, a new index. A change that loses or reinterprets data (a
  rename, a type changed, a key changed, a field dropped) is refused
  unless the program states it (`migrate`: `renamed`, `dropped`,
  `backfilled with`).
- Each adapter applies the plan in its own terms, once, under its lock,
  recorded (ADR-0246's way): SQL `CREATE TABLE`, `ALTER TABLE` and `CREATE
  INDEX`, a refinement as a `CHECK` (a `PositiveInt` quantity, which
  PostgreSQL holds by hand today, ADR-0298); MongoDB's collections,
  `createIndex` and a `$jsonSchema` validator from the type; Cassandra's
  tables, a key as partition and clustering columns, an index as a table
  of its own the adapter writes beside, additive `ALTER` only.
- The host refuses to start where the database's recorded schema is not
  the build's (drift), as `held_to` refuses a shortfall (ADR-0246).

#### 1.5 One adapter per database, and its suite generated

- **Adapters**: memory, the reference; PostgreSQL first; then SQLite,
  MongoDB and Cassandra; CockroachDB through PostgreSQL's. Each maps the
  types, generates its statement for each primitive and table (prepared
  once), its set reads (`= ANY`, `json_each`, `$in`, a request a key), its
  transactions, its feed and its `provides()`.
- **The conformance suite is the adapter's**, written once for each
  primitive and run on every table shape of every program: answers equal
  to memory's on generated data; calls counted at two sizes as `sets`
  states; the guarantees and their negative controls; the anomalies;
  refused commits; delivery; idempotency (decision 5.5's seven parts). An
  app's tables get the suite without a line of Rust.
- **A relation is read by level**: each level's keys one set read, so a
  read's rounds are its data-dependency depth (Haxl's), never its rows.
  Within one SQL source an adapter may fuse a declared relation's levels
  into one join: a fixed rewrite, not an optimizer (charter §2).
- **A tree is read by a field its rows share**: the feed's thread, each
  post holding its root (`thread`) and an index on it, is one range read
  on every database, and the program assembles the tree; no recursive
  statement.
- **Precedent**: Convex declares a table's indexes in its schema ("To add
  an index onto a table, use the index method on your table's schema"),
  and a query names the index it reads (".withIndex method defines which
  index to query"); without a range it scans the table (docs.convex.dev,
  Indexes, read 2026-10-14).

#### 1.6 What proves it

- **The store and the feed, served with no Rust data layer**: their data
  declared as tables, their operations Pleris; `store.rs`'s operations,
  `store_pg.rs`, and `feed.rs`'s and `feed_pg.rs`'s data code deleted.
- **Their whole server suites and browser suites**, on memory and
  PostgreSQL first, then on SQLite, and the store across its three
  databases (decision 2), each adapter through the generated suite.
- **The same declarations on every adapter**: no app's file names a
  database; a deployment chooses each source's (decision 5.1).

### 2. The app: the store, its data in three sources

The app is the store, the DoorDash customer side (NEXT's first app): its
pages and commands as they are, served with its data in three databases.
Each slice is in the database whose guarantees it needs, and is a source
(ADR-0207): the resources it holds, and what its database gives.

| Source | Holds | Database | Why there |
| --- | --- | --- | --- |
| `Purchases` | `Stores` (rows, hours, fees, promotions, zones), `Carts`, `Orders`, `Addresses`, `Estimates` | SQLite first (owner decision 1); PostgreSQL, the reference; CockroachDB, optionally | The store's commands decide on what they read here (two also read the menu, below), and ask `transaction serializable` (ten of sixteen), `idempotent_by` and `emits` (all sixteen): one transaction that the writes, the outbox, the interaction's record and the scheduled runs commit in (ADR-0208, ADR-0325, ADR-0334). |
| `Catalog` | `Menus`: each store's sections and items, an item's name, description, price, how it is served and whether it can be ordered; the menus' search | MongoDB (owner decision 1) | Read by every reader, shared and cached. A store's menu is one document, read whole. It is written by the merchant's side, outside the program, so only the database's own change stream can tell an open page that it changed. |
| `Activity` | `OrderActivity`: an order's steps (placed, paid, preparing, picked up, delivered), appended by the kitchen's and the courier's side | Cassandra or ScyllaDB (owner decision 1) | Appended, never updated, partitioned by order, read by the order's page, where a step shown late is harmless: what a wide-column store is for, and what the store can hold there without asking more than it gives. |

- **The principle: a command's decision and its writes are in one source.**
  The commands read `Stores` (hours, fees, promotions, zones) and decide
  with it, so `Purchases` holds it, and the catalog does not. `Menus` is
  the one resource a command reads that another database holds:
  `add_to_cart` and `increase_in_cart` ask whether the item can be
  ordered (decision 4, PW0354).
- **The kitchen's and the courier's side write `OrderActivity`.** They are
  the backend's services, outside the app layer (the owner's scope,
  capabilities.md); in the tests, writes through the table's primitives
  stand in for them, as the host's `orders#set-status` stands in for the
  kitchen today (ADR-0298).
- **`Recommendations` stays a `network.fetch`** (the simulated recommender,
  ADR-0165), held by no source; the benchmark's `Notices`, `Kitchen` and
  `Categories` stay unheld, the default layer's (decision 5.1).

**What the program changes**, each in the track that needs it (decision
5):

- **Three reads that would span two sources are written in Pleris, one
  operation per source** (4c's decision 3). PW0356 refuses the search's
  as it is declared; the other two declare only what they write, and
  cannot run once the cart's database holds no menu:
  - the search: the match in `Catalog` (`menus#matching`), the stores and
    their reach in `Purchases` (`stores#by-ids`, `coverage_of_stores`);
  - a cart's line: its item's name, store and price read in `Catalog` by
    the program (`menus#items`) and written to `Purchases` with the line,
    where the layer read them from its own menus (`catalog_item`);
  - a cart's stores: each line's store, kept with the line
    (`carts#stores`), where the layer found it by reading every store's
    menu (`cart_reaches`, and `store_of` in `checkout.rs` and `hours.rs`).
- **`add_to_cart` and `increase_in_cart` state what their read admits**:
  `reads_outside Catalog`. An item sold out between the read and the
  commit is added. `place_order` does not read the menu: it prices the
  lines as they were added and refuses a total other than the one shown
  (ADR-0329), so an item that sells out after it was added reaches an
  order today, on one database; the split adds only the window between
  the read and its commit, and says so where it is written.
- **The order's page shows its activity**: `user query
  OrderActivity(reader)`, `consistency eventual`, read again on its
  `freshness` (Cassandra) or when its source tells `OrderProgressed`
  (ScyllaDB, decision 3).

**And the store declared on each other kind** (the negative matrix, each
pinned by a test, decision 6): `Purchases` as MongoDB gives it, as
Cassandra gives it, and as D1 gives it; `Catalog` as Cassandra gives it.
What each is refused, by name, is decision 4's table.

### 3. What each source declares

ADR-0207's clauses (`holds`, `transactions`, `reads`, `changes`), the 4c
note's `sets`, and one this note adds, `tells`:

```
source Purchases
    holds        Stores, Carts, Orders, Addresses, Estimates
    transactions serializable
    reads        strong
    changes      none
    sets         one_call

source Catalog
    holds        Menus
    transactions snapshot
    reads        snapshot, read_your_writes
    changes      feed
    tells        MenuChanged, InventoryChanged
    sets         one_call

source Activity
    holds        OrderActivity
    transactions none
    reads        eventual
    changes      none
    sets         per_key
```

- **`tells E, ..`** (new): the events the source's feed tells, of a write
  to what it holds by anyone: a command, the host's own route, another
  host, or a program that is not Pleris. Each names a declared `event`;
  the layer maps a change to it and its values (a menu document changed
  is `MenuChanged(store)`). A told event is delivered as an outboxed one
  is (ADR-0219), on every host that opened the feed. A clause left out
  tells nothing.
- **`sets`** (the 4c note's): `one_call`, `chunked(k)` or `per_key`, what a
  set read costs. A clause left out is `per_key`.
- **`transactions partition`** (proposed for track T4b, decision 6): a
  command whose writes are to one partition commits them, its
  interaction's record and its events atomically and in isolation, by one
  conditional batch (Cassandra's lightweight transactions). A table's
  key's first field is its partition (decision 1.1), so the checker reads
  the partitions a command writes from the keys it passes. Nothing across
  partitions.
- **What each layer measures** before the host serves (ADR-0246's
  `provides()`), in each database's own terms:

| Database | `transactions` | `reads` | `changes` | `sets` | Measured by |
| --- | --- | --- | --- | --- | --- |
| PostgreSQL (the reference) | `serializable` (set on each) | `strong` on the primary | `none` (the outbox; decision 5.3) | `one_call` (`= ANY($1)`) | `transaction_isolation`, `transaction_read_only` (ADR-0246) |
| SQLite | `serializable` (`BEGIN IMMEDIATE`) | `strong` | `none` | `one_call` (a list as one JSON parameter, `json_each`) | `PRAGMA journal_mode`; a command's transaction begun `IMMEDIATE` |
| D1 | `read_committed` at most: auto-commit, writes atomic in one `batch()`, reads before it | `strong` on the primary; `read_your_writes` in a session, with replicas | `none` | `chunked(100)` (bound parameters), or `one_call` by one JSON parameter | not built (owner decision 1) |
| MongoDB, a replica set | `snapshot` (multi-document transactions) | `snapshot`, `read_your_writes` (a causally consistent session, majority) | `feed` (change streams) | `one_call` (`$in` in one `find`, its batch size set to the keys, within 16 MiB) | `hello`'s `setName`: a standalone server gives `transactions none` and no feed |
| Cassandra | `none` | `eventual` at `ONE`; `read_your_writes` only where reads and writes overlap (W + R > RF) | `none` (its CDC is files on each node) | `per_key` | the keyspace's replication factor and the layer's consistency levels |
| ScyllaDB | `none` | as Cassandra | `feed` where a table has CDC | `per_key` | as Cassandra, and the table's `cdc` option |
| CockroachDB | `serializable` | `strong` | `feed` (a changefeed) | `one_call` (`= ANY($1)`) | the PostgreSQL layer's measurement, and `kv.rangefeed.enabled` for the feed |

### 4. What the compiler refuses and warns, per source

**Already refused** (ADR-0207), and where each meets this app:

| Code | Rule | Meets |
| --- | --- | --- |
| PW0344 | a query asks no more `consistency` than its source's `reads` | `Purchases` as Cassandra gives: the nine `read_your_writes` queries reading it (`Cart`, `Order`, `Checkout`, `OrderTotal`, `CartHours`, `SavedAddresses`, `Reach`, `Estimate`, `Search`) and the five `snapshot` ones (`Store`, `StoreList`, `StoreStatus`, `StoresOpen`, `Places`). `Catalog` as Cassandra gives: `Menu`. |
| PW0345 | a command's writes are one transaction, in one source | a command writing the cart and the order's activity; the repair is a command it leads to (decision 5.4) |
| PW0346 | a command's `transaction` is at most its source's `transactions` | the ten serializable commands, by name, wherever `Purchases` is not serializable: MongoDB (`snapshot`), D1 (`read_committed`), Cassandra (`none`) |
| PW0347 | a command's events commit with its writes, or a feed tells them | every command of `Purchases` as Cassandra gives |
| PW0348 | a command's interaction's record commits with its writes | every command of `Purchases` as Cassandra gives (all sixteen are `idempotent_by`) |
| PW0349 | a source holds what the program's effects name, once | revision 2: `sets` and `tells` take only their domain's words, and `tells` names declared events |

The decisive one, on Cassandra (NEXT): the cart's commands refused by
name, with ADR-0207's own words.

```
error[PW0346]: `place_order` asks `transaction serializable` of `Purchases`, whose transactions are `none`
  --> examples/store/app.pw:426:1
   |
426 | command place_order(shown: Money<USD>)
   | ^^^^^^^^^^^^^^^^^^^ asks `transaction serializable` (line 431)
   |
   = note: `Purchases` states what it guarantees here (examples/lib/Purchases.pw)
   = help: ask what `Purchases` gives
```

**Proposed** (each a revision or a code for the integrator to number;
PW0354 to PW0399 are free on `master` and on every track's branch, read
2026-10-14):

| Code | Rule | Meets in this app |
| --- | --- | --- |
| PW0347, revision 2 | a command's events commit with its writes, or its source's feed `tells` each of them: a feed that tells `MenuChanged` tells no `UserCartChanged` | today any feed is accepted for any event |
| PW0354 `read_outside_transaction` | a command that states a `transaction` reads only its writes' source, or states `reads_outside` each other source it reads: no transaction spans two databases | `add_to_cart` and `increase_in_cart` (`Menus` in `Catalog`), until they state `reads_outside Catalog` |
| PW0355 `snapshot_across_sources` | a query that asks `snapshot` or `strong` reads one source: no one snapshot spans two databases | none of the store's: each `snapshot` query reads one source. `Search` reads two and asks `read_your_writes`, which each source gives for its own writes, and is not refused |
| PW0356 `operation_spans_sources` | a host function's effect row names one source's resources: one layer answers an operation | `search#find`, `search#find-for` (`Stores` and `Menus`) as written today |
| PW0357 `told_without_feed` | a source that `tells` events states `changes feed` | the order's activity told by `OrderProgressed` on Cassandra: the change-feed-dependent live query on a source with no change feed |
| PW0358 `schedule_without_commit` | a command that schedules or unschedules writes to a source with transactions (ADR-0334: a run commits with its scheduler's writes) | `place_order` (`schedule expire_order`), `settle_payment` and `cancel_order` (`unschedule`) as Cassandra gives `Purchases` |
| PW0359 `writes_two_partitions` (T4b) | a command's writes to a `transactions partition` source name one partition | a command appending to two orders' activity |
| PW0360 `reads_outside_unread` | a `reads_outside` names a source the command reads and does not write (a clause nothing reads is refused, ADR-0243's lesson) | `reads_outside Catalog` on a command that reads no menu |
| PW0361 `table_malformed` (D1) | a table's `row` is a record type; its key's, order's and indexes' fields are the row's, of types every adapter keys (no list, no function); `refers` names a table whose key the field holds; `bounded` a positive count | a key over a `List`; a reference to a table keyed otherwise |
| PW0362 `data_not_declared` (D1) | where a source holds tables, its data is reached only through their generated functions: a host binding performing `database.read` or `database.write` of a table is refused | a `store:data/…` binding left over a table the store declared |
| PW0363 `index_read_apart` (T4) | a read by an index its source writes apart from the row (a CQL index table, written beside in another partition) gives `eventual`, whatever the source's `reads` | a `read_your_writes` query reading the order's activity by an index on Cassandra |
| PW0364 `all_unbounded` (D1) | `all()` only of a `bounded` table | the stores read whole without `bounded` |
| PW0365 `refers_across_sources` (D1), a warning | a reference to a table of another source is held by no database: the referred row may be missing, which `get` answers as `None` | a cart's line referring to `Catalog`'s menu items |
| PW0366 `migration_unstated` (D1) | a change to a table that loses or reinterprets data is planned only with its `migrate` | a field renamed, or its type changed, between two releases |

And the 4c note's PW0405 (a read per loop element, refused) and PW0406 (a
budget that grows on a `per_key` source, a warning) apply per source: on
`Activity`, a page listing each of a reader's orders with its activity
would be warned (`sets per_key`); on `Purchases` and `Catalog`
(`one_call`) not.

The two new refusals, as a reader meets them:

```
error[PW0354]: `add_to_cart` reads `Menus` in `Catalog`, outside its transaction in `Purchases`
  --> examples/store/app.pw:350:9
   |
350 |     if !Menus.is_available(item.id) {
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^ read in `Catalog`, which commits apart
   |
   = note: `add_to_cart` asks `transaction serializable` of `Purchases`, where it writes;
           no transaction spans two databases, so what it read may have changed by its commit
   = help: hold `Menus` in `Purchases`, or state what the read admits: `reads_outside Catalog`

error[PW0357]: `Activity` tells `OrderProgressed`, and states `changes none`
  --> examples/lib/Activity.pw:14:5
   |
14 |     tells        OrderProgressed
   |                  ^^^^^^^^^^^^^^^ no feed carries it
   |
   = note: `OrderActivity` is read again on `OrderProgressed(reader)` (examples/store/app.pw);
           with no feed of `Activity`'s changes nothing tells it, and an open page keeps the activity it first read
   = help: read `OrderActivity` again on its `freshness`, or hold it in a source with `changes feed`
```

### 5. The adapter seam

#### 5.1 Several layers in one host

- **Today one layer serves a program**: `from_build_layers` picks it by
  the contracts' import prefix (`feed:`, `kiokun:`, else the store's), and
  `held_to` compares every source with that layer's `provides()`. A
  program whose sources are in two databases cannot be served.
- **`Sources`** (`server/src/sources.rs`), a `DataLayer` over several. A
  layer is a source's adapter bound to its database (decision 1.5), or,
  until a program declares its data, a hand-written layer as today:
  - **The deployment names a database for each source**, by a variable of
    its name: `PW_SOURCE_CATALOG=mongodb://127.0.0.1:27017/?replicaSet=rs0`.
    The scheme picks the adapter: `sqlite:` a file; `postgresql:` PostgreSQL
    or CockroachDB; `mongodb:`; `cql:` Cassandra or ScyllaDB; each pair
    told apart by the server's own report of what it is. A source no
    variable names is in memory, as every source is today.
    `PW_FEED_DATABASE_URL` and `PW_STORE_DATABASE_URL` keep naming
    `FeedData`'s and `StoreData`'s, and the store's is the default layer.
  - **An operation goes to the layer of the resource its import names.**
    The contract's import already carries its capability
    (`database.read<Menus>`), so `Menus` names `Catalog` and `Catalog` its
    layer. A resource no source holds, and an operation that names none
    (the recommender's `network.fetch`), is the default layer's (ADR-0207:
    "the host's own database's"). An operation two layers supply, or none,
    refuses the host's start (ADR-0218).
  - **A command is staged on the layer of the source it writes**, one by
    PW0345. What it reads of another source, which it states
    (`reads_outside`), is that layer's read, outside the transaction. A
    command that writes nothing is staged on the default layer.
  - **Each layer is held to the sources it holds**: `held_to` per layer,
    a shortfall naming the source, the clause and what the database gives
    (ADR-0246).
  - **The platform's records stay with the writes**: an interaction's
    record (ADR-0325), the outbox (ADR-0208) and a scheduled run
    (ADR-0334) commit in the transaction of the layer the command writes.
- **The trait gains three methods**, each defaulted, so no layer changes
  before it needs to:
  - `feed()`: the committed changes its database tells, as the events of
    its sources' `tells` with their values, from a position the host
    keeps (decision 5.3);
  - `tells()`: the events its feed can tell. A source whose `tells` names
    one its layer cannot refuses the host's start;
  - `calls()`: the round trips it has made, a test's (4b's `Tally`,
    generalized: statements, commands or requests, decision 5.5).

#### 5.2 What each adapter implements

- **Every primitive, for any table `tables.json` declares**, in its
  database's own terms:

| Adapter | Types | `get`, `prefix`, `index` | Set form | A command's transaction | Feed | `provides()` |
| --- | --- | --- | --- | --- | --- | --- |
| memory, the reference | values as they are | maps by key and by index, under a lock | a lookup a key | the lock; rows staged, published at commit (ADR-0218) | none | the host database's (ADR-0207) |
| PostgreSQL | `BIGINT`, `TEXT`, `BOOLEAN`; `JSONB` for a case or a nested record; `NULL` for `Option`; `CHECK` for a refinement | a prepared `SELECT` by the key's or the index's columns, `ORDER BY` the order | `= ANY($1)`, one statement | `SERIALIZABLE`, set on each | none (a logical slot read by SQL, later) | ADR-0246's |
| SQLite | `INTEGER`, `TEXT`, JSON text; `CHECK` | as PostgreSQL's, with `?` placeholders | one JSON list, read by `json_each` | `BEGIN IMMEDIATE` | none | `PRAGMA journal_mode`; a read-only file refused |
| MongoDB | a document a row, its `_id` the key's fields; a `$jsonSchema` validator | `find` by the key's or the index's fields, sorted by the order | `$in`, one `find` whose batch size covers the keys | a multi-document transaction, `majority` read and write | change streams, as `tells` | `hello`'s `setName` |
| Cassandra or ScyllaDB | CQL's types; the key's first field the partition key, the rest clustering columns | a `SELECT` of one partition, by a clustering prefix; an index, a table the adapter writes beside | a request a key, at once | none; with T4b, a partition's conditional batch | none (Cassandra); the CDC log (ScyllaDB) | the keyspace's replication factor; the consistency levels |

- **The platform's tables**, which every adapter with transactions
  creates beside the program's: the outbox (ADR-0208), the interactions
  (ADR-0325), the scheduled runs (ADR-0334) and the applied schema
  (`pw_schema`). A source with no transactions has none: the checker has
  refused every command that would need them (PW0346 to PW0348, PW0358).
- **Its round trips counted** (`calls()`): SQL statements, MongoDB
  commands, CQL requests.
- **Until a program declares its data**, a hand-written layer serves its
  imports by name (`store:data/…`) beside the adapters (`pw:tables/…`),
  and `Sources` routes both; a layer is deleted when its program
  declares the last of what it held (decision 6).

#### 5.3 Feeds: a change told to every host

- **A layer with a feed opens it before the host serves**, from the
  position the host kept for that source (MongoDB's resume token; a
  CockroachDB changefeed's cursor; ScyllaDB's CDC stream position). The
  host keeps it in its materializer, per source, written after each
  change is delivered.
- **A told event reaches every session that reads an entry it drops**
  (ADR-0219), on every host that opened the feed. The committing host
  delivers its own outbox's events at once, as today; a second delivery
  of the same change from the feed drops an entry already dropped, a read
  at most, never a wrong value.
- **A position the database no longer holds** (MongoDB's oplog rolled
  past the token) drops every entry of every query reading the source,
  once, and the host says so in its log: a full read again, never a
  missed change.
- **So a source with a feed is the first piece of a second host**
  (capabilities.md, "A second host"): each host hears every commit of
  `Catalog`, its own or another's. `Purchases` on SQLite or PostgreSQL
  keeps the outbox's limit (ADR-0246): a second host hears nothing of the
  first's commits, until its layer reads one (Not claimed).

#### 5.4 Across sources: a command it leads to

- **PW0345's repair is "write one source in a command, and the other in a
  command it leads to"**, and ADR-0334 built the parts: the first command
  `schedule`s the second; the scheduling commits in the first's
  transaction, which makes it the outbox; the host runs the second once it
  is due, in the second's source, on behalf of the same reader.
- **A run can commit and its scheduling's removal fail**, since they are
  in two databases, so a run may run again. Its record, `idempotent_by`
  its scheduling, answers it in its own source (ADR-0325), which PW0348
  holds to a transaction, or, with T4b, to a partition transaction whose
  partition the record is written in.
- **In this app**: the order's first step ("placed") could be appended by
  a command `place_order` schedules. On Cassandra that command is refused
  (PW0348) until T4b; until then the backend's host operation appends
  every step, which is the owner's scope anyway.

#### 5.5 The conformance suite, every layer

4b's method, each operation counted at two data sizes and pinned,
generalized into a suite each layer runs (`server/src/tests/conformance/`,
over a layer it is handed), in seven parts:

1. **Answers**: every operation of the slice answers what the in-memory
   layer answers, before and after the same writes, and the store's pages
   are served to the byte (ADR-0298's parity, generalized).
2. **Guarantees**: `provides()` against the database opened, and each
   clause's negative control, a deployment that gives less refused at
   start: a database opened read-only for `transactions`; a standalone
   MongoDB for `changes feed` and `transactions snapshot`; Cassandra at
   `ONE` for `read_your_writes`; CockroachDB with `kv.rangefeed.enabled`
   off for `changes feed`.
3. **Anomalies**: each refusal's anomaly shown on a database that admits
   it, and absent on one that does not, so a refusal cites a measured
   failure as well as a document: write skew under MongoDB's snapshot
   transactions (two commands, each reading what the other writes),
   absent under SQLite's and PostgreSQL's serializable; a lost update
   under Cassandra at `ONE` (two read-modify-writes of one row); a stale
   read from a D1-like replica is not built (owner decision 1).
4. **Costs**: each operation's round trips at two data sizes, as its
   source's `sets` states: SQL statements (4b's `Counted`), MongoDB
   commands (the driver's `command_event_handler`, mongodb 3.9.1), CQL
   requests (the driver's `observability::history`, scylla 1.9.0).
5. **Commits**: a commit the database refuses keeps nothing, writes,
   events, record or runs, each refused its own database's way:
   PostgreSQL's deferred constraint trigger (ADR-0246), SQLite's
   `RAISE(ABORT)` trigger, MongoDB's write conflict, Cassandra's condition
   not applied (T4b).
6. **Delivery**: an outboxed event delivered if and only if its commit; a
   told event delivered for a write made outside the host (the database's
   own client), to a second host as well, and after a restart from the
   kept position.
7. **Idempotency**: a retried interaction answered once, on one host and
   across two (ADR-0325's tests, per layer).

#### 5.6 Each database, here and on CI

- **Here**: the PC's WSL2, Ubuntu 22.04.3, 32 cores and 31 GiB, with no
  Docker; PostgreSQL 18.6 from PostgreSQL's apt repository (track
  `pc-setup`'s record). No Java, `mongod`, Cassandra, ScyllaDB or
  CockroachDB is installed (checked 2026-10-14). The rule for each new
  server:
  - a vendor's tarball, its checksum checked against the vendor's own
    (`.sha256`, `.sha512` and `.asc`, `.sha256sum`, each published beside
    its tarball, checked 2026-10-14), unpacked under
    `~/.local/pw-db/<name>-<version>/` and recorded in
    `tools/versions.lock`;
  - no system service, nothing run as root but a Java runtime's install;
  - each recipe starts its server on ports in the worker's range, its data
    under `/tmp/<track>-<db>/`, and stops it on the way out, whatever
    happened, as a test's schema is dropped (ADR-0246).
- **Each database:**
  - SQLite: in the test's process; nothing.
  - MongoDB 9.0.4: three `mongod` members on the one machine, as MongoDB's
    testing tutorial runs them (`--replSet rs0 --bind_ip 127.0.0.1 --port
    P --dbpath D --oplogSize 128`), initiated by the layer's test helper
    (`replSetInitiate`): the manual's "multiple node replica set".
    Whether one member behaves alike is something the suite records, not
    assumes.
  - Cassandra 5.0.9: its tarball and Ubuntu's `openjdk-17-jre-headless`
    (owner decision 3), one node, its native, storage and JMX ports moved
    into the worker's range, its heap bounded.
  - ScyllaDB 2026.3.3, if chosen: its apt repository and packages, run by
    hand in developer mode; WSL2 is not in its documentation, so the
    track's first step shows it starts here or stops.
  - CockroachDB v26.3.2, or the v26.2.7 long-term release: the binary,
    `cockroach start-single-node --insecure --listen-addr=127.0.0.1:P
    --http-addr=127.0.0.1:Q --store=<dir>`; a single node needs no key.
- **Ports**, within a worker's 200 (docs/PARALLEL.md): MongoDB at
  `PORT+150..152`, Cassandra's native, storage and JMX at
  `PORT+160..162`, CockroachDB's SQL and HTTP at `PORT+170..171`: the
  integrator's to assign, as the browser hosts' ranges are.
- **On CI**, verify's database shards (ADR-0278): `NEEDS_DATABASE` names
  the kind each recipe needs (`postgresql`, `mongodb`, `cassandra`,
  `cockroachdb`), and its shard starts that kind:
  - PostgreSQL as today;
  - MongoDB by the same recipe and tarball as here, three members on the
    runner. A `mongo:9.0.4` service can be started `--replSet rs0`, but it
    is one member; three services on a job's network know each other by
    names the runner does not resolve without more setup, and a driver
    follows those names (owner decision 4);
  - `cassandra:5.0.9` as a service, healthy once `cqlsh` answers;
  - `cockroachdb/cockroach:v26.3.2` as a service whose `command` is
    `start-single-node --insecure`.

  Each image's tag is pinned and its digest recorded.

#### 5.7 What to approve

For the recommended choices (owner decision 1), each read on 2026-10-14:

| Item | Version | Licence | Size | From | For |
| --- | --- | --- | --- | --- | --- |
| `rusqlite` (crate) | 0.40.1, in `Cargo.lock` (0.40.2 the newest) | MIT; SQLite public domain | 177,923 bytes; `libsqlite3-sys` 0.38.1 bundles SQLite 3.53.2 | crates.io | T2: approved with ADR-0019 |
| `postgres` (crate) | 0.19.14, in `Cargo.lock` | MIT OR Apache-2.0 | | crates.io | T5: approved 2026-10-07 (ADR-0246) |
| MongoDB Community Server | 9.0.4 (2026-10-13) | SSPL v1 | 152,030,619 bytes | fastdl.mongodb.org, `mongodb-linux-x86_64-ubuntu2204-9.0.4.tgz` and its `.sha256` | T3, T6 |
| `mongodb` (crate) | 3.9.1 (2026-09-10), feature `sync` | Apache-2.0 | 415,243 bytes; rust-version 1.88 | crates.io | T3 |
| Apache Cassandra | 5.0.9 (2026-08-07) | Apache-2.0 | 73,341,539 bytes | downloads.apache.org, `apache-cassandra-5.0.9-bin.tar.gz` with its `.sha512` and `.asc` | T4, if Cassandra |
| OpenJDK 17's runtime | 17.0.20.1 (`openjdk-17-jre-headless`, Ubuntu 22.04) | GPLv2 with the Classpath Exception | 48,415,616 bytes; 188,784 KiB installed | Ubuntu's universe repository, by apt (root) | T4, if Cassandra |
| ScyllaDB | 2026.3.3 (2026-10-07) | ScyllaDB Software License Agreement 1.1, source-available | not measured (apt packages) | downloads.scylladb.com's apt repository | T4, if ScyllaDB |
| `scylla` (crate) | 1.9.0 (2026-09-15) | MIT OR Apache-2.0 | 622,427 bytes; rust-version 1.88 | crates.io | T4, either server |
| `scylla-cdc` (crate) | 0.7.0 (2026-08-17) | Apache-2.0 | 44,670 bytes | crates.io | T4, ScyllaDB's feed only |
| CockroachDB | v26.3.2 (2026-09-18), or v26.2.7 | CockroachDB Software License | 149,960,594 bytes (v26.2.7: 145,543,907) | binaries.cockroachdb.com, with its `.sha256sum` | T5, optional |
| CI images | `mongo:9.0.4`, `cassandra:5.0.9`, `scylladb/scylla:2026.3.3`, `cockroachdb/cockroach:v26.3.2` | as their servers | | Docker Hub | the CI shards |

Only for the choices not recommended: `libsql` 0.10.0 (MIT, 121,697 bytes,
published 2026-10-13) for libSQL; `wrangler` 4.151.0 (MIT OR Apache-2.0,
15,722,951 bytes unpacked, Node 22 or later) for D1; `firebase-tools`
15.34.0 (MIT, 6,109,659 bytes unpacked, its emulator's archives and Java
21 apart) and the `firestore` crate 0.58.0 (Apache-2.0, 316,623 bytes)
for Firestore. Each crate's own dependencies are counted when it is
added, as ADR-0246 counted `postgres`'s 47, and held by `cargo deny`
(licences, bans, sources, advisories). The servers are no crates, and
`deny.toml` does not see them: MongoDB's SSPL, ScyllaDB's licence (which
ScyllaDB calls source-available) and the CockroachDB Software License are
the owner's to weigh (owner decisions 1 and 5). None of them is linked
into Pleris; each runs beside it.

### 6. The build, as tracks

Each track is a worktree and a branch of its own, its ADR unnumbered, its
codes in the block the integrator gives it, its recipe in
`just/<track>.just` writing `docs/evidence/E14/<name>.txt`, and its
mutation script in `scripts/<track>_mutations.py`, every mutant a
well-typed program and killed by a test. The migration path, from
today's hand-written layers to none, in order of what each needs:

| Track | What it does | Needs | Downloads |
| --- | --- | --- | --- |
| D1 `tables` | the `table` declaration, its checks, its generated functions, `tables.json`, the schema's diff | this note ruled | none |
| D2 `memory-adapter` | the table primitives (`pw:tables/…`), the host's routing, the in-memory adapter, the generated suite; TodoMVC with no Rust data layer | D1 | none |
| D3 `postgres-adapter` | the reference adapter: migrations, statements, set reads, platform tables, `provides()` | D2 | none: `postgres` is in `Cargo.lock` |
| D4 `feed-declared` | the feed's data declared, `feed.rs`'s and `feed_pg.rs`'s data code deleted | D3 | none |
| D5 `store-declared` (one track a composite) | the store's data declared, its operations Pleris, `store.rs`'s operations and `store_pg.rs` deleted | D3; 4c's composites | none |
| T1 `sources` | several layers in one host, and the rules across sources | D2 | none |
| T2 `sqlite-adapter` | the SQLite adapter; the feed and the store on it | D3 | none: `rusqlite` is in `Cargo.lock` |
| T3 `mongodb-adapter` | the MongoDB adapter, its change streams; the store's `Catalog` | T1, D5 | the MongoDB server and the `mongodb` crate (owner decision 2) |
| T4 `cql-adapter` | the Cassandra or ScyllaDB adapter; the store's `Activity`; the decisive refusals | T1, D5 | Cassandra and a Java runtime, or ScyllaDB; the `scylla` crate |
| T4b `partition-transactions` | lightweight transactions as `transactions partition` | T4 | none beyond T4's |
| T5 `cockroachdb` (optional) | the PostgreSQL adapter against CockroachDB | D3 | the CockroachDB binary |
| T6 `three-databases` | the store on its three databases and the feed on two, no Rust data layer | T2, T3, T4, D4 | none beyond theirs |

**D1 `tables`: the declaration.**
- Files: `compiler/pw-syntax/src/grammar.rs` (`table`), `compiler/pw-core/
  src/policy.rs` (its clauses' domains), a new `compiler/pw-core/src/
  tables.rs` (the checks and the generated functions, typed, with their
  effects), `codes.rs`, the build's `tables.json` and `schema.lock`,
  `compiler/pw-core/tests/tables.rs`.
- Codes: decision 4's table refusals.
- Recipe: `just/tables.just`, `e14-tables`.
- Acceptance: each clause's words; each refusal and its control; the
  generated functions' types and effects; `tables.json` for a fixture; a
  schema's diff: additive changes planned, a rename or a dropped field
  refused without its `migrate`.
- Mutation controls: a key field not checked; an index's fields not
  checked; a set form not generated; a destructive change planned
  silently.

**D2 `memory-adapter`: the primitives, and the first app with no Rust
data layer.**
- Files: `runtime/pw-host` (the `pw:tables/…` imports, bound per table),
  a new `server/src/adapters/memory.rs`, `server/src/adapters/suite.rs`
  (the generated conformance suite, decision 5.5), `main.rs` (a program's
  tables served by their source's adapter, at lines marked `TRACK SEAM
  (tables)`); TodoMVC's program (track `todomvc`'s) declaring its todos.
- Acceptance: TodoMVC's behaviours, every one its track tests, with its
  layer's Rust deleted; the suite's seven parts on memory; a component
  linked only to the tables its capabilities name.
- Mutation controls: a primitive answering another table's rows; a set
  read answering keys in another order; an index not kept on a write; a
  staged write visible before its commit.

**D3 `postgres-adapter`: the reference.**
- Files: `server/src/adapters/postgres.rs`; the platform's tables as one
  migration of the adapter's; the schema applied from `tables.json` under
  the advisory lock (ADR-0246's way); `provides()` as ADR-0246's.
- Recipe: `e14-postgres-adapter`, in `NEEDS_DATABASE`.
- Acceptance: the suite's seven parts against PostgreSQL 18.6, every
  primitive's statements counted at two sizes (one statement each, its
  set form `= ANY`); a schema drifted under the host refused at start; a
  refinement held by its `CHECK`; TodoMVC on PostgreSQL.
- Mutation controls: a set read looped per key; a transaction at the
  connection's default; a migration applied twice; drift accepted.

**D4 `feed-declared`: the feed with no Rust data layer.**
- The feed's users, posts, likes, follows, notifications, conversations
  and messages declared as tables in `FeedData`; a post holding its
  thread's root, indexed, so a thread is one range read; its operations
  (`posts#publish`, `#timeline`, `#thread`, the likes, the follows'
  timeline, the notifications, the messages) Pleris over them.
- Acceptance: the feed's server tests and browser suite in three engines,
  on memory and on PostgreSQL, unchanged in what they assert;
  `feed.rs`'s and `feed_pg.rs`'s data code deleted; the 4b-style counts
  of every page and command pinned on PostgreSQL.
- Mutation controls: the feed's existing scripts, re-anchored in Pleris
  (ADR-0281's rule: each run whole).

**D5 `store-declared`: the store with no Rust data layer, a composite at
a time.**
- One track for each of the 4c note's composites and the store's
  remaining operations, in its order (the search, the hours, the
  checkout, the cart, the order and its payment, the addresses, the
  estimates), each declaring the tables it needs, rewriting the
  operation in Pleris, deleting it from `store.rs`, and measuring it
  with 4b's pins before and after; the test controls moved onto the
  primitives (decision 1.3).
- Acceptance: the whole server suite and the browser suite, on memory
  and PostgreSQL, after each; at the last, `store.rs`'s operations and
  `store_pg.rs` deleted.
- Mutation controls: the store's thirteen scripts whose anchors are in
  `store.rs` (ADR-0298), re-anchored in Pleris as each operation moves.

**T1 `sources`: several layers in one host, and the rules across sources.**
- Files: `compiler/pw-core/src/check.rs` (`sources_give_what_is_asked`),
  `policy.rs` (`tells`, `reads_outside`; `sets` if 4c has not landed it),
  `codes.rs`, `compiler/pw-core/tests/sources_across.rs`;
  `spikes/own-renderer/server/src/sources.rs` (new), `data.rs` (`feed`,
  `tells`, `calls`, each defaulted; `held_to` per layer), `main.rs`
  (`from_build_layers`, at lines marked `TRACK SEAM (sources)`),
  `server/src/tests/sources.rs`.
- Codes: PW0347 revision 2, PW0354, PW0355, PW0356, PW0360.
- Recipe: `just/sources.just`, `e14-sources`.
- Acceptance: each rule refused and its control accepted; the store, the
  feed and kiokun check unchanged (the test confirms that no program reads
  across sources today, as 4c confirmed no read per loop element); a
  program of two sources served by two in-memory layers answers as one
  layer does, to the byte; a command staged on its write source's layer,
  and its `reads_outside` read served by the other's; a layer short of
  its source's clause refuses the start naming that source alone; an
  operation two layers supply refused; a source no variable names served
  in memory.
- Mutation controls (`scripts/sources_mutations.py`): an operation routed
  by its prefix, not its resource; a command staged on the default layer;
  `held_to` asked of the default layer only; each new rule's check
  removed; PW0347 accepting any event from any feed again.

**T2 `sqlite-adapter`: the SQLite adapter, and the feed and the store on
it.** The host's materializer has been SQLite since ADR-0019, and the
apps' data in memory, then on PostgreSQL; this puts their data on SQLite
behind the same seam (ADR-0005: "the same generated interface").
- Files: `server/src/adapters/sqlite.rs`; `main.rs` (`sqlite:<file>` for a
  source).
- Codes: none.
- Recipe: `e14-sqlite-adapter`; not in `NEEDS_DATABASE`, since SQLite runs
  in the test's process.
- Acceptance: the suite's seven parts on SQLite; the feed's and the
  store's server suites with their sources on SQLite; `provides()`
  measured (WAL; a command begun `IMMEDIATE`; a file opened read-only
  refused); a commit refused by a `RAISE(ABORT)` trigger keeps nothing;
  interactions and scheduled runs kept across a restart of the host (the
  file outlives it); two hosts on one file write one at a time,
  `SQLITE_BUSY` waited out, no write lost; every primitive's statements
  at two sizes, a set read one statement by one JSON list.
- Mutation controls (`scripts/sqlite_adapter_mutations.py`): a command
  begun `DEFERRED`; events committed outside the command's transaction; a
  refused commit answered committed; a set read looped per key.

**T3 `mongodb-adapter`: the MongoDB adapter, and the menus in it, told by
its change stream.**
- Files: `server/src/adapters/mongodb.rs` (and its change streams, as a
  feed), `examples/lib/Catalog.pw` (the source), `examples/lib/StoreData.pw`
  (without `Menus`), `examples/lib/Menus.pw` and `UserCarts.pw` (a line
  written with its store and price), `examples/store/app.pw`
  (`reads_outside Catalog` on `add_to_cart` and `increase_in_cart`), the
  workspace's `Cargo.toml` (`mongodb`, with `sync`).
- Codes: PW0357; PW0349 revision 2 (`tells` names declared events).
- Recipe: `e14-mongodb-adapter`, in `NEEDS_DATABASE`.
- Acceptance: the suite's seven parts on MongoDB; a store's menu one
  document, served to the byte as in
  memory; `Menu` one command (`find` by `_id`); a change written with the
  driver outside the host reaches an open store page in three engines, and
  a second host's; a standalone `mongod` refused at start; the negative
  matrix's MongoDB row (the ten serializable commands refused by name
  where `Purchases` is declared as MongoDB gives it); write skew shown
  under snapshot (5.5, part 3); `items(ids)` one command at two sizes; a
  restarted host resumes from its token; a lost token drops every menu
  entry once.
- Mutation controls: the change stream opened after the first read; a told
  event delivered from a read of `local` concern, before a majority has
  it; the token not kept; a standalone accepted; `$in` looped per id;
  PW0354 not asked of `add_to_cart`.

**T4 `cql-adapter`: the Cassandra or ScyllaDB adapter, an order's activity
in it, and the decisive refusals.**
- Files: `server/src/adapters/cql.rs`, `examples/lib/Activity.pw` (the
  source and its `OrderActivity` table), `examples/store/app.pw`
  (`OrderActivity`, the order page's section), an e2e spec, `Cargo.toml`
  (`scylla`).
- Codes: none new.
- Recipe: `e14-cql-adapter`, in `NEEDS_DATABASE`.
- Acceptance:
  - the suite's seven parts on Cassandra, its set reads a request a key;
  - the order's steps appended by a test's writes through the table's
    primitives (the backend's stand-in), read `eventual`, shown in three
    engines;
  - `provides()` from the keyspace's replication factor, read from
    `system_schema.keyspaces` (what the keyspace is declared to give, not
    the nodes running), and the adapter's consistency levels;
  - the negative matrix's decisive row: `Purchases` declared as Cassandra
    gives it, the store refused by name, every diagnostic pinned as
    ADR-0207 pinned its 21 (decision 4: PW0344, PW0346, PW0347, PW0348,
    PW0358); `Catalog` declared so, `Menu` refused (PW0344);
  - a lost update at `ONE`, two read-modify-writes of one row, shown on
    one node: the anomaly PW0346 refuses;
  - `tells OrderProgressed` refused on Cassandra (PW0357), and on ScyllaDB
    with CDC accepted, its event delivered from the CDC log;
  - `sets per_key` counted, a request a key, and 4c's PW0406 warning a page
    that lists each of a reader's orders with its activity.
- Mutation controls: the consistency levels not read; the replication
  factor not read; a feed claimed on Cassandra; a set read silently made
  one `IN` request; the steps' order not kept.

**T4b `partition-transactions`: lightweight transactions stand in for
transactions, one partition at a time.**
- Files: `check.rs` (PW0359, the partitions a command writes read from
  the keys it passes), `policy.rs` (`transactions partition`),
  `adapters/cql.rs` (one conditional batch: the writes, the interaction's
  record and the outbox in the partition),
  the host's run path (a run's record in its own source, its scheduling
  removed in its scheduler's after).
- Codes: PW0359.
- Acceptance: a command appending one order's step, `idempotent_by`,
  commits its write and its record in one conditional batch and is
  answered once when retried, across two hosts; a command naming two
  partitions refused; the "placed" step appended by a run `place_order`
  scheduled, once, even when its scheduling's removal fails after the run
  committed.
- Mutation controls: the record written outside the batch; an
  unconditional insert; two partitions accepted; the scheduling removed
  before the run commits.

**T5 `cockroachdb` (optional): the PostgreSQL adapter against
CockroachDB.**
- Files: `adapters/postgres.rs` where CockroachDB differs, found by
  running its migrations and suite against it first and recorded (Not
  claimed: which of `xid8`, `pg_current_xact_id()` and `hashtext()` it
  has); the commit path, where a retry error (SQLSTATE `40001`) is a
  refused commit, answered so and never retried (ADR-0246's alternative).
- Codes: none.
- Recipe: `e14-cockroachdb`, in `NEEDS_DATABASE`.
- Acceptance: the suite's seven parts and the feed's and the store's
  server suites on a single node; `provides()` serializable; two hosts'
  conflicting commits, one refused with `40001`, keeping nothing;
  optionally a sinkless changefeed as `changes feed` (at least once,
  ordered per key; the host drops a duplicate).
- Mutation controls: as the PostgreSQL adapter's, and a `40001` answered
  committed.

**T6 `three-databases`: the app at once, with no Rust data layer.**
- `Purchases` on SQLite, `Catalog` on MongoDB, `Activity` on Cassandra or
  ScyllaDB, one host; the feed on PostgreSQL and on SQLite; the store's
  and the feed's browser suites in three engines against them; two hosts
  sharing `Catalog`'s feed; the negative matrix recorded beside it.
  Files: `just/three-databases.just`, a Playwright project serving the
  three, an e2e spec of what only the three show (a menu changed outside
  the host reaching both hosts' pages).
- Its evidence moves capabilities.md's "Database independence" row
  (the integrator's file).

## Alternatives

- **Keep a hand-written layer per app, one per database.** Every app
  would need Rust, and every new database each app's again: the store
  alone has 63 operations (4b), written twice today.
- **Generate each app's Rust layer from its declarations.** Generated
  code per app is still code per app to build, review and mutate; one
  adapter per database, run on every app's `tables.json`, is tested once
  and serves all (decision 1.5).
- **A query language in Pleris, compiled to each database's** (SQL's
  joins and filters, MongoDB's pipelines). It is a SQL optimizer by
  another name (charter §2), and its costs are not a count a program can
  state; declared keyed and set reads are (4c).
- **The whole store on each database, and no app across three** (NEXT's
  "one app on each", read literally). Kept, as the negative matrix: it is
  what shows each database refusing what it cannot give. As the app it
  shows less: on MongoDB the store would run only with its commands'
  `transaction` weakened to `snapshot`, which admits write skew and which
  this note does not propose, and on Cassandra it does not build.
- **One interface over every database** (an ORM or a query builder:
  Prisma, SeaORM, sqlx's `Any`). It would hand a program the least
  guarantee they share, or hide that they differ. The boundary is the
  guarantees, stated and measured; each layer's operations are its
  database's own, behind one set of operations per slice.
- **A transaction across databases** (two-phase commit, XA). A
  distributed transaction is a distributed database's work (charter §2);
  PW0345 refuses it, and a command it leads to (5.4) is the outbox's
  answer.
- **Debezium and Kafka between each database and the host.** A JVM and
  two servers more; each layer reading its database's own feed is the
  smaller dependency.
- **Infer a source's guarantees from its driver.** ADR-0207's reason
  holds: a MongoDB has transactions on a replica set and none alone, a
  Cassandra's reads depend on its consistency levels and replication
  factor, and a Cassandra 6 on whether Accord is enabled for a table.
- **Make `reads_outside` a warning.** A command stating `transaction
  serializable` promises what no one database gives across two; a warning
  lets the promise stand, which is what ADR-0207 removed.
- **Let the host merge reads across layers below the seam** (a layer
  calling another's operations). It would hide the cross-database read
  from the checker, as the four composites found above hide the menus'
  reads today.
- **Firestore, D1, libSQL or ScyllaDB in place of the recommended**: owner
  decision 1, each with its reason.

## Acceptance (when built)

- The store and the feed served with no Rust data layer: their data
  declared as tables, their operations Pleris, `store.rs`'s operations,
  `store_pg.rs`, and `feed.rs`'s and `feed_pg.rs`'s data code deleted, and
  TodoMVC's layer before them (D2).
- Each track's, in decision 6: each rule refused with its control
  accepted, each layer through the conformance suite's seven parts with
  its counts pinned at two sizes, each mutant killed.
- The store served in three engines with `Purchases` in SQLite, `Catalog`
  in MongoDB and `Activity` in Cassandra or ScyllaDB, one host and two.
- The negative matrix pinned, each diagnostic by name: `Purchases` as
  MongoDB, Cassandra and D1 give it, `Catalog` as Cassandra gives it.
- A menu changed outside the host, by the database's own client, shown on
  every open store page of both hosts.

## Not claimed

- **A database, or a transaction or snapshot across two**: none is built
  (charter §2); PW0345 and PW0355 refuse a program that needs one.
- **A query language or an optimizer**: the declared reads are keys,
  prefixes and indexes, and their set forms; a filter or a join beyond
  them is the program's Pleris over them, counted. A SQL adapter fusing a
  declared relation into a join is a fixed rewrite, and later.
- **Full-text search as a declared index**: NEXT's own item (a full-text
  source a program declares); the menus' match here is Pleris over a
  declared read.
- **kiokun's data declared**: its build output stays a read-only host
  layer (decision 1.3).
- **Migrations that move data between databases**: a source's database
  is chosen once per deployment; moving one is a deployment's work.
- **That a measurement proves a database.** `provides()` reads what a
  deployment says of itself: its isolation, whether it is a replica set,
  its replication factor and consistency levels. It does not test the
  implementation, which is Jepsen's work. An anomaly test shows that a
  refused level admits the anomaly, not that a granted level never does.
- **Cassandra under clock skew.** A mutation's timestamp comes from a
  client's or the coordinator's clock, and "Updates resolve according to
  the conflict resolution rule of last write wins" (dynamo), so two hosts'
  clocks can order two writes wrongly; nothing models it beyond `reads
  eventual`.
- **Cassandra 6's Accord**: an alpha. A table with it enabled could give
  more, which a layer would measure (`accord.enabled`,
  `transactional_mode`); not built.
- **A second host for `Purchases`**: still the outbox's limit (ADR-0246).
  PostgreSQL's logical decoding is its way, read by SQL since the client
  has no replication protocol; not built.
- **D1, libSQL and Firestore**: not built (owner decision 1). D1 needs a
  host inside Workers, which no part of Pleris is.
- **Search**: the menus' match in MongoDB is a match over names, a regular
  expression or a text index, not a search engine (capabilities.md: "a
  search engine (the database's, or a declared search source)" is reused,
  not built).
- **Regions**: CockroachDB's are optional (T5), and several nodes need a
  licence key.
- **CockroachDB's differences for the PostgreSQL layer**, beyond
  session-scoped advisory locks: which of `xid8`, `pg_current_xact_id()`
  and `hashtext()` it has is not read here; T5's first run measures them.
- **A one-member MongoDB replica set**: the manual asks for several
  members, so the tests run three; whether one behaves alike is recorded
  by the suite, not claimed.
- **D1's `read_committed`** is this note's reading of auto-commit and
  batches, for a command that reads and then writes; no D1 layer is built
  to measure it.
- **Performance**: no throughput or latency of any database; the costs
  are round trips, counted.
- **One migration across the three**: each layer migrates its own
  database, once, recorded.
- **The browser's own storage** (charter §9.1's fourth kind of state): the
  Docs app's, not this one's.

## Owner decisions

1. **One of each pair.** Recommended:
   - **SQLite**, in the host's process: serializable, interactive
     transactions, nothing to download. Not D1 now: its transactions are
     batches, not interactive, so a command that reads and then writes
     commits at `read_committed` at most, and the store's ten serializable
     commands are refused there (PW0346) as on Cassandra; and a host
     outside Workers reaches it only by an administrative API, 1,200
     requests in five minutes. kiokun.com, read only, suits D1 as it is.
     Not libSQL: maintained, with its new work moved to Turso Database,
     not yet 1.0.
   - **MongoDB**: an official Rust driver with a blocking API,
     transactions and change streams, and a tarball that runs as the user.
     Not Firestore: no Google Rust client for its data, listeners over
     gRPC only, and an emulator that needs Java 21 and does not implement
     all of production's transactions. The MongoDB layer could later be
     pointed at Firestore's MongoDB-compatible edition (snapshot or
     serializable transactions, change streams in Preview), measured as
     any deployment, on a cloud account the owner would choose.
   - **Cassandra 5.0.9**: Apache-2.0, generally available, run from a
     tarball as the user, with a Java runtime. Its CDC is files a Java
     reader parses on each node, so it states `changes none`, and the app
     shows the change-feed-dependent query refused (PW0357). **Or
     ScyllaDB**: no JVM, and a CDC log read with plain CQL, so `changes
     feed`; but source-available since 2025.1 (free within 10 TB and 50
     vCPUs, never as a service), installed by apt as root, and not
     documented on WSL2. The layer is the same: `scylla` speaks to both.
2. **The downloads** of 5.7, for the choices made.
3. **What runs on the PC**: the servers from tarballs under the user's
   home, started and stopped by recipes, no system service; and, for
   Cassandra, Ubuntu's `openjdk-17-jre-headless` by apt (root), or a JDK
   tarball under the user's home.
4. **CI**: Cassandra and CockroachDB as the vendors' images (NEXT prefers
   containers on CI) and MongoDB by the same tarball and recipe as here
   (5.6), or every database by its tarball, one recipe for both places.
   Either adds database shards and minutes within the Free plan's 20 jobs
   at once.
5. **CockroachDB (T5)**, or not. Its licence is the CockroachDB Software
   License; a single node needs no key; several nodes, for regions, need
   one or are throttled after seven days.
6. **The Cassandra slice**: an order's activity, written by the backend
   (recommended now, within the owner's scope for the app), or the
   store's reviews once track `reviews` lands; a reader writes a review,
   so its "one review an order" would be T4b's lightweight transaction.
7. **The store's program changes**: a cart's line written with its store
   and price, read from the menu by the program; `reads_outside Catalog`
   on the two cart commands; the search composed in Pleris.

## Questions for the integrator

1. **The declaration** (decision 1): its word (`table`) and clauses, the
   generated functions' names (`get`, `get_all`, `of`, `of_all`, `by_…`),
   and the imports' namespace (`pw:tables/…`); TodoMVC (track `todomvc`)
   as D2's first app with no Rust data layer; the 4c note's `set` pairing
   generated rather than written.
2. **Codes**: PW0354 to PW0366 in DeclarationRules, beside ADR-0207's
   (this note's proposal, each free on every branch), or a track block
   (PW66). And PW0347's and PW0349's revisions.
3. **The seam in `main.rs`**: `from_build_layers` handing its layers to
   `Sources`, and a program's tables to their sources' adapters, at lines
   marked `TRACK SEAM (sources)` and `TRACK SEAM (tables)`.
4. **`sources.json`** gains `sets` and `tells` for each source, and
   `tables.json` is new; the host finds each command's source from the
   contracts.
5. **The name**: `Purchases` (this note's), or `StoreData` kept, holding
   all but `Menus`.
6. **The order**: D5's composites in the 4c note's order, and T3 after the
   search's, a line's price and a cart's stores are Pleris.
7. **`transactions partition`** (T4b): a word of ADR-0207's
   `transactions`, as proposed, or a clause of its own.
