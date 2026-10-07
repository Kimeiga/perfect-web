# ADR-0246: the feed's data in PostgreSQL, held to what its source states

Status: accepted under the owner's delegation of 2026-10-02; the owner
approved its crates (`postgres` 0.19.14 and what it brings) on 2026-10-07.
It builds ADR-0207's "Not claimed": "a host does not yet compare a source's
clauses with what its database provides". Date: 2026-10-07. Milestone: E14,
the app layer. Written on branch `pg-data-layer` by a parallel session, on
the owner's direction, and integrated here.

## Context

- **The feed's data was in memory** (ADR-0218): users and posts under one
  lock, a command's writes staged and published after the materializer's
  transaction committed its events. No program had run on a database a
  deployment chose.
- **ADR-0005 and charter §10.4**: SQLite first, PostgreSQL when nodes
  separate; "use ordinary transactions and a transactional outbox". Both
  engines "must sit behind the same generated interface".
- **ADR-0207 gave a program the words** to state what its database
  guarantees (`transactions`, `reads`, `changes`) and held the program to
  them. Nothing held the database to them. A program stating serializable
  transactions would run on a read committed one, silently.
- **ADR-0208**: a command's events commit with its writes, "if and only if
  the database transaction commits". With the feed in PostgreSQL and the
  events in the host's SQLite materializer, that would be two transactions.
- **Found building it**: the host's commit path could not put a command's
  events in the layer's transaction. A `Staged` had its rows and a
  `publish` that cannot fail; it was never handed the events, and a
  database's commit can fail.

## Research

- **PostgreSQL 18** (postgresql.org, read 2026-10-07; 18.6 is the newest
  supported release on its versioning page):
  - "Read Committed is the default isolation level in PostgreSQL."
  - Repeatable Read "is implemented using a technique known ... as Snapshot
    Isolation": ADR-0207's `snapshot`.
  - A serialization failure: the application "should abort the current
    transaction and retry the whole transaction from the beginning."
  - On a hot standby "`transaction_read_only` is always true", and setting
    serializable "will generate an error".
  - `NOTIFY` inside a transaction is "not delivered until and unless the
    transaction is committed"; its payload "must be shorter than 8000
    bytes"; a session receives "events committed after" it listens.
- **The client, read on crates.io and in the crate's source**:
  `postgres` 0.19.14, the newest, not yanked, MIT OR Apache-2.0,
  rust-version 1.85, rust-postgres's synchronous client over tokio-postgres
  0.7.18. Pure Rust, no libpq. `Client::connect(url, NoTls)`,
  `batch_execute`, `query`, `query_one`, `query_opt`, `execute`,
  `transaction()` and `Transaction::commit`, `Error::as_db_error` and
  `DbError::code`, checked in `~/.cargo/registry/src/*/postgres-0.19.14`
  and `tokio-postgres-0.7.18`.

## Decision

### 1. `FeedPg`, a `DataLayer` on PostgreSQL

- **Beside the in-memory layer, which stays the default.** `main` opens it
  where `PW_FEED_DATABASE_URL` names a database. `Server::from_build`, which
  every existing test calls, is unchanged; `from_build_with` takes the
  layer a deployment opened.
- **Its schema is plain SQL**, `server/migrations/feed/0001_schema.sql` and
  `0002_seed.sql`, the in-memory layer's first data. Each applies once, in
  one transaction under an advisory lock, recorded in `pw_migrations`.
  - Users; posts, each `p` and its sequence number, replying to a post by a
    foreign key; likes, a row each, so two likes never update one count;
    the outbox, its values kept as JSON as the command computed them.
  - Every written row records `committed_in`, `pg_current_xact_id()`: a
    test shows a post and its event written by one transaction.
- **A thread is one recursive statement**, so one snapshot. The timeline
  is newest first, `LIMIT` the page's `shown`.
- **A command is one transaction**, `BEGIN ISOLATION LEVEL SERIALIZABLE` at
  `begin`, whatever the connection's default. What it reads, it reads in
  the transaction. A command that fails, traps or answers a declared error
  is dropped and rolls back.
