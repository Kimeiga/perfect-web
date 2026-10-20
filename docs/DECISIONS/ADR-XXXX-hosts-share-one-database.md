# ADR-XXXX: hosts share one database: each hears every commit from the outbox, and a session is the database's

Status: proposed by track `second-host` (`track/second-host`, first based on
`track/bounded-statements` a59a18a3, merged with `origin/master` at train-9,
32757966, on 2026-10-19), under the owner's delegation of 2026-10-02. Dates:
2026-10-14 (the design), 2026-10-15 (the integrator's rulings, the outbox,
the sessions), 2026-10-19 (the merge, the render merge's seat model, the
browser suite, the mutation controls). Milestone: E14. Builds on ADR-0246
(the feed on PostgreSQL, its outbox read back by the committing host),
ADR-0297 (a commit tells the pages that ask, at once), ADR-0298 (the store on
PostgreSQL), ADR-0325 (an interaction's answer commits with its writes),
ADR-0334 (a command runs when the clock passes its time), the identity ADRs
(0258, 0263, 0264), track store-accounts' Q3 (a command is answered for the
reader its page was shown to) and the render merge's ADR (train-11, its
seats and its snapshots).

## Corrections and findings (first)

- **A host passed another's commits by in silence** (found 2026-10-15
  reviewing this track's own code, fixed 53774c72): `Tail::heard` read the
  trim's mark before the rows, in two statements; a trim by another host
  between them deleted rows past the cursor while the mark it had read said
  none was missed. The rows are read first now, and the mark after them: a
  trim between the two is learned, at worst a needless reload. Tested by
  running a trim between the two reads.
- **A press made on a page one host served, sent again to another, was
  refused, though the first host had committed it** (found 2026-10-19 by
  this track's browser suite; fixed, interim: Decision 2). A command names
  its page (`pw-document`) by the number its host gave it, and each host
  numbers its pages from one: on B the number names no page, or a page B
  showed another reader, and B answered 403 `out-of-date` or
  `another-reader` before any record was read. Nothing ran twice, but the
  reader was told the page was out of date for a press that had committed.
- **A live page is its host's** (found by the same suite, stated as a
  deployment requirement below): its documents, subscribers and query
  entries are the host's memory (ADR-0161), so a page's stream that reaches
  another host is told to reload, and its commands are answered as from a
  page that host does not hold. A deployment routes a session's requests to
  one host while that host lives (session affinity, as by the `pw-session`
  cookie); a host gone, its pages reload on another, signed in still.
- **This track's connections were unbounded** (found 2026-10-19 reading the
  render merge's ruling 2, fixed ce9f8fb3): `SessionsPg` made a connection
  for every request that asked, and kept eight idle; and every layer opened
  its tail's listening connection as it opened, a test's whose host never
  hears among them. Decision 5.
- **Each command's count is one statement more**, as this ADR's Tests said
  (2a9a2427): a commit that hands the outbox anything takes the lock and
  notifies in one statement. `tests/bounded_statements.rs` re-pinned with
  the reason; every page's count unchanged.
- **Opening a PostgreSQL layer deleted its outbox** (before this track): with
  two hosts, the second to start ate the first's undelivered rows, and
  `delivered()` was `DELETE … RETURNING` (one consumer).
- **Pending sign-ins (state, nonce, PKCE verifier) were in memory too**: a
  callback landing on another host was refused.
- **Correction to the brief**: idempotency with the writes is ADR-0325's,
  already built (store migration 0005, feed 0008); scheduled runs across
  hosts are ADR-0334's (keyed `schedule-<id>`).
- **This track's edits broke three mutation anchors** (fixed 73bc7e0a and
  after): a renamed sign-in and sign-out path, and a duplicated line. Anchors
  are checked by text, so a track that moves shared code re-anchors it, and
  runs each re-anchored script whole.
