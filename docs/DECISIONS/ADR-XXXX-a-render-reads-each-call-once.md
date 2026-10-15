# ADR-XXXX: a render reads each call once, in one snapshot

Status: proposed by the render-merge worker, NEXT 4c's first step (track
`render-merge`), under the owner's delegation of 2026-10-02. Date:
2026-10-14. Builds the second case of W9's design note, ADR-XXXX "a read is
bounded by construction" (track `bounded-reads`, its section "The second
case: one render's identical calls (the integrator's)", measured in its
"Measurements (4b)"), which the integrator accepted on 2026-10-14 as the
first step of 4c. Builds on NEXT 4b's ADR-XXXX "a read is a bounded number
of statements on PostgreSQL" (the `Counted` connection and its `Tally`),
ADR-0218 (a host serves any program; its data is the deployment's),
ADR-0246 (a layer's database held to what its sources state), ADR-0303 (one
query with one key read once for its document), ADR-0304 (what a read holds
until) and ADR-0127 (a query's declared policies).

## Context

- **One render called one operation six times.** After `place_order`'s
  commit the host reads the session's documents again: the drain reads the
  cart for the session's entry (`drain_held`, `part_text`), then every open
  document is read (`send_documents_where`). Every store page's layout binds
  `Cart(current_user())`, and ADR-0303 reads one query once per document,
  not per render, so `user-carts#current` ran once for the drain and once
  for each of five documents: 6 calls of 21 statements (4b's tally).
- **Each call read its own snapshot.** A query's reads (`DataLayer::reads`)
  took the store as it was when the query ran, in memory; on PostgreSQL the
  store opened a read transaction per call, and the feed ran each statement
  on its own. Two bindings of one page could straddle a commit: the cart
  page's `Cart` read before another writer's commit and its `Checkout`
  after it showed a cart and a total that never existed together.
- **The settled rules** (the integrator's ruling, 2026-10-14): within one
  render the host makes each identical operation call once, the operation
  and its arguments as the key, and every binding that asks gets the same
  answer; nothing is kept past the render, so no invalidation model is
  needed; the render's calls run in one read transaction; the control, one
  operation with other arguments, still reads twice; on both layers, memory
  and PostgreSQL, the isolation chosen from PostgreSQL 18's own documents.
- **PostgreSQL 18's isolation levels** (Transaction Isolation, read
  2026-10-14, and SET TRANSACTION):
  - 13.2.1, Read Committed: each statement sees a snapshot as of its own
    start, so "two successive `SELECT` commands can see different data".
  - 13.2.2, Repeatable Read: a statement sees "a snapshot as of the start
    of the first non-transaction-control statement in the transaction", so
    successive statements see the same data; and "read-only transactions
    will never have serialization conflicts". Its limit: the view "will not
    necessarily always be consistent with some serial (one at a time)
    execution of concurrent transactions", the documents' example a
    read-only transaction that sees a batch's control record completed and
    not one of its detail records.
  - 13.2.3, Serializable, and SET TRANSACTION: a `SERIALIZABLE READ ONLY
    DEFERRABLE` transaction "may block when first acquiring its snapshot",
    and then runs without a serialization failure; its data alone is
    "known to be valid as soon as it is read".

## Decision

1. **A render** is the host reading what a session's documents show at
   one moment, on one thread:
   - a page's bindings (`bindings_where`): a page served, a part's binding
     (`part_text`), a keyed read's other bindings;
   - a session's documents read again (`send_documents_where`), after a
     commit, a store's change or a clock's instant;
   - the drain after a commit (`drain_held`): its read of the cart and the
     documents' read after it, one render, so the entry's value and what its
     documents show are of one snapshot.
   One begun inside a render of the same layer for the same session is that
   render. One for another session is its own: a command that commits
   inside a render (a test's hook does) reads what it committed in its own.
   A read for no session inside a render (a materialization's, which is
   public) is the render's. A render is the host's alone
   (`render_merge.rs`, a thread's stack of renders); no program changes.
2. **Each identical call once.** In a render, a call of a data layer's
   operation is answered from what the render already read when the render
   made the same call, for the same session, with equal arguments
   (`Val`'s equality); otherwise the layer is called and its answer kept
   until the render ends. Only answers are kept: a call that failed is made
   again by the next binding that asks. Only the layer's operations are
   merged: the platform's (the session, the principal, the clock) are
   answered by each query, since the clock's note what a read holds until
   (ADR-0304), per document. `/metrics` and `/bench/calls` count the calls
   the layer ran, as they did; a call answered again is not one.
3. **One snapshot.** A layer gives a render its snapshot
   (`DataLayer::snapshot`, at the render's first read) and the reads of
   each of its queries over it (`DataLayer::reads_in`); by default none,
   and each query reads as before (kiokun's files, which never change).
   - **In memory** (the store's and the feed's): the data as its last
     commit left it, each part behind its `Arc`, so nothing is copied.
   - **On PostgreSQL** (the store's and the feed's): one transaction,
     `BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY` (`feed_pg.rs`'
     `READ_SNAPSHOT`), opened at the render's first statement and
     committed when the render ends. Repeatable read because every
     statement of the render then reads the snapshot its first statement
     took (13.2.2), where read committed gives each statement its own
     (13.2.1); read only because such a transaction never meets a
     serialization failure (13.2.2), so a render is never refused and never
     retried. Not `SERIALIZABLE READ ONLY DEFERRABLE`: it may block at its
     first statement until the serializable writers running then end
     (SET TRANSACTION), which would hold every render behind every other
     session's command, and what it adds, freedom from the anomaly 13.2.2
     describes, needs writers that run concurrently. A host runs its
     commands one at a time (each layer's `writes` lock, from a command's
     first write to its commit), so every snapshot one host's render takes
     is of a serial history.
   - **A read that fails ends the transaction** (it may have aborted it,
     and every statement after would fail), and the render's next read
     opens another.
   - **A render's connection never waits** on the store pool's bound of
     eight. A render holds its connection from its first read to its end,
     and meanwhile may wait for a read another thread is making
     (pw-resource's one flight per key); a render that waited for a
     connection could wait on renders that wait on it. So the pool lends a
     render an idle connection or opens one past the bound (`lend`), and
     takes it back as idle only while the connections out and idle stay
     under the bound (`keep`), since a command or a single read takes an
     idle one without waiting. The feed's pool has no bound.

## Measurements

On PostgreSQL 18.6, 2026-10-14, by 4b's tally (`tests/bounded_statements.rs`,
each count equal at the seed and with fifty stores more):

| `place_order`, taking no payments: its queries read again | before | after |
| --- | --- | --- |
| user-carts#current (`Cart`): calls | 6 | 1 |
| statements | 21 | 16 |

The 16, by operation: `user-carts#current` 1 (1 call), `hours#for-cart` 6,
`checkout#parts` 2, `fees#for-stores` 1, `checkout#chosen` 1,
`user-orders#current` 2, `coverage#for-store` 2, `addresses#list` 1, each one
call. Every other count 4b pinned is unchanged: no other page or command
there calls one operation twice with the same arguments in one render
(each page is one render already, and its queries call distinct operations
or distinct arguments). In scenario (a), `place_order`'s 61 (33 + 7 + 21)
becomes 56 (33 + 7 + 16); the design note's 51 needs its first case too,
the composites moved above the seam.

## Alternatives

- **Merge queries, not operations, across a render's documents**
  (ADR-0303's read once per document, for the render). Saves running a
  query's component again too, but a query's run notes what its document
  holds until (ADR-0304), per document, and the rule is the operation's:
  two different queries calling one operation are merged here as well.
- **A cache kept past the render**, by session. Each commit would have to
  say which kept answers it makes stale: the invalidation model the ruling
  does without.
- **Merge the platform's calls too.** The clock's answer is for one
  instant, and its call notes what the read holds until; answered again, a
  second document's read would note nothing and not be read again when the
  instant passes (`a_renders_queries_each_note_what_the_clock_holds_them_until`).
- **Serializable, deferrable, read only**: above.
- **A savepoint around each read**, to keep the snapshot after a failure:
  two round trips more for every read, for a failure that already makes
  its binding's page unshowable.
- **Renders take their connection from the bound**, eagerly at their
  start, with every flight's reader inside one: deadlock-free only if no
  read ever runs a flight outside a render (a stream, a keyed read), and a
  connection taken for every render, cached or not.

## Acceptance

- `render_merge.rs`' own tests (7): identical calls once, other arguments
  each, another session's call its own, a failure made again, one snapshot
  and nothing kept past the render, a render inside another its for its
  session alone, one layer's render answering for no other's.
- `tests/render_merge.rs` (14), the store in memory and on PostgreSQL
  (`PW_STORE_TEST_LAYER`), the feed on PostgreSQL: a commit's documents
  read in one render, the cart once and each store's reach for its own;
  the feed's two timelines read once; another writer's commit between two
  bindings' reads seen by neither, on the cart page and on the feed's
  notifications page, and the controls, two renders that straddle it; each
  layer's snapshot (store and feed, memory and PostgreSQL); a failed read
  ending the render's transaction (store and feed); a render's connection
  not waiting on the bound, and the control, a single read that waits; the
  clock's calls never merged.
- `tests/bounded_statements.rs` re-pinned: the re-read's 21 statements
  16, and a row of its own, `user-carts#current` called again, 1.
- `scripts/render_merge_mutations.py`, 22 mutants, one for each rule, each
  killed: `just e14-render-merge`.
- The server's suite unchanged otherwise, in memory and with the store on
  PostgreSQL.

## Not claimed

- **Streams** (`delivery streamed`) are read after the render, each in its
  own reads, as before; **a keyed read's binding** (ADR-0152) too, the
  page's other bindings, read with it, one render.
- **What pw-resource answers without a read**: a value its cache keeps, or
  a flight another thread began, is as old as the query's policy lets it
  be, not the render's snapshot.
- **A render whose read fails** may see two snapshots, the one before the
  failure and the one after.
- **Several hosts on one database**: commands then run concurrently, and a
  render may see the anomaly 13.2.2 describes; `DEFERRABLE` would rule it
  out, at the cost above.
- **Connections**: the store's pool holds eight for commands and single
  reads, and one more for each render reading at once.
- **The first case** (one command's repeated reads): the composites move
  above the seam, the design note's decision 3; not this step.

## Questions for the integrator

- A render's PostgreSQL connection is lent past the store pool's bound
  (decision 3): the conservative choice, as a render that waited could
  deadlock, and cheap to reverse; the cost is a connection per render
  reading at once beyond the eight. Should the bound be one the renders
  share instead, with every flight read inside a render?

## Merge notes

- `just e14-render-merge` joins NEEDS_DATABASE (`scripts/ci_plan.py`).
- NEXT 4c: its first step, the render merge, is built; the design note's
  "The number" for the second case is measured (21 -> 16, 6 -> 1).