- **One command of a host at a time**, as the in-memory layer's lock. A
  host never conflicts with itself; serializable holds against every other
  writer of the database, and a commit it refuses keeps nothing.

### 2. A command's writes and its events commit in the layer's transaction

- **`Staged::commit(events, invalidated)`**, defaulted to `Ok(None)`.
  - The host calls it after the command ran and wrote, before anything is
    delivered.
  - `FeedPg` inserts each into its outbox, in the command's transaction,
    and commits. A commit the database refuses is the command refused: the
    host answers it so, and tells no one.
  - It returns what its outbox committed, read back once the transaction
    committed and consumed (`DELETE … RETURNING`). The host delivers that,
    through the same `invalidate_queries` and telling as before (ADR-0219).
  - `None`: the layer keeps no outbox, and the host's materializer commits
    the events with `rows` (ADR-0208), as before. The store's and the
    in-memory feed's layers are unchanged.
- **Read back from the outbox, not LISTEN/NOTIFY.**
  - Delivery is in-process: the host that committed is the host whose
    sessions are open. Reading back the rows its transaction wrote is the
    outbox pattern itself, and needs no connection held open to listen.
  - NOTIFY delivers to sessions listening at the commit, and nothing to one
    that is not. A payload under 8000 bytes would bound an event's values.
  - A second host would need one or the other to hear the first's commits;
    the outbox's rows are what it would read (Not claimed).

### 3. The host holds a layer's database to what the program's sources state

- **`pw build` writes `sources.json`**: each source's clauses, read as the
  checker reads them (a clause left out guarantees nothing).
- **`DataLayer::provides`**, defaulted to the host's own database (ADR-0207:
  serializable, `strong`, no feed). `FeedPg` measures it: a transaction
  opened as a command's is, asked `transaction_isolation` and
  `transaction_read_only`, and rolled back.
  - `serializable`; `repeatable read` is `snapshot`; read committed and
    read uncommitted are `read_committed`.
  - A transaction that may write is on the primary: `strong`. A read only
    one is refused outright: no command's writes could commit.
  - No feed of its changes: the program's events are its outbox's.
- **Before serving, the host compares** each source holding a resource the
  layer's grants name (`database.read<Post>` names `Post`), and the host's
  own guarantees for a resource no source holds:
  - its transactions at least as isolated, in the checker's order;
  - each promise its reads state, `strong` giving every one;
  - a feed of its changes, where it states one.

  A shortfall refuses to start, naming the source, the clause and what the
  database gives, as an operation no layer supplies is refused.
- **`PW_FEED_TRANSACTIONS=connection`** leaves a command's isolation to the
  database's default, which the host then measures: refused against a
  read committed default, served against a serializable one.

### 4. The feed states what its database gives

```
source FeedData
    holds        Post, User
    transactions serializable
    reads        strong
    changes      none
```

The program checks unchanged. The in-memory layer gives the same.

## Alternatives

- **The trait unchanged: the layer commits in `publish`.** `publish` runs
  after the materializer's transaction, is handed no events and cannot
  fail. The events would commit in SQLite and the writes in PostgreSQL:
  two transactions, which ADR-0208 rules out. The two methods added are
  defaulted, so no implementation and no caller of the trait changes.
- **The outbox in the host's SQLite, a two-phase commit.** It is a
  distributed transaction for one command, and charter §2 forbids building
  a database.
- **`sqlx`**: asynchronous, with macros and a runtime of its own, and the
  host is synchronous and threaded. **`diesel`**: an ORM over libpq, a C
  library. `postgres` is the synchronous client of the same project as
  tokio-postgres, pure Rust, and the smallest that does this.
- **Declare what a database gives in the deployment's configuration.** A
  configuration can be wrong as a program can; measuring the connection is
  what finds `default_transaction_isolation` set otherwise.
- **Retry a serialization failure.** PostgreSQL's advice. A command is a
  compiled component run once per interaction (ADR-0121), and a retry would
  run it again. One host's commands do not conflict with each other here.

## Consequences

- A deployment chooses where the feed's data is, and the program states
  what it needs of it; a mismatch stops the host before a request.
