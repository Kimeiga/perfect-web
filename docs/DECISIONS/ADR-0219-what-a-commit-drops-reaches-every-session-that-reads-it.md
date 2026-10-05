# ADR-0219: what a commit drops reaches every session that reads it

Status: accepted under the owner's delegation of 2026-10-02. It is the feed
reference app's second step (ADR-0218, NEXT 24). Date: 2026-10-05.
Milestone: E14.

## Context

- **A commit told only the session that made it.** `command_answered` sent
  the committing session's documents what changed (ADR-0145, ADR-0161). A
  query's answer the commit dropped for everyone, such as the feed's
  `Timeline`, which listens for `Posted(_)`, was read again only by the next
  page another reader loaded.
- **The feed needs it.** The owner's scope for the feed lists "new posts
  reaching every open reader". A timeline that shows the author's own posts
  live and everyone else's on reload is not a live feed.
- **The store never met it.** Every command it has emits an event keyed by
  the session (`CartChanged(current_session())`) on a query cached
  privately. What such a commit drops is the session's alone. The menu,
  which every reader shares, changes through the store's own broadcast
  (ADR-0178), not through a command.

## Research

- **Convex** re-runs a subscribed query when a committed write meets what
  that query read. Its documentation ([How Convex
  works](https://stack.convex.dev/how-convex-works)) says the read set
  "precisely records all of the data that a transaction queried". A
  subscription manager holds every subscription's read set and walks the
  transaction log. When it finds an intersection, "it pushes a message to the
  appropriate sync worker session, which then reruns the query". The
  subscription is told apart from the mutation's own reply.
- **Here the reads and the writes are both declared:**
  - what a page reads, by its plan's bindings and streams (ADR-0190);
  - what a commit makes stale, by `invalidates_on` and `invalidates`
    (ADR-0208, ADR-0209).

  The host needs no read-set tracking. A commit's dropped cache entries are
  its write set, at the level of a query. A page's bindings are its read set,
  at the same level.

## Decision

- **A commit's drops say which sessions may hold stale answers.**
  `invalidate_queries` returns each query it dropped whole, or by a key its
  sessions share. An entry keyed by the session (`entry_key`'s `session=`
  part, for a private or non-public query) is that session's alone, and
  reaches no one else.
- **Every other session whose open document reads a dropped query is read
  again and sent what changed.**
  - The reads are the document's page plan's bindings and streams.
  - Each such session is told in its own hold (ADR-0172), at the host's
    clock, against its documents' entry (`session_documents`, ADR-0218).
  - The browser holds versions per entry, so that entry's frames order
    themselves.
- **The author is answered first.**
  - A commit queues its telling (`telling`).
  - The connection that committed tells it after the answer is written and
    the connection is closed (`handle`, then `tell_waiting`).
  - A post's answer does not wait on every reader, as Convex's mutation does
    not wait on its subscriptions.
  - The author's own documents are still sent within the commit's hold, so
    the author reads the write.
- **A session told late is told the latest.**
  - Each telling reads the session when its hold is taken.
  - Two commits told in either order leave a reader showing both.
  - A document being served while a commit is told is read again by the
    serve path's own check: a frame reached it after its read began
    (ADR-0151).

## Acceptance

- **The server's tests:**
  - `a_post_reaches_every_open_timeline`: two sessions open the feed's home
    page and one posts. The other's document is sent nothing before the
    author is answered, and the post when the telling runs.
  - `a_post_over_http_reaches_another_reader_after_its_answer`: the same
    over a real connection, through `handle`, answered `202`.
  - `a_page_reading_nothing_dropped_is_told_nothing`: with `Thread` listening
    for likes alone, a post sends another session's open thread page nothing.
  - `a_sessions_own_change_reaches_no_other_session`: the store's
    `add_to_cart` tells its own cart page, and another session's is sent
    nothing.
- **`scripts/cross_session_mutations.py`: 8 mutants.** Each must be killed:
  - no other session told;
  - a session's own entry reaching the others;
  - a whole query's drop reaching no one;
  - every open page read again;
  - the committing session told twice;
  - the author waiting for every reader;
  - a queued telling never run;
  - a connection that tells no one.

  `just e14-cross-session` records them.
- **The server's 127 tests; the workspace; the browser suite, 728 in three
  engines, unchanged.** The store's commands drop only what is the session's.
  - The structural test that a command runs its compiled component reads
    `command_held` now.
  - Three older mutants re-anchored: the session's hold
    (`cart_lines_mutations.py`), a refusal's answer
    (`command_answers_mutations.py`) and an invalidated entry's drop
    (`command_invalidations_mutations.py`).

## Not claimed

- **Precision by key.** A query dropped by a shared key, such as `Thread`
  by the liked post's id, reaches every session whose page reads `Thread`,
  whatever post it shows.
  - Such a page is read again and sent no change.
  - Its own entry was not dropped, so it reads from the cache.
  - The cost is one derive per open page of that query, not one data-layer
    read.
- **The feed in browsers.** Two browser contexts, one posting and one
  watching, come with the feed's browser step: its build, its server and a
  Playwright spec on their own ports.
- **The menu.** The store's shared menu still changes through its
  broadcast, not through a command and this path (ADR-0218's shared
  fragment, not yet generalized).
- **A reader that is not open.** A session with no open document is told
  nothing; its next page is read from the data.
