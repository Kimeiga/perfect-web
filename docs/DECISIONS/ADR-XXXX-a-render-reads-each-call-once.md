# ADR-XXXX: a render reads each call once, in one snapshot, in a bounded few seats

Status: proposed by the render-merge worker, NEXT 4c's first step (track
`render-merge`), under the owner's delegation of 2026-10-02. Dates:
2026-10-14 (the merge), 2026-10-17 (the integrator's rulings of 2026-10-15
built). The first of ADR-0337's ruled order ("1. The render merge"), its
second case. Builds on ADR-0336 (NEXT 4b: the `Counted` connection and its
`Tally`), ADR-0352 (bounded statements wait for the completed payment),
ADR-0218 (a host serves any program; its data is the deployment's), ADR-0246
(a layer's database held to what its sources state), ADR-0303 (one query
with one key read once for its document), ADR-0304 (what a read holds
until), ADR-0127 (a query's declared policies, its invalidations), ADR-0151
(a document's last attempt read in the table), ADR-0152 (a flight held
while it is read), ADR-0271 (a session's turns) and ADR-0277 (a
materialization kept).

## Corrections and findings (first)

- **The flaky count this track's branch showed was ADR-0352's race**, not
  the render merge. Found here on 2026-10-15 07:06Z with temporary
  instrumentation (never committed): the sweep's `settle_payment`, a
  serializable transaction, was refused (PostgreSQL 40001, "concurrent
  delete") because the scheduler's `Runs::ran` deleted the run's
  `schedules` row on another connection, outside the writes lock; the pinned
  12 was the count of that refused attempt, 14 the committed one. The base
  gave the refused 12 in 4 of 4 runs; this branch, slower, sometimes let the
  settle commit (14). ADR-0352 found the same cause independently and fixed
  it (`ran`/`retry` take the writes guard; the tests wait for the payment's
  completion, not a quiet tally; the pin corrected to 14). On this branch
  merged with it, the payments scenario ran 12 times: every run, at the seed
  and with fifty stores more, gave 37, 17, 25, 14, 26, 14, the pins
  (`/tmp/render-merge-pay-runs.log`, 21-29 s each).
- **Correction of this track's first design** (2026-10-14): "a render's
  connection never waits on the bound" lent a render a connection past the
  store pool's bound of eight: unbounded, which the integrator refused
  (ruling 2). Replaced by the renders' own bounded seats (decision 4).
- **A render held its connection while it waited for the subscriber
  table** (found 2026-10-17, by reading the code for ruling 2): the drain's
  render spanned `send_documents_where`'s table lock and its derive loop,
  and a document that could not be read, or a session whose count could
  not be, was said unshowable (the table) inside the render. A document's
  last attempt (ADR-0151) holds the table while it renders: with a bounded
  pool that waits, eight drains waiting for the table while holding every
  seat, and the last attempt holding the table while waiting for a seat,
  is a deadlock. Restructured (decision 4), tested both orders, and now
  checked across the whole suite: in tests, taking the table or a session's
  turn inside a render panics (`Table::lock`, `Turns::lock`); the memory
  suite's 612 tests met neither.
- **A render filled the shared cache with what may be older than an
  invalidation** (found 2026-10-17): a render whose snapshot predates a
  commit, reading a query after the commit's invalidation dropped it, kept
  its pre-commit value for the query's freshness (the store's `Store` 30 s,
  `Menu` 5 minutes, `Recommendations` 10). Before the merge every fill read
  the latest commit. Closed by decision 5.
- **Reads that never join another's flight override a declared policy**
  (found 2026-10-17): the first answer to ruling 3 (a render's read never
  joins one in flight, commit ec72b02a) failed two existing tests that hold
  `concurrency one_per_key`, the default: concurrent reads of one key share
  one (ADR-0152's `a_flight_another_reader_waits_for_is_not_stopped`, and
  `pages_read_at_once_share_an_ask_their_query_says_is_one_per_key`).
  Replaced by decision 5, which keeps the policy and closes the snapshot
  question where the program's invalidations say it matters.
- **PostgreSQL's repeatable-read snapshot is its first statement's**, not
  `BEGIN`'s (13.2.2): on PostgreSQL a render's snapshot is its first read's,
  in memory its beginning's. Both are one snapshot per render; the test
  that holds a read in flight holds it once it has read.
- **A test hook of the shape `if let Some(h) = m.lock().take() { h() }`
  holds the mutex through the call** (the 2024 edition's `if let` keeps the
  scrutinee's temporaries for the block): this track's in-flight hook first
  blocked the second render on the hook's own mutex, not on any flight.
  Written `let h = m.lock().take(); if let Some(h) = h { .. }`. The older
  `between_bindings`, `keyed_fetched` hooks have the shape; one thread fires
  each, so it is harmless there.
- **The feed's PostgreSQL pool has no bound at all** (pre-existing,
  `feed_pg::Pool`: a connection for every thread that asks). Its renders are
  now bounded (eight seats); its commands and single reads are not. Next
  step, below.
- **Six other tracks' mutation controls were anchored in the code this
  moved** (clock, documents, keyed_reads, materializer_failure, pages,
  tell_at_once): each re-anchored to the same mutation, each script run
  whole (Acceptance).

## Context

- **One render called one operation six times.** After `place_order`'s
  commit the host reads the session's documents again: the drain reads the
  cart for the session's entry (`drain_held`, `part_text`), then every open
  document is read. Every store page's layout binds `Cart(current_user())`,
  and ADR-0303 reads one query once per document, not per render, so
  `user-carts#current` ran six times, 21 statements (4b's tally).
- **Each call read its own snapshot.** Two bindings of one page could
  straddle a commit: the cart page's `Cart` read before another writer's
  commit and its `Checkout` after it showed a cart and a total that never
  existed together.
- **The settled rules** (the integrator's ruling, 2026-10-14; ADR-0337's
  second case): within one render the host makes each identical operation
  call once, the operation and its arguments as the key; nothing is kept
  past the render; the render's calls run in one read transaction; the
  control, one operation with other arguments, reads twice.
- **The integrator's rulings of 2026-10-15** (on this track):
  1. A render reads in `REPEATABLE READ READ ONLY`: accepted. Several hosts
     on one database may show 13.2.2's read-only anomaly: not claimed.
  2. Connections past the bound: an unbounded number is not acceptable. The
     total must be bounded and stated, and deadlock-free by structure;
     tests: the bound held under N concurrent renders, no deadlock.
  3. One render reads in one snapshot. Whether a render can take another
     render's flight (read in another snapshot): say so, and close it or
     state it.
  4. The flaky count: find its cause; never re-pin to hide a race.
- **PostgreSQL 18's isolation levels** (read 2026-10-14): 13.2.1, read
  committed, each statement its own snapshot; 13.2.2, repeatable read, "a
  snapshot as of the start of the first non-transaction-control statement in
  the transaction", and "read-only transactions will never have
  serialization conflicts", but a view not necessarily consistent with some
  serial execution; 13.2.3 and SET TRANSACTION, `SERIALIZABLE READ ONLY
  DEFERRABLE` "may block when first acquiring its snapshot".

## Decision

1. **A render** is the host reading what a session's documents show at
   one moment, on one thread: a page's bindings (`bindings_where`); a
   session's documents read again (`send_documents_where`); the drain after
   a commit (`drain_held`), its read of the cart and the documents' read
   after it, one render. **A thread reads in one render of a layer at a
   time**: one begun inside it, for any session, is that one (each call is
   still its session's own: the session is part of its key), so no thread
   holds two seats. A read for no session inside a render (a
   materialization's) is the render's. The host's alone
   (`render_merge.rs`); no program changes.
2. **Each identical call once.** In a render, a call of a data layer's
   operation is answered from what the render read when it made the same
   call, for the same session, with equal arguments; otherwise the layer is
   called and its answer kept until the render ends. Only answers are kept,
   never failures; only the layer's operations, never the platform's (the
   session, the principal, the clock, whose calls note what a read holds
   until, ADR-0304). `/metrics` counts the calls the layer ran.
3. **One snapshot.** A render asks its layer for a snapshot where it
   begins (`DataLayer::snapshot`) and reads each of its queries over it
   (`reads_in`); by default none (kiokun's files).
   - In memory (the store's, the feed's): the data as its last commit left
     it, each part behind its `Arc`.
   - On PostgreSQL (the store's, the feed's): a seat (decision 4) and one
     transaction, `BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY`
     (`feed_pg::READ_SNAPSHOT`), opened at the render's first read, whose
     snapshot is that statement's, committed when the render ends. Read only:
     never a serialization failure, never retried. Not `SERIALIZABLE READ
     ONLY DEFERRABLE`: it may block at its first statement behind every
     serializable writer, and one host's commands run one at a time (each
     layer's `writes` lock), so one host's renders see a serial history
     (ruled by the integrator, 2026-10-15: accepted).
   - A read that fails ends the transaction, and the next read opens
     another.
4. **Seats: bounded, every one given back** (ruling 2, ruled by the
   integrator 2026-10-15). Each PostgreSQL layer has its renders' own pool
   (`feed_pg::Renders`): `RENDER_CONNECTIONS` = 8 seats, no more connections
   open than that (a seat asks for one only while it holds none, an idle one
   first). Stated total per layer: the store's eight for commands and single
   reads (`CONNECTIONS`) and eight for renders, sixteen; the feed's renders
   eight, its own pool unbounded (finding above). Deadlock-free by
   structure:
   - **A render takes its seat where it begins**, waiting while all eight
     are taken. Its thread holds then at most its session's turn (a drain, a
     telling) or the subscriber table (a document's last attempt): never a
     connection, another seat (decision 1), the writes guard (a command's
     staging is given up before its drain), or a read in flight or a
     materialization's making (no render begins inside one).
   - **Holding its seat, a render waits on nothing a waiter for one holds.**
     It ends before its thread waits for the table: the drain takes the
     documents it tells (and the interactions their values include) before
     its render begins, and its render ends once they are read, before they
     are told (`to_tell`, `tell_in`); a document or a session that could not
     be read is said so after it. It takes no session's turn. A read in
     flight it joins, or a making it waits for, is another render's (which
     holds its seat already) or a single read's (which waits only for the
     commands' pool, whose holders wait for no seat). Checked in every test:
     the table's and a turn's `lock` panic in a render.
   - **A render that waits past `SEAT_WAIT` (30 s) fails, saying so**:
     every read of it answers that it waited; never a read outside its
     snapshot, never a seat past the bound. A backstop, not a bound a render
     is expected to meet. A test's host waits 5 s, so a control that keeps a
     seat fails in seconds.
5. **What a render takes of another's** (ruling 3, stated and closed where
   the program's invalidations say it matters). A render begins as of the
   queries' invalidations so far (`pw_resource::Resources::epoch`, taken
   before its snapshot); pw-resource stamps each resource's last
   invalidation (by `invalidate_key`, `invalidate_where`, `invalidate`,
   `expire`; one stamp per resource, as many as the program has).
   - **Nothing invalidated the query's resource since the render began**:
     the read is as the query's policy says: what the cache keeps, or a read
     in flight (`concurrency one_per_key`, the default), which may have begun
     in another render's snapshot or in none. **So a render can take another
     render's flight: yes.** Its value is what this render's snapshot holds as
     far as the program's declared invalidations say: a commit that changes
     it, declared, invalidates it, and stops a read in flight (ADR-0127).
   - **Something did**: the render reads it alone
     (`fetch_cancellable_as_of`), in its own snapshot: from no cache, joining
     no read and joined by none, its value not kept. So no render shows a
     value newer than its snapshot by a change the program declared, and no
     render keeps, or hands another reader, a value older than one.
   - **Not closed** (stated): a change the program does not declare (an
     undeclared dependency, the graph's to refuse); the moment between a
     commit and its invalidation, the same for every reader as before; a
     materialization's kept entry, or its making under way, which a render
     waits for where the materialization declares `stampede single_flight`,
     is as of its making.

## Measurements

On PostgreSQL 18.6, 2026-10-17, by 4b's tally (`tests/bounded_statements.rs`,
each count equal at the seed and with fifty stores more), with the merge and
with it switched off (a temporary edit, reverted; off, every row is master's):

| one render after a commit | off | on |
| --- | --- | --- |
| `place_order`, taking no payments: its queries read again | 21 | 16 |
| its `user-carts#current` calls | 6 | 1 |
| `rate_order`: its reader's seven open pages read again | 42 | 34 |
| their `user-carts#current` calls | 7 | 1 |
| their `user-orders#current` calls (2 statements each) | 2 | 1 |

Every other count 4b and ADR-0352 pinned is unchanged. The payments
scenario (b): 12 runs, every one 37, 17, 25, 14, 26, 14 at both sizes.

The bound, 24 renders of one page at once, each held at its reads until
eight were seated together (`on_postgres_the_renders_bound_holds_under_many_renders_at_once`):
all 24 read the page; most seats taken at once 8; most connections open at
once at most 8.

## Alternatives

- **An interim hard cap that fails at the cap** (ruling 2's fallback):
  renders never wait, and the ninth at once fails. Bounded and
  deadlock-free trivially, but a burst past eight fails readers that would
  have been served a moment later. Waiting, with the structure above, is
  the full answer; the 30 s limit keeps a loud failure for a wait no one
  ends.
- **The seat taken at the render's first read** (lazily): a render's first
  read can happen inside a read in flight or a making, which others wait
  for: a waiter for a seat would hold what a seat holder waits on.
- **Renders share the commands' pool**: a render holding a seat may make a
  single read (the commands' pool) while a command's drain waits for a
  render's seat: two pools ordered one way are simpler to argue.
- **A render never joins a read in flight** (built first, ec72b02a):
  overrides `one_per_key`, and still filled the cache from old snapshots.
- **A render never fills the cache**: the cache then fills only outside
  renders, which read almost nothing.
- **Merge queries, not operations; a cache kept past the render; the
  platform's calls merged; serializable deferrable; a savepoint per read**:
  as in the first design (2026-10-14), rejected for the reasons given there:
  a query's run notes its document's instant; a kept cache needs the
  invalidation model the ruling does without; a clock call answered again
  notes nothing; deferrable may block; a savepoint is two round trips per
  read for a failure that already makes its page unshowable.

## Acceptance

- `render_merge.rs`' own tests (8): identical calls once; other arguments
  each; another session's call its own; a failure made again; one snapshot,
  nothing kept past the render; the snapshot asked for where the render
  begins; a render begun inside another that one, whatever its session; one
  layer's render answering for no other's.
- `tests/render_merge.rs` (22), the store in memory and on PostgreSQL
  (`PW_STORE_TEST_LAYER`), the feed on PostgreSQL: a commit's documents read
  in one render, the cart once, each store's reach for its own; the feed's
  two timelines read once; another writer's commit between two bindings'
  reads seen by neither (cart page, notifications page) and the controls,
  two renders that straddle it; each layer's snapshot; a failed read ending
  the render's transaction (store, feed); the clock's calls never merged; a
  render reads in a seat with the commands' bound held, and the control, a
  single read waits; a render past the bound waits for a seat; one waiting
  past its limit fails saying so; 24 renders at once within the bound; no
  render holds its seat waiting for the table, both orders, on the store
  (drains) and the feed (documents told); a render reads alone what a
  commit invalidated since it began; a document, and a session, that could
  not be read said so after the render.
- pw-resource `tests/read_as_of.rs` (4): ordinary where nothing was
  invalidated; alone, unkept and not served what is kept, after each of the
  four invalidations; neither joins nor is joined nor kept; the controls.
- `tests/bounded_statements.rs`: the re-reads 21 -> 16 and 42 -> 34, and
  rows of their own for the calls made again.
- `scripts/render_merge_mutations.py`, 40 mutants, one for each rule (the
  merge, the host's renders and their order with the table, the seats, the
  snapshots in memory, pw-resource's stamps and reads alone), each command
  read by `cargo_test_results` (ADR-0352); results in Evidence.
- The re-anchored scripts, each run whole (Evidence).
- The server's suite, in memory and on PostgreSQL; `just ci`.

## Evidence

Recorded on this branch, 2026-10-17, PostgreSQL 18.6, rustc 1.97.1 (the
machine shared with other workers, load average 28 to 64 on 32 threads):

- `just e14-render-merge` at a500f77a, clean
  (`docs/evidence/E14/render-merge.txt`, 2094 s): the merge's tests with
  the store in memory 30 passed; with the store on PostgreSQL, and NEXT 4b's
  counts, 40 passed; pw-resource's `read_as_of` 4 passed;
  `scripts/render_merge_mutations.py` **40 of 40 mutants killed** (21 in the
  `memory` suite, 8 `postgres`, 9 `resource`, 1 `unit`, 1 `bound`), every
  baseline green, no process stopped.
- The server's whole suite: on PostgreSQL 616 passed, 0 failed, 9 ignored
  (a500f77a); in memory 616 passed, 0 failed, 9 ignored (6670583a, the same
  host; a500f77a changed only a test host's seat wait and the script's
  suites); and in memory again in `just ci` (below). The table's and a
  turn's checks fired in none of them.
- The six re-anchored scripts, each run whole at a500f77a: clock 25 of 25,
  documents 6 of 6, keyed_reads 10 of 10, materializer_failure 6 of 6,
  pages 7 of 7, tell_at_once 6 of 6 (the two with browsers after their
  recipes' preparation, chromium, PORT 8541; their first run lacked
  `kiokun-server`, which their recipes' callers build, and refused its
  baseline).
- The payments scenario, 12 runs at 14331449: each 37, 17, 25, 14, 26, 14 at
  both sizes.
- **A run stopped, and a mutant left applied** (finding): the first whole
  run of the script hung at its fifth mutant, "a render is kept past its
  end": the leaked render's read transaction held its tables, and a served
  feed test's schema, dropped at its end, waited for it for good (under the
  strict runner a command past 1200 s is no kill, and stops the run).
  Stopped by SIGTERM to the script and its cargo group killed, the mutant
  stayed applied; restored from the commit, `mutation_anchors.py` checked.
  The control now runs in a `unit` suite, the merge's own tests (killed in
  7 s); "an idle connection is never taken again", which piles up a
  connection per render toward the shared server's `max_connections` of
  100, in a `bound` suite, the bound's own test (27 s); and a test's host
  waits 5 s for a seat, a deployment's 30 s, so "a seat is never given
  back" fails in seconds (139 s for its suite).
- **A flake, not this track's**: `the_store_sends_its_slots_after_its_own_content`
  holds the store page's shell to 150 ms. On PostgreSQL under this load it
  failed on master too: twelve pairs run alternately, master 1 of 12
  (215 ms), this branch 2 of 12 (176 and 171 ms). Not loosened.

## Not claimed

- **Streams** (`delivery streamed`) are read after the render, each in its
  own reads; **a keyed read's binding** (ADR-0152) too, the page's other
  bindings, read with it, one render.
- **A render whose read fails** may see two snapshots, the one before the
  failure and the one after.
- **Several hosts on one database** (ruling 1): commands then run
  concurrently, and a render may see 13.2.2's anomaly.
- **What decision 5 leaves open**: an undeclared change, the moment between
  a commit and its invalidation, a materialization's kept entry.
- **The feed's own pool**, unbounded (finding).
- **The first case** (one command's repeated reads): the composites move
  above the seam (ADR-0337's step 4).

## Next

- Bound the feed's pool as the store's is (`CONNECTIONS`), or give both
  layers one pool type; then a host's total is stated for every layer.
- The commit-to-invalidation moment, for every reader: stamp a command's
  resources before its commit, not after, if a measurement shows it matters.

## Questions for the integrator

- `RENDER_CONNECTIONS` = 8 and `SEAT_WAIT` = 30 s are chosen, not measured
  against a production load; a deployment setting for both?
- The table's and a turn's `lock` panic in a render in tests only (a
  structural rule checked by the whole suite): keep, or also in debug
  builds of the host?

## Merge notes

- `just e14-render-merge` joins NEEDS_DATABASE (`scripts/ci_plan.py`); its
  seconds are measured (`scripts/ci_seconds.json`).
- pw-resource gains `epoch` and `fetch_cancellable_as_of`; `fetch` and
  `fetch_cancellable` are unchanged.
- Six other tracks' scripts are re-anchored (27338ab1), each run whole
  (Evidence); their recipes' evidence files are not regenerated here.
- The commits after the merge with train-9 were rewritten before any push
  (the pre-rewrite head is the local tag `render-merge-before-squash`, the
  same tree).