- The server builds tokio as a dependency of `postgres` (its runtime runs
  the client's connection): 47 crates enter the lockfile, `stringprep`,
  `md-5`, `hmac` and `sha2` among them for SCRAM authentication.
  cargo-deny: licenses, bans and sources pass;
  six duplicate crates warn (`sha2` 0.10 and 0.11 and their digests,
  `fallible-iterator` 0.2 and 0.3). Its advisories failed only for
  Wasmtime, as on master then; integrated after ADR-0244, they pass.
- A test that sets `PW_FEED_DATABASE_URL` runs in a schema of its own and
  drops it. Without one, each PostgreSQL test passes doing nothing; the
  recipe says so and records nothing.

## Acceptance

Recorded by `just e14-feed-postgres` in
`docs/evidence/E14/feed-postgres.txt`, against PostgreSQL 18.6:

- **`server/src/tests/feed_pg.rs`, 10 tests:**
  - the database provides what the feed states, its default read committed;
  - a post and its event are written by one transaction, and the outbox is
    consumed;
  - a post reaches another session's open timeline, after the author is
    answered;
  - a like is a row, and reaches another session's open thread; a like of
    no post writes nothing;
  - a reply is in its thread, as deep as it goes; a reply to no post writes
    nothing;
  - the timeline shows twenty, then forty when the page asks, replies in
    none;
  - a command refused at its commit (a deferred constraint, after the post
    and its event were written) leaves no post, no event, and no session
    told; the next post commits;
  - the feed on PostgreSQL serves the same home page and thread as in
    memory, to the byte, after the same commands;
  - **negative controls**: `changes feed` is refused; `transactions
    serializable` against a read committed default is refused, and served
    against a serializable default and with serializable set on each.
- **The server's other 150 tests**, on the in-memory layer, unchanged.
- **`scripts/feed_postgres_mutations.py`: 4 of 4 mutants killed.**
  - the events committed outside the command's transaction;
  - a command's transaction left at the connection's default;
  - the host serving whatever its database gives;
  - a refused commit answered as committed.
- **The compiler's tests**: the committed kiokun build gains its
  `sources.json` (`[]`), which `evidence_is_current` compares.

## Not claimed

- **A second host.** Delivery is the committing host's. Another host on the
  same database would hear nothing of the first's commits: its sessions
  need the outbox polled or a NOTIFY relayed. Rows a host left undelivered
  (stopped between a commit and its read) are consumed when one next opens
  the database, which is right only while one host serves it.
- **Concurrent commands of one host.** They are serialized by the layer's
  lock, as in memory. Serializable is exercised against another writer
  only through a refused commit, not a measured serialization failure.
- **A replica.** A standby is refused; reads from a replica that lags, with
  `reads eventual`, are not built.
- **Idempotency kept in the database.** A command's interaction is still
  remembered by the host in memory (ADR-0121), not committed with its
  writes. ADR-0207's PW0348 asks for a transaction it commits in; the host
  does not yet put it there.
- **The store on PostgreSQL**, and the feed in browsers on it: the browser
  suite runs on the in-memory layer.
- **`reads` measured beyond primary or not.** `synchronous_commit` and
  similar settings change durability, not what a reader sees, and are not
  read.

## Integration

Written on `pg-data-layer` (six commits on `745a972`) and rebased onto
ADR-0245 without a conflict; the lockfile builds `--locked` with Wasmtime
48.0.5 beside PostgreSQL's crates, and `just audit` passes. Here:

- **Numbered**: ADR-0246, in the ADR, the code's comments, the recipe, its
  mutation script, the migration and `tools/versions.lock`.
- **The verification run's database job** (ADR-0245): a recipe run against
  a database (`e14-feed-postgres`) is planned there, not in a shard, and
  runs against PostgreSQL 18.6 in a service container, a throwaway
  database each run. A test of the plan holds it there.
- **kiokun's `sources.json`** was written by hand on the branch. `just
  e10-kiokun`, run again here, writes the same bytes: it is the recorded
  command's now.
- **The recipe, run here** against PostgreSQL 18.6: 10 tests, the server's
  other 150, 4 of 4 mutants killed.
