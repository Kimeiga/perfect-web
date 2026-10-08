# ADR-0275: a row shown before the server answers waits

Status: accepted under the owner's delegation of 2026-10-02, as the integrator
ruled at notifications' merge (ADR-0274, Found). Date: 2026-10-08.
Milestone: E14.

## Context

- **A post is shown before the server answers** (ADR-0222): `pending()`
  puts a row first in the timeline, by "You", keyed `PostId("pending-{n}")`.
  The row had the server's row's controls. Its author linked to
  `/post/pending-{n}` and its handle to `/user/you`, which no server has, and
  its Like and Delete acted on its id.
- **A like pressed on it found no such post.** W3's browser suite once
  pressed Like on Ada's post while it was still that row, and the like
  counted nothing ("0 likes", Chromium, once; ADR-0274, Found). A reader
  pressing that fast meets the same.
- **The integrator's ruling at notifications' merge.** This is not a runtime
  rule: disabling a speculated row's controls would break the store, whose
  speculated cart line carries a real item id. It is the program's own,
  since `pending()` makes the id.
- **The links passed the checker.** PW5009 checks a link against the
  routes by its pattern, and `/post/{p.id}` matches `/post/{id}`, so a link
  the checker passes named a post no server has while the row waited.
- **Prior art.**
  - React's `useOptimistic` (react.dev, read 2026-10-08) marks an
    optimistic item `pending: true` in its reducer and shows it with
    "(Adding...)" until the server's list replaces it; its delete example
    marks an item `deleting`.
  - The HTML Living Standard (§4.5.1, the `a` element): an `a` with no
    `href` "represents a placeholder for where a link might otherwise have
    been placed". It is interactive content only with an `href`.

## Decision

1. **A row the page made waits.** `fn waits(id: PostId) -> Bool` is true for
   an id the page made, `pending-..`: `pending()`'s, and `replied()`'s. The
   page made the id, so the page knows it; no server's id begins so (the
   feed's are `p{n}`).
2. **While it waits, nothing on the timeline's row links or acts.**
   - Its author and its handle are placeholder links, an `a` with no `href`.
   - Its Like and its Delete are disabled.
   - The server's row, in its place once the server answers, links and acts.
3. **The id is read as text by `id.value`**, an opaque type's inside in its
   own module, as `text.value` reads a `PostText`. An interpolation, `"{id}"`,
   was refused by the speculation module at build: an opaque type's value
   has no text form there.

## Acceptance

- **`e2e/feed.spec.mjs`, "a post shown before the server answers waits, and
  acts once it is the server's", in three engines.**
  - With the post held at the network: the row's author and handle have no
    `href`, and its Like and Delete are disabled.
  - Released: the server's row links to `/post/p{n}` and to its author's
    page, its Delete is enabled, and a Like pressed on it is counted. The
    count is the server's too: it stands after a reload.
- **The feed's suite whole, in three engines**; and notifications', whose
  comment on the row it waits for now names this ADR.
- **`scripts/waiting_rows_mutations.py`, 7 mutants**: every row waits; none
  does; a reply's waits and a post's does not; a waiting row's Like acts;
  its Delete acts; its author links; its handle links. Recorded by `just
  e14-waiting-rows`.

## Not claimed

- **A reply shown before the server answers still links to itself**,
  `/post/pending-reply-{n}`. The thread's replies are a view that contains
  itself (ADR-0203), which computes no value in its template yet (ruling
  0073-a), and `feed.sh` refuses `waits(post.id)` there. A pending reply has
  no button.
- **Ids the client makes and the server accepts** (queued at ADR-0274).
  With them a row would not wait: a like pressed on a post not yet committed
  would be ordered after its post by the session's turns (ADR-0271).
- **No "Sending..." text**: a waiting row looks like the server's but for
  its disabled buttons and its links.
- **Only the home timeline shows a row the page made**: `post` speculates
  on `Timeline` alone, and the Following timeline and a user's page show the
  server's rows.

## Alternatives

- **A field on the row, `pending: Bool`, as React's**: the host's `Item` and
  `Post` would carry a word only the page's own rows set, in both of the
  feed's data layers and on the wire, for every row. The id already says it.
- **The runtime disabling a speculated row's controls**: refused at
  ADR-0274's merge. The store's speculated cart line carries a real item id,
  and its controls act.
- **Hiding the row's controls while it waits**: the row would change shape
  when the server's row replaces it; disabled controls keep its place.
