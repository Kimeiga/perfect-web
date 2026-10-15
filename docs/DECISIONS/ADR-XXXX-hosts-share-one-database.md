# ADR-XXXX: hosts share one database: each hears every commit from the outbox, and a session is the database's

Status: proposed by track `second-host` (`track/second-host`, based on
`track/bounded-statements` a59a18a3), under the owner's delegation of
2026-10-02. Date: 2026-10-14. Milestone: E14. Builds on ADR-0246 (the feed on
PostgreSQL, its outbox read back by the committing host), ADR-0297 (a commit
tells the pages that ask, at once), ADR-0298 (the store on PostgreSQL),
ADR-0325 (an interaction's answer commits with its writes), ADR-0334 (a
command runs when the clock passes its time) and the identity ADRs (0258,
0263, 0264). **Work in progress**: the design is written first; "Built so
far" at the end says what exists.

## Context

- **Every production deployment runs more than one host, and Pleris could
  not.** The capability matrix (`docs/vision/capabilities.md`, checked at
  ADR-0279) says: "A second host on one database hears nothing of the
  first's commits (0246)"; "Idempotency is kept in the host's memory, not
  with the writes (0121, 0246)"; and, of accounts, "sessions in memory".
- **What the code does today** (read 2026-10-14 on a59a18a3):
  - **Delivery is the committing host's.** `FeedPg` and `StorePg` insert a
    command's events and invalidated entries into their schema's `outbox` in
    the command's transaction, commit, and read them back by id with
    `DELETE … RETURNING` (`feed_pg::delivered`): the outbox is a queue its
    one reader consumes. The host then tells its own sessions
    (`invalidate_queries`, `drain_held` or `send_documents`, ADR-0297's
    tellers).
  - **A host opening the layer deletes the whole outbox** (`DELETE FROM
    outbox` in `FeedPg::open_with` and `StorePg`'s open): "a host just
    opened keeps no answer it could make stale". With two hosts, the second
    to start deletes the first's undelivered rows. ADR-0246 said so: "right
    only while one host serves it".
  - **A session's principal is a map in the host's memory**
    (`identity::Principals`, a `HashMap<session id, Principal>`), and so is
    a sign-in under way (`Accounts::pending`, its `state`, `nonce` and PKCE
    verifier). A restarted host signs every reader out; a second host does
    not know the first's readers; a sign-in begun on one host and called
    back on another is refused.
  - **Idempotency is already with the writes** (correcting the matrix):
    ADR-0325 (2026-10-12) commits an interaction's record in the command's
    transaction (`interactions`, store migration 0005, feed 0008), and "two
    hosts at once: the second's commit meets the first's record by its key
    and rolls back; it answers what the first recorded". Its key is the
    session, the command and the interaction: the same browser's retry on
    another host names the same key. ADR-0334's runs are keyed by their
    scheduling, `schedule-<id>`, "the same on every host".
  - **What is per host and stays so**: each host's documents, subscribers
    and query entries (`pw-resource`), its materializer (SQLite, in the
    process), its reservations (`try_command`), its timers, its clock.

## Research

Primary sources, read 2026-10-14 (postgresql.org/docs/18; the `postgres`
crate's source in the lockfile).

- **NOTIFY** (`sql-notify.html`):
  - "if a `NOTIFY` is executed inside a transaction, the notify events are
    not delivered until and unless the transaction is committed";
  - a listening session inside a transaction is told "just after the
    transaction is completed": "notification events are only delivered
    between transactions";
  - the payload "In the default configuration ... must be shorter than 8000
    bytes. (If binary data or large amounts of information need to be
    communicated, it's best to put it in a database table and send the key
    of the record.)";
  - "notifications from the same transaction get delivered in the order
    they were sent", and "messages from different transactions are
    delivered in the order in which the transactions committed"; identical
    channel and payload within one transaction are delivered once;
  - "If this queue becomes full, transactions calling `NOTIFY` will fail at
    commit" (8 GB in a standard installation); "no cleanup can take place
    if a session executes `LISTEN` and then enters a transaction for a very
    long time".
  - It sends "to each client application that has previously executed
    `LISTEN`": nothing is kept for a session that is not listening.
- **LISTEN** (`sql-listen.html`): "`LISTEN` takes effect at transaction
  commit"; "the session will receive all events committed after an instant
  during the transaction's commit step"; the advised order: listen and
  commit, "then in a new transaction inspect the database state ..., then
  rely on notifications to find out about subsequent changes"; "A session's
  listen registrations are automatically cleared when the session ends."
- **Sequences** (`functions-sequence.html`): "the value obtained by
  `nextval` is not reclaimed for re-use if the calling transaction later
  aborts", so "sequence objects cannot be used to obtain 'gapless'
  sequences". A value is taken at the insert, not at the commit: two
  transactions may commit in the other order from their ids (an inference
  the page does not state; it is why a cursor over a plain `bigserial` can
  pass a row that commits later with a smaller id).
- **Transaction ids and snapshots** (`functions-info.html`, §9.27.8): "The
  main use of these functions is to determine which transactions were
  committed between two snapshots"; a snapshot's `xmin` is the "Lowest
  transaction ID that was still active. All transaction IDs less than
  `xmin` are either committed and visible, or rolled back and dead";
  `xid8` "does not wrap around during the life of an installation".
- **Logical decoding** (`logicaldecoding-explanation.html`): a slot "will
  emit each change just once in normal operation", but after a crash "might
  return to an earlier LSN, which will then cause recent changes to be sent
  again", and "clients are responsible for avoiding ill effects from
  handling the same message more than once"; slots "will prevent removal of
  required resources even when there is no connection using them ... In
  extreme cases this could cause the database to shut down to prevent
  transaction ID wraparound"; "For most applications, a separate slot will
  be required for each consumer", and "Only one receiver may consume
  changes from a slot at any given time". It needs `wal_level` `logical`, a
  server setting, and a role allowed replication.
- **The client** (`postgres` 0.19.14, already in the lock, ADR-0246):
  `Client::notifications()` and `Notifications::timeout_iter(Duration)`, a
  `FallibleIterator` whose `None` is "no pending notifications or that the
  server has disconnected"; `Client::is_closed()`. No crate is added.

## Decision

### 1. The outbox is a log every host reads by its cursor; NOTIFY only wakes them

- **A commit's rows are numbered in commit order.** Just before its
  `COMMIT`, after its writes, its interaction's record and its runs, a
  command's transaction takes `pg_advisory_xact_lock` on its schema's
  outbox key and only then inserts its outbox rows (`id` from the outbox's
  sequence), and `pg_notify`s its schema's channel with an empty payload.
  The lock is held to the commit's end, so a transaction that inserts later
  takes its ids later and commits later: **id order is commit order**, and
  a row a reader sees means every smaller id is final (committed, or rolled
  back for good). One statement more per commit that hands the outbox
  anything (the lock and the notify together); a commit that hands it
  nothing takes no lock.
- **Each row says whose it is**: `host`, the committing host's id (128 bits
  from the operating system, made when the layer is opened, never reused),
  and `created`, the database's clock.
- **The committing host still delivers its own commit at once**, read back
  by its ids as today, but by `SELECT`, not `DELETE`: its presser is
  answered after its own pages are told (ADR-0297), exactly as before.
- **Every host tails the outbox** on a connection of its own (a thread per
  layer that keeps an outbox): `LISTEN` on the schema's channel, then
  `SELECT … FROM outbox WHERE id > $cursor ORDER BY id`, and again at each
  notification, and at least every second without one (a notification lost
  to a dropped connection costs at most that second). It delivers each row
  of another host's commit, one commit at a time in id order, through the
  path a commit's delivery takes for sessions other than the presser's
  (`invalidate_queries`, the tellers of ADR-0297, the materializer's record
  for a layer with a session entry), and then moves its cursor past it. Its
  own rows it skips: they were delivered inline.
- **A host opening the layer no longer deletes the outbox.** Its cursor
  starts at the outbox's highest id: it has no page yet, and every page it
  serves reads the database as it is then.
- **Rows are trimmed by age, not by a reader**: the tail deletes rows older
  than ten minutes on the database's clock and records the highest id it
  trimmed (`outbox_trimmed`). A host whose cursor is below it missed rows
  it can no longer read: it tells each of its documents to reload
  (`Recovery::Reload`, the protocol's "this document cannot continue"),
  never skips them silently, and goes on from the trimmed id.
- **The memory layers are one host by definition**: their data is the
  process's, and a second process has data of its own. They keep no outbox,
  take no lock, start no tail, and the host says at start that its data is
  in memory, one host.

### 2. Idempotency with the writes: ADR-0325's, held across two served hosts

Nothing new is built: ADR-0325 keeps the record with the writes, and a host
that meets another's record by its key rolls back and answers it. What this
ADR adds is the evidence through two served hosts, with sessions persisted
(3), so that a retry through B is answered as A answered it, not refused for
a session B does not know.

### 3. A session is the database's, kept only as its hash

- **A `sessions` table in each PostgreSQL layer's schema**: the SHA-256 of
  the session's id (128 bits from the operating system, ADR-0258), its
  principal (user, handle, name, issuer) and when it was opened. The id
  itself is never written: a copy of the table opens no session. A fast
  hash is enough for a 128-bit random secret, which no guess reaches; a
  password's slow hash (ADR-0258's Argon2id) is for a low-entropy secret.
- **Sign-in writes it, sign-out and rotation delete it**, each in a
  transaction of its own, before the browser is answered.
- **Each request reads its session once** (`Principals::refresh`, one
  statement by the hash's primary key), so a sign-out on A is seen by B's
  next request; what the host keeps in memory is that request's cache. A
  telling between requests uses the principal as the session's last request
  read it (Not claimed).
- **A sign-in under way is the database's too** (`sign_ins`: the state's
  hash, the nonce and the PKCE verifier, which the token request needs as
  they are, and when it began): a callback that lands on another host
  completes it, once, within ten minutes, as ADR-0258 holds it.
- **The guest model keeps nothing**: a guest is its session, named for it,
  on any host. **The development provider's accounts stay in its memory**:
  it is plainly not production, and a deployment's provider is its own.

### 4. Tests

- Two hosts in one process, each its own `Server` on one PostgreSQL schema:
  a command through A told live to a page on B; a retry through B of an
  interaction A committed answered from the record and run once; a session
  signed in on A signed in on B, and signed out on A out on B; B's listening
  connection terminated (`pg_terminate_backend`) while A commits, and B
  catching up from its cursor; B opened after A committed starting past
  them; B lagging past the trim telling its documents to reload; the
  committing host telling its own pages once (no second telling from its
  tail).
- The browser: two servers on two ports on one database, in Chromium,
  Firefox and WebKit.
- `scripts/second_host_mutations.py`, each rule's mutant killed;
  `just e14-second-host`.
- `tests/bounded_statements.rs` re-pinned where a commit gains its lock and
  notify, and a request its session's read, each with its reason.

## Alternatives

- **LISTEN/NOTIFY as the delivery.** At most once, and only to a listener
  connected at the commit: a host restarting, or whose connection dropped,
  misses what was said meanwhile, with nothing to read it from; a payload
  under 8000 bytes bounds an event's values. Kept as the wake-up only.
- **A cursor over the outbox's ids without the lock**: ids are taken at
  the insert, not the commit, so a reader that has seen id 11 committed
  while 10 is still running would pass 10 for good. The snapshot's `xmin`
  answers it without serializing commits (read rows from transactions not
  yet delivered whose ids are at or above the last `xmin`, remembering
  those seen above it), but delivers in no commit order, which ADR-0297's
  "told once and in order" wants; the lock is one statement and holds only
  the commit's tail. Cheap to reverse if commits contend.
- **Logical decoding**: a slot per host, which keeps WAL and catalog rows
  while its host is down, down to stopping the database; at-least-once
  after a crash; `wal_level` and a replication role a deployment must
  grant. The outbox already holds what is delivered.
- **An outbox per host, written for each live host at the commit and
  consumed by it** (keeps ADR-0246's "consumed"): a registry of live hosts
  and their heartbeats, a copy of each commit per host, and a dead host's
  rows to clean. A log read by cursors needs neither.
- **Sticky sessions at a load balancer** instead of persisted sessions: a
  restart still signs everyone out, and a host taken out of service takes
  its readers' sessions with it.
- **Sessions as signed cookies** (no table): sign-out cannot revoke one
  before it expires (OWASP's Session Management Cheat Sheet: on logout the
  server must invalidate the session).

## Failure cases

| Case | What happens |
| --- | --- |
| B's listening connection drops while A commits | B reconnects, listens, and reads from its cursor: it misses no row (the notification is lost; the row is not). |
| B restarts | Its pages are told to reload when they ask (a forgotten subscriber, as today); its cursor starts at the highest id. |
| A stops between its commit and its own delivery | A's pages are gone with it; every other host delivers A's commit from the outbox. |
| A commit is refused at `COMMIT` | Its rows roll back: an id never committed, which no reader waits for (the next id is visible only after it is final). |
| B lags past the trim | B tells its documents to reload, and goes on from the trimmed id. |
| A notification queue full | `NOTIFY` fails the commit (PostgreSQL's 8 GB queue); said, and the command refused as any refused commit. |
| Two hosts run one interaction | ADR-0325: one commit records it; the other rolls back and answers the record. |
| A session signed out on A | B's next request reads no row, and the id is no one. |

## Not claimed

- **Effects outside the database run once**: two hosts running one
  interaction both run it, and one commits (ADR-0325, ADR-0334's provider
  cancel).
- **A telling between requests sees a sign-out on another host** before
  the session's next request; its own documents only, at most.
- **A host's own commits and another's interleaved in commit order on its
  pages**: its own are told inline, another's by its tail, each in order;
  every telling derives from the database as it is, so no page shows an
  older value after a newer.
- **The development provider's accounts across hosts.**
- **A replica**, as ADR-0246.

## Questions for the integrator

1. **The outbox kept, not consumed** (a rule ADR-0246 and ADR-0298 rely on:
   "the outbox is consumed", "both outboxes end empty", the mutant "a
   delivered event left in the outbox"). Interim, cheap to reverse: rows
   read back by `SELECT` and trimmed at ten minutes; those tests say "read
   back once and kept for the other hosts until trimmed".
2. **Migration numbers**: feed `0010_hosts.sql`, store `0018_hosts.sql`
   (scheduled-orders takes 0016, reviews 0017).
3. **The capability matrix**: "Idempotency is kept in the host's memory" is
   ADR-0325's past; this track does not rebuild it.

## Built so far

(WIP, 2026-10-14) This ADR. Next: the outbox log and the tail (feed, then
store), sessions persisted, the tests, the browser suite, the mutation
script and the recipe.