- **Two flakes seen, neither this track's** (recorded, nothing loosened):
  `tests::bounded_statements::each_page_and_command_is_a_bounded_number_of_statements`
  on PostgreSQL failed "equal at both sizes" in 2 of 2 runs at load 80-90
  (rate_order's reader's open pages read again 42 at the seed, 31 with
  fifty stores more; every command's row, this track's among them, equal):
  ADR-0297's LIVE window leaving the earliest-fetched pages out of a
  commit's re-reads on a slow machine, found on train-10's head and fixed
  test-side on train-11 (1e2dd39c), not yet on master; at load 40-55 its
  mutation script's baseline passed. And
  `the_store_sends_its_slots_after_its_own_content` (1 of 1 whole-suite
  runs at load 60-74), as the render merge's ADR records it on master.

## Context

- **Every production deployment runs more than one host, and Pleris could
  not.** The capability matrix (`docs/vision/capabilities.md`, checked at
  ADR-0279) says: "A second host on one database hears nothing of the
  first's commits (0246)"; "Idempotency is kept in the host's memory, not
  with the writes (0121, 0246)"; and, of accounts, "sessions in memory".
- **What the code did** (read 2026-10-14 on a59a18a3):
  - **Delivery was the committing host's.** `FeedPg` and `StorePg` insert a
    command's events and invalidated entries into their schema's `outbox` in
    the command's transaction, commit, and read them back by id with
    `DELETE … RETURNING` (`feed_pg::delivered`): the outbox was a queue its
    one reader consumed. The host then tells its own sessions
    (`invalidate_queries`, `drain_held` or `send_documents`, ADR-0297's
    tellers).
  - **A host opening the layer deleted the whole outbox** (`DELETE FROM
    outbox` in `FeedPg::open_with` and `StorePg`'s open): "a host just
    opened keeps no answer it could make stale". ADR-0246 said so: "right
    only while one host serves it".
  - **A session's principal was a map in the host's memory**
    (`identity::Principals`), and so was a sign-in under way
    (`Accounts::pending`). A restarted host signed every reader out; a
    second host did not know the first's readers; a sign-in begun on one
    host and called back on another was refused.
  - **Idempotency was already with the writes**: ADR-0325 commits an
    interaction's record in the command's transaction, and "two hosts at
    once: the second's commit meets the first's record by its key and rolls
    back; it answers what the first recorded". Its key is the session, the
    command and the interaction.
  - **What is per host and stays so**: each host's documents, subscribers
    and query entries (`pw-resource`), its materializer (SQLite, in the
    process), its reservations (`try_command`), its timers, its clock, its
    renders' seats.

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
  transaction ID that was still active"; `xid8` "does not wrap around during
  the life of an installation".
- **Logical decoding** (`logicaldecoding-explanation.html`): a slot "will
  emit each change just once in normal operation", but after a crash "might
  return to an earlier LSN, which will then cause recent changes to be sent
  again"; slots "will prevent removal of required resources even when there
  is no connection using them ... In extreme cases this could cause the
  database to shut down to prevent transaction ID wraparound"; "Only one
  receiver may consume changes from a slot at any given time". It needs
  `wal_level` `logical`, a server setting, and a role allowed replication.
- **Isolation** (`transaction-iso.html`, read 2026-10-19 for the render
  merge's claim): 13.2.2, repeatable read, "a snapshot as of the start of
  the first non-transaction-control statement"; "read-only transactions
  will never have serialization conflicts", but a repeatable-read view is
  not necessarily consistent with some serial order of the serializable
  transactions that committed (the read-only anomaly).
- **The client** (`postgres` 0.19.14, already in the lock, ADR-0246):
  `Client::notifications()` and `Notifications::timeout_iter(Duration)`, a
  `FallibleIterator` whose `None` is "no pending notifications or that the
  server has disconnected"; `Client::is_closed()`. No crate is added.

## The integrator's rulings (2026-10-15)

Each ruled by the integrator, 2026-10-15, on this track's questions:

1. **The outbox kept as a log** (read back by `SELECT`, trimmed after ten
   minutes by the database's clock, a reader behind the trim told to
   reload): accepted. The tests and the mutant that assumed "consumed"
   changed, saying why (8ed23e0a; `on_postgres_an_order_keeps_its_lines_and_
   the_outboxes_hold_nothing` is now `..._hold_only_its_hosts_rows`,
   a8f89569; store_postgres's "a command's delivered events stay in the
   materializer's outbox" is the materializer's, unchanged).
2. **Migrations** feed `0010`, store `0018`: accepted (scheduled-orders
   takes store 0016, reviews 0017; the integrator renumbers at a merge where
   needed). The sessions' are feed `0011_sessions`, store `0019_sessions`;
   the outbox's feed `0010_outbox_log`, store `0018_outbox_log`.
3. **Idempotency with the writes is ADR-0325's**: noted; track capabilities
   refreshes the matrix.
4. **The render merge reads each render in one `REPEATABLE READ` snapshot**;
   with several hosts PostgreSQL's read-only anomaly (13.2.2) can show, and
   this ADR says what a second host does to that claim: below, "What a
   second host does to the render merge's claims": not claimed.

## Decision

### 1. The outbox is a log every host reads by its cursor; NOTIFY only wakes them

- **A commit's rows are numbered in commit order.** Just before its
  `COMMIT`, after its writes, its interaction's record and its runs, a
  command's transaction takes `pg_advisory_xact_lock` on its schema's
  outbox key and `pg_notify`s its schema's channel with an empty payload,
  in one statement (`hosts::ordered`), and only then inserts its outbox rows
  (`id` from the outbox's sequence). The lock is held to the commit's end,
  so a transaction that inserts later takes its ids later and commits
  later: **id order is commit order**, and a row a reader sees means every
  smaller id is final (committed, or rolled back for good). A commit that
  hands the outbox nothing takes no lock.
- **Each row says whose it is**: `host`, the committing host's id (128 bits
  from the operating system, made when the layer is opened, never reused),
  and `created`, the database's clock (feed `0010_outbox_log`, store
  `0018_outbox_log`).
- **The committing host still delivers its own commit at once**, read back
  by its ids as before, but by `SELECT`, not `DELETE`: its presser is
  answered after its own pages are told (ADR-0297), exactly as before.
- **Every host tails the outbox** on a connection of its own (a thread per
  served host, `Server::hear_other_hosts`): `LISTEN` on the schema's
  channel, then `SELECT … FROM outbox WHERE id > $cursor ORDER BY id`, and
  again at each notification, and at least every second without one (a
  notification lost to a dropped connection costs at most that second). It
  delivers each of another host's commits, one at a time in id order,
  through the path a commit's delivery takes for sessions other than the
  presser's (`invalidate_queries`, ADR-0297's tellers), and then moves its
  cursor past it. Its own rows it passes by: they were delivered inline. A
  dropped connection is made again, listening first and reading after.
- **A host opening the layer no longer deletes the outbox.** Its cursor
  starts at the outbox's highest id, read as the layer opens: it has no
  page yet, and every page it serves reads the database as it is then.
- **Rows are trimmed by age, not by a reader**: the tail deletes rows older
  than ten minutes on the database's clock, at most once a minute, and
  records the highest id it trimmed (`outbox_trimmed`). A host whose cursor
  is below it missed rows it can no longer read: it tells each of its
  documents to reload (`Recovery::Reload`), never skips them silently, and
  goes on from the trimmed id. The rows are read before the mark.
- **The memory layers are one host by definition**: their data is the
  process's. They keep no outbox, take no lock, start no tail, and the host
  says at start that its data layer shares no database; a host on a shared
  one says its id and its schema.

### 2. Idempotency with the writes: ADR-0325's, held across two hosts

- ADR-0325 keeps the record with the writes, and a host that meets another's
  record by its key rolls back and answers it. With sessions persisted (3),
  a retry through B names the same key as A's press, and is answered from
  A's record, not refused for a session B does not know.
- **A press made on a page another host served** (the finding above,
  interim, cheap to reverse): `Server::kept_answer`, which answers a press
  from a page the host no longer holds or showed another session out of
  the host's memory (the integrator's finding of 2026-10-11), now, where
  the host's memory holds nothing of the press, looks for its interaction's
  committed record (the layer's, or the materializer's where the layer
  keeps none), under the request's own session and then the page's, and
  answers from it. Nothing runs; a press never committed has no record and
  is refused as before; the order of the checks (a kept answer, the stale
  page, the order's wait, the command) is unchanged. One statement, on the
  refusing path only.

### 3. A session is the database's, kept only as its hash

- **A `sessions` table in each PostgreSQL layer's schema** (feed
  `0011_sessions`, store `0019_sessions`): the SHA-256 of the session's id
  (128 bits from the operating system, ADR-0258), its principal (user,
  handle, name, issuer) and when it was opened. The id itself is never
  written: a copy of the table opens no session. A fast hash is enough for
  a 128-bit random secret, which no guess reaches; a password's slow hash
  (ADR-0258's Argon2id) is for a low-entropy secret.
- **Sign-in writes it, sign-out and rotation delete it**, each in a
  statement of its own, before the browser is answered; one not kept is
  said, and the browser keeps its session (503) or the sign-in is refused.
- **Each request reads its session once** (`Principals::refresh`, one
  statement by the hash's primary key, before the request's route), so a
  sign-out on A is seen by B's next request; what the host keeps in memory
  is that request's copy. A read that fails signs the request out.
- **A sign-in under way is the database's too** (`sign_ins`: the state's
  hash, the nonce and the PKCE verifier, and when it began): a callback that
  lands on another host completes it, once, within ten minutes on the
  database's clock, as ADR-0258 holds it; one abandoned is forgotten by the
  next begun.
- **The guest model keeps nothing**: a guest is its session, named for it,
  on any host. **The development provider's accounts stay in its memory**:
  it is plainly not production, and a deployment's provider is its own.

### 4. A deployment of several hosts

- **Every host of a deployment opens its layer on one database and one
  schema**: the store's `pw_store`, or `PW_STORE_DATABASE_URL`'s; the feed's
  the connection's search path, or `PW_FEED_SCHEMA` where the deployment
  names one (TRACK SEAM (second-host) in `main()`; a test run names its
  own).
- **It routes a session's requests to one host while that host lives**
  (session affinity): a page's live state is its host's (the finding above).
  A host gone, its pages reload on another, which finds their readers in
  the database; a press whose answer was lost with it is answered from its
  record (Decision 2).

### 5. Connections, in the render merge's bounded model

The render merge (train-11) holds a layer's pools to a stated bound,
deadlock-free by structure (its ruling 2). This track's connections, per
layer and host:

| | the store | the feed |
| --- | --- | --- |
| commands and single reads | 8 (`CONNECTIONS`) | unbounded (the render merge's finding) |
| renders (the render merge) | 8 (`RENDER_CONNECTIONS`) | 8 |
| sessions and sign-ins (`SessionsPg`) | 8 (`SESSION_CONNECTIONS`) | 8 |
| the outbox's reader (`Tail`) | 1 | 1 |

- **Sessions**: no more than `SESSION_CONNECTIONS` open at once; a request
  past them waits for one to be given back. Each is taken for one
  statement by a thread holding nothing else of the host's (a request's
  read of its session comes before its route; a sign-in's and a sign-out's
  writes hold no lock), so a waiter waits only on statements in flight. A
  guard gives each back, whatever the statement did.
- **The tail**: one listening connection, made by its first read (a layer
  whose host never reads the outbox holds none), never a seat. Its thread
  holds no connection while it tells: `heard()` returns before the telling
  begins, which takes seats as any telling does, holding at most what a
  telling holds.
- **The outbox's lock** is taken on the command's own connection, in its
  transaction, and holds only its commit's tail: a commit waiting for it
  waits on another host's commit tail, which waits on nothing of this
  host's.

### 6. Tests

- Two hosts in one process, each its own `Server` on one PostgreSQL schema
  (`tests/second_host.rs`, 22): a post through A told live to a page on B,
  and not before B reads; each commit told once on each host (A's reader
  hears none of A's own); B's listening connection terminated
  (`pg_terminate_backend`) while A commits, and B catching up from its
  cursor on a connection made again; B opened after A committed starting
  past them; a layer listening only once its host reads; opening either
  layer deleting nothing; B behind the trim reloading its pages; a trim
  between B's two reads learned; a commit through either layer waiting for
  the outbox's lock held by another transaction that writes a row
  meanwhile, its ids past that row, and waking a waiting host at once; ids
  in commit order; a sign-in begun on A completed on B and holding on both,
  kept by hash only; a completed sign-in refused again on the other host;
  one begun eleven minutes ago (the database's clock) refused; an abandoned
  one forgotten; a sign-out on A no one on B; a restarted host keeping its
  readers; a request reading its session once; the sessions' bound held
  under 24 requests at once; the store: a cart changed through A told to
  the reader's page on B; a retry through B answered from A's record; a
  press on A's page sent again to B answered from A's record.
- The browser: two servers per engine on one schema of the engine's own, in
  Chromium, Firefox and WebKit (`e2e-hosts/second-host.spec.mjs`, through
  `playwright.hosts.config.mjs`): a session made on A holding on B and a
  post through A told live to B's page, not reloaded; the browser's post
  request sent again to B answered with A's result, posted once; B stopped
  while A commits and started again, its page reloaded by the new process
  showing what A committed meanwhile, signed in still, and told A's next
  post live; a session signed out on B no one on A.
- `scripts/second_host_mutations.py`, 34 mutants, each rule's;
  `just e14-second-host`.
- `tests/bounded_statements.rs` re-pinned where a commit gains its lock and
  notify (each command one more); a request's session read is no statement
  of the store's tally (its own connections), pinned at one in
  `tests/second_host.rs`.

## What a second host does to the render merge's claims

- **"One host's renders see a serial history"** (the render merge, decision
  3, ruled 2026-10-15) holds per host only: one host's commands run one at a
  time (each layer's `writes` lock), but two hosts' commands run at once,
  and a render's `REPEATABLE READ READ ONLY` snapshot may then show
  PostgreSQL's read-only anomaly (13.2.2): a state no serial order of the
  committed serializable commands produces. **Not claimed** (ruling 4).
  Closing it would read renders in `SERIALIZABLE READ ONLY DEFERRABLE`,
  which may block at its first statement; the render merge rejected it, and
  this track does not reopen it.
- **The commit-to-invalidation moment is longer on another host**: B
  invalidates A's commit when its tail hears it, normally at A's commit's
  notification, at most a second later where the notification was lost
  (`hosts::WAIT_MS`). Until then a render on B whose snapshot is after A's
  commit may take a cached value from before it (the render merge's decision
  5 leaves "the moment between a commit and its invalidation" open, the same
  for every reader); B's telling of A's commit then reads those documents
  again. A render whose snapshot is before A's commit, reading after B's
  tail invalidated, reads alone (decision 5): the two compose.
- **Composed, measured**: `origin/track/render-merge` (117803bf) merged on a
  local trial branch (0230ffb8, never pushed) over this track's then head
  (ce9f8fb3): two conflicts (main.rs' module list; `bounded_statements`'
  PAGES, both comments kept, `rate_order`'s transaction 9, this track's lock
  with the render merge's 8, its open pages 34 and the two calls-again rows,
  the render merge's), and on PostgreSQL 18.6 the two-host tests, the render
  merge's (`tests::render_merge::`, `render_merge::tests::`),
  `bounded_statements`, `feed_pg` and `store_pg`: 93 passed, 0 failed
  (/tmp/second-host-trial-0230ffb8.log).

## Alternatives

- **LISTEN/NOTIFY as the delivery.** At most once, and only to a listener
  connected at the commit: a host restarting, or whose connection dropped,
  misses what was said meanwhile, with nothing to read it from; a payload
  under 8000 bytes bounds an event's values. Kept as the wake-up only.
- **A cursor over the outbox's ids without the lock**: ids are taken at the
  insert, not the commit, so a reader that has seen id 11 committed while 10
  is still running would pass 10 for good. The snapshot's `xmin` answers it
  without serializing commits, but delivers in no commit order, which
  ADR-0297's "told once and in order" wants; the lock is one statement and
  holds only the commit's tail. Cheap to reverse if commits contend.
- **Logical decoding**: a slot per host, which keeps WAL and catalog rows
  while its host is down, down to stopping the database; at-least-once after
  a crash; `wal_level` and a replication role a deployment must grant.
- **An outbox per host, written for each live host at the commit and
  consumed by it**: a registry of live hosts and their heartbeats, a copy of
  each commit per host, and a dead host's rows to clean. A log read by
  cursors needs neither.
- **Sticky sessions at a load balancer instead of persisted sessions**: a
  restart still signs everyone out, and a host taken out of service takes
  its readers' sessions with it. (Affinity for live pages is still asked of
  a deployment, Decision 4: it routes a page's requests to the host holding
  its state; it does not keep the session.)
- **Sessions as signed cookies** (no table): sign-out cannot revoke one
  before it expires (OWASP's Session Management Cheat Sheet: on logout the
  server must invalidate the session).
- **Page numbers made deployment-wide** (the host's id in each), so another
  host would know a page is another's: it changes the protocol's document
  cursor and every page's number for a refusal the record already answers
  (Decision 2); left for the integrator's question below.

## Failure cases

| Case | What happens |
| --- | --- |
| B's listening connection drops while A commits | B reconnects, listens, and reads from its cursor: it misses no row (the notification is lost; the row is not). |
| B restarts | Its pages are told to reload when they ask (a forgotten subscriber); its cursor starts at the highest id; its readers are signed in still. |
| A stops between its commit and its own delivery | A's pages are gone with it; every other host delivers A's commit from the outbox. |
| A commit is refused at `COMMIT` | Its rows roll back: an id never committed, which no reader waits for (the next id is visible only after it is final). |
| B lags past the trim | B tells its documents to reload, and goes on from the trimmed id. |
| A notification queue full | `NOTIFY` fails the commit (PostgreSQL's 8 GB queue); the command refused as any refused commit. |
| Two hosts run one interaction | ADR-0325: one commit records it; the other rolls back and answers the record. |
| A press's answer lost, its retry on another host | Answered from the record its commit wrote (Decision 2); a press never committed is refused as from a page the host does not hold. |
| A session signed out on A | B's next request reads no row, and the id is no one. |
| More requests at once than the sessions' connections | They wait for one given back; each holder runs one statement. |

## Not claimed

- **Effects outside the database run once**: two hosts running one
  interaction both run it, and one commits (ADR-0325, ADR-0334's provider
  cancel).
- **A telling between requests sees a sign-out on another host** before the
  session's next request; its own documents only, at most.
- **A host's own commits and another's interleaved in commit order on its
  pages**: its own are told inline, another's by its tail, each in order;
  every telling derives from the database as it is, so no page shows an
  older value after a newer.
- **A render's snapshot consistent with a serial order of two hosts'
  commands** (13.2.2's read-only anomaly; above).
- **A page served by one host, its stream or its commands on another**:
  told to reload, or answered as from a page that host does not hold (a
  committed press from its record). A deployment routes a session to one
  host (Decision 4).
- **Sessions that expire**: a session row lives until its sign-out
  (ADR-0258 claims no expiry either; it now outlives the process).
- **The development provider's accounts across hosts.**
- **A replica**, as ADR-0246.

## Questions for the integrator

1. **A press on another host's page answered from its record** (Decision 2,
   interim, one call in `kept_answer`): keep it, or make page numbers
   deployment-wide (the host's id in each) so a host knows another's page?
2. **Session expiry**: a session row lives until sign-out. An idle or
   absolute lifetime (OWASP: both) is a rule ADR-0258 never had; a
   deployment setting?
3. **Session affinity** as a stated requirement of a deployment of several
   hosts (Decision 4): in `docs/` where a deployment is described?
4. **`PW_FEED_SCHEMA`**: a deployment setting (the store's schema is fixed
   at `pw_store`): keep, and give the store one?

## Merge notes

- Merged `origin/master` at train-9 (32757966) into this branch, c1ced3d2:
  three conflicts, each a list both sides appended to, resolved keeping
  both (main.rs' module list and test-module list; store_pg.rs'
  MIGRATIONS, reviews' 0017_ratings before this track's 0018 and 0019).
- **With train-10 and train-11** (not on master on 2026-10-19):
  `bounded_statements`' pins move on each (train-10 32/53, the render merge
  16/34, the two 19/37 per train-11's notes); this track adds one statement
  to each command that hands the outbox anything and none to a page, so
  each command's pin there is one more, each page's the same. The render
  merge's composition is measured above.
- **The browser host map** (train-11) registers every spec under `e2e/` and
  starts only the selected tests' hosts; this track's suite is outside it
  (`e2e-hosts/`, its own configuration), its spec starting its own hosts at
  `PORT` + 0..5, so neither collects or registers the other.
- `just/second-host.just` is imported by the justfile, as the reviews and
  render merge tracks import theirs; `e14-second-host` joins
  `NEEDS_DATABASE`.
- `ADR-XXXX` is named in `just/second-host.just`, `hosts.rs`, the PG layers,
  `identity.rs`, `main.rs`, the migrations, the tests, the spec and its
  configuration, and `scripts/second_host_mutations.py`.

## Evidence

Recorded on the PC, 2026-10-19/20 (WSL2, 32 threads shared with other
workers, load averages 40-100 through these runs), PostgreSQL 18.6, rustc
1.97.1, Playwright 1.58.0:

- **`just e14-second-host`** at 5cf742f2, clean
  (`docs/evidence/E14/second-host.txt`, 61 min, PORT=9264):
  `tests::second_host::` 22 passed; `tests::feed_pg::` and
  `tests::store_pg::` on PostgreSQL 32 passed;
  `e2e-hosts/second-host.spec.mjs` 12 passed (4 tests in Chromium, Firefox
  and WebKit, 1.2 min); `scripts/second_host_mutations.py` **34 of 34
  mutants killed** (31 by the two-host tests, 1 by the store's, 2 by the
  browser suite), every baseline green, no process stopped by the memory
  bound.
- **Every mutation script the branch reaches** (`ci_plan.py --changed
  origin/master HEAD`: twelve recipes), each run whole on this branch's code
  (057801ce, the same code as 5cf742f2), in this worktree and two
  throwaway lanes of it: scheduled 37 of 37, reviews 30 of 30, identity 16
  of 16, bounded_statements 12 of 12, store_postgres 12 of 12,
  interactions 10 of 10, cross_session 8 of 8, feed_postgres 7 of 7,
  notifications 23 of 23, uploads 33 of 33, store_accounts 37 of 37 (their
  browser suites in Chromium at PORT=9141, one at a time); with this
  track's own, **259 of 259 mutants killed in twelve scripts**, every
  baseline green, no process stopped by the memory bound, the anchors
  checked after each (3083 in 268 scripts, each once).
- **The server's whole suite** at 057801ce's code (in memory, the
  PostgreSQL tests with the databases named): 607 passed, 1 failed, 9
  ignored; the one, `the_store_sends_its_slots_after_its_own_content`
  (154.1 ms past its 150 ms bound at load 60-74), passed alone, the flake
  the render merge's ADR records on master.
- **The render merge composed** (above): 93 passed on a local trial merge.
- `just ci` at the pushed head, before the push (`just ci > log && git
  push`): nothing is pushed on a red one.

## Next

- The feed's own pool bounded as the store's (the render merge's next step);
  this track's table above then states every layer's total.
- An answer to each question above.
