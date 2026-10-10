# Parallel tracks

How more than one session builds Pleris at once (ADR-0253; charter risk R10:
"One integrator; separate worktrees; non-overlapping assignments; mandatory
charter+ADR reading (`AGENTS.md`); small merges; full gate tests").

## Who does what

- **The integrator** (the session working on `master`) alone merges to
  `master`, numbers ADRs, edits the four status documents (`docs/STATUS.md`,
  `docs/NEXT.md`, `docs/DECISIONS.md`, `docs/KNOWN_LIMITATIONS.md`), runs the
  chain, pushes `master`, and rules on anything that crosses tracks. It also
  keeps a track of its own: the follows timeline (ADR-0195's ruling 10).
- **A worker** builds one track, in its own worktree, on its own branch, in
  the files its track owns. Everything in `CLAUDE.md` and `AGENTS.md` holds
  for a worker as for the integrator: evidence from a recorded command,
  dependency versions and APIs checked against primary sources, no secrets,
  nothing published.

| track | branch | worker |
|---|---|---|
| accounts and sign-in: a `requires` evaluator, sign-up, sign-in and sign-out, per-user sessions, an OIDC-style deployment interface with a local provider that is plainly not production | `track/identity` | W1, a local session; merged 2026-10-07 (ADR-0258) |
| image uploads on a post: a typed upload with size and content-type limits, a deployment's blob-storage capability, served safely | `track/uploads` | W2, a local agent of the session that runs W1; merged 2026-10-07 (ADR-0260) |
| notifications: private, per-user live data from others' actions (a like, a reply or a follow involving you), an unread count and mark-read, on a typed principal | `track/notifications` | W3, a local agent of the session that runs W1 and W2; merged 2026-10-08 (ADR-0270, ADR-0274) |
| direct messages: a conversation private to its two users, a list of them with unread counts, a conversation page that sends, live to both, to someone who follows you or has messaged you | `track/messages` | W4, a local agent of the session that runs W1 to W3 |

The follows timeline landed on 2026-10-07 (ADR-0257), and notifications
on 2026-10-08 (ADR-0270, ADR-0274); direct messages are W4's, under the
rulings below.

## What each track owns

| | identity | uploads | notifications | messages |
|---|---|---|---|---|
| recipes | `just/identity.just` | `just/uploads.just` | `just/notifications.just` | `just/messages.just` |
| diagnostic codes | `PW55xx`, `Owner::Identity` | `PW56xx`, `Owner::Uploads` | `PW57xx`, `Owner::Notifications` | `PW58xx`, `Owner::Messages` |
| the development server | `spikes/own-renderer/server/src/identity.rs` | `spikes/own-renderer/server/src/uploads.rs` | `spikes/own-renderer/server/src/notifications.rs` | `spikes/own-renderer/server/src/messages.rs` |
| new files | anything new under a directory or name the track's own: `identity`, `accounts`, `sign_in` | `uploads`, `blob` | `notifications`, `principal` | `messages`, `conversation` |
| browser hosts | `PORT+70..72`, `IDENTITY_PORTS` | `PORT+80..82`, `UPLOADS_PORTS` | `PORT+90..92`, `NOTIFICATIONS_PORTS` | `PORT+100..102`, `MESSAGES_PORTS` |
| PostgreSQL migrations | `0003` | `0005` | `0006` | `0007` |

- **Recipes**: the root `justfile` imports each track's file, whose recipes
  run in the repository's root under the root's settings and `PATH`. An
  evidence recipe is `e14-<name>:` and writes `docs/evidence/E14/<name>.txt`;
  the verification run plans it like any other (`scripts/ci_plan.py` reads
  every part of the justfile).
- **Codes**: a track registers codes only in its block, with its owner; a
  test holds each block to its owner (`codes.rs`,
  `a_tracks_block_holds_only_its_tracks_codes`). A code another track's
  change needs is the integrator's to register.
- **`main.rs`**: a track touches it only at the lines marked `TRACK SEAM`
  for that track, and where it must, in a few more lines marked the same
  way: `// TRACK SEAM (identity): ..`. The seams:
  - `Identity::answer(method, route, headers, session, fresh, body,
    stream) -> bool`, asked for every request after its body is read and
    before any other route: the identity track's own routes.
  - `Identity::requires(predicate, session) -> Result<bool, String>`, asked
    by `Server::run` for each predicate a command's `requires` names. Until
    the track lands, the development model: a session is `SignedIn`, and an
    unknown predicate is an error.
  - `Uploads::claims(method, route) -> bool`, asked for every request
    before its body is read, and `Uploads::answer(route, headers, session,
    fresh, reader, stream)` for one it claims: an upload reads its own body,
    within its own limits, where a command's is bounded to 64 KiB.
  - Each module's state is a field of `Server`, `identity` and `uploads`,
    built by `Default`; a track grows its own struct.
  - `pw:host/principal#read` (W3), answered beside `pw:host/session#read`
    where a query's and a command's host operations are built, from the
    identity's principals.
  - `Identity::holds` (W4): the predicate `MayMessage(to)`, beside
    `OwnsPost(post)` and evaluated as it is, at lines marked `TRACK SEAM
    (messages)`.
- **Shared files** (the compiler, the runtime, the platform packages, the
  examples): a track may change them where its work needs, in small,
  separate commits whose messages say why, and says so to the integrator
  first when the change is to a rule other code relies on.

## Branches, commits and ADRs

- Branch `track/<name>` from `master`, in a worktree of its own, with its
  own `CARGO_TARGET_DIR`.
- **An ADR is unnumbered**: `docs/DECISIONS/ADR-XXXX-<slug>.md`, its title
  `# ADR-XXXX: ..`. The integrator numbers it when it merges, and indexes it.
- **A worker never edits the four status documents.** What it would write
  there goes in its ADR: what was done, what is claimed and not, and what to
  do next.
- **Rebase on `master` before pushing**, and push only the track's branch:
  never `master`, never another branch, no release, no PR unless the owner
  asks for one.
- **Every push is checked**: `ci` runs on every branch, and `verify` runs on
  a track's branch the recipes its commits changed since `master`
  (`scripts/ci_plan.py --changed <merge base> <head>`, ADR-0253). A track is
  ready to merge when both are green at its head.
- **A merge is small**: the integrator merges a track when a piece of it is
  done and green, rebasing it onto `master`, numbering its ADRs, writing the
  status documents, and running the chain.

## Questions for the integrator

A question that crosses tracks, or changes a rule other code relies on, is
the integrator's to answer:

- **W1** sends it by message (`SendMessage` to the integrator's session).
- **W2**, a cloud session that cannot message, writes it in its branch's ADR
  under a heading `## Questions for the integrator`, and pushes; the
  integrator answers in a commit to `master` the worker rebases onto, or in
  the ADR's own section, `## The integrator's answers`.

## What the language already holds a track to

- **An affine resource is held or refused** (ADR-0250, ADR-0251): an
  acquisition, an upload's handle as much as a transaction, is bound to a
  name, ended where it is made, returned to a caller, or held by a resource's
  `acquire` clause, whose `release` ends it exactly once on every path.
- **A value's privacy label follows it** (ADR-0129, ADR-0252): through
  calls, branches, assignments and early returns; a session's principal and
  anything read with it are private to it.

## The integrator's rulings

Each a decision for a track, with its date; a track's ADR records it too.

- **2026-10-07, identity (W1's plan).** Pleris owns the relying party:
  state, nonce, PKCE S256, the callback, the session it opens, its cookies,
  CSRF and the `requires` evaluator; a deployment owns the provider behind
  an `IdentityProvider` interface. A development provider refuses to start
  unless the deployment says development and its origin is loopback.
  - `Guest`, every session its own guest principal, stays the default and is
    named and development-only like the rest; a deployment opts the feed
    into accounts. Flipping the feed's default is the integrator's ruling
    once the track merges.
  - A `requires` refusal is answered as its own case, 403 with
    `{"committed":false,"refused":"<predicate>"}`, never as a declared
    error. The runtime is unchanged: a page hides what its viewer cannot
    do, and a stale tab gets the runtime's failure (to queue in NEXT).
  - No compiler change: the host resolves the principal from the session.
    A typed principal in the language is a language ADR, the integrator's.
  - `DataLayer::identified_by`, defaulted, is the one change to the trait.
  - Session cookies are `HttpOnly`, and `Secure` off loopback; a session
    id is 128 random bits from the OS.
  - CSRF, in `Identity::answer` for every unsafe request, in both
    providers: Go 1.25's `CrossOriginProtection` (`Sec-Fetch-Site`, then
    `Origin` against `Host`).
  - A new crate is the owner's to approve, in the track's own session; its
    version is checked against its primary source and `just ci`'s
    advisories gate stays green. A crate already in `Cargo.lock` is taken
    at the lock's newest version.
- **2026-10-07, uploads (W2's four questions).**
  - Limits are the program's: an `upload` declaration, parsed and checked
    like `source` and written by `pw build` to `uploads.json`, its limits
    literals, its `types` a closed set, its `route` no page's (PW56xx). A
    deployment may lower a limit, never raise one.
  - A staged upload is an affine handle, claimed and published or discarded
    in the command's body, so PW2005 holds every path; the development
    origin grants `resource.acquire<Upload>` and `resource.release<Upload>`.
  - A post's image is the post's own data: a field of `feed.rs`'s row and
    columns of `feed_pg.rs`'s posts (migration 0005), written in the post's
    transaction. No decorating layer; blobs are the uploads module's.
  - `image: Option<Image>` on `Post` and `Item`, with a required alt text;
    the timeline shows images.
- **2026-10-07, at identity's merge (ADR-0258).** A typed principal, the
  host's routes in the route table and a refusal shown by the runtime are
  queued (NEXT); the database job sets up browsers and the build for a
  recipe that needs them, and `e14-identity` runs there.
- **2026-10-07, at uploads' merge (ADR-0260).** Its four questions are
  answered in ADR-0260: a deleted post's image stops being served and its
  blob is collected (queued first, the integrator's), a runtime that sends
  a file (queued after notifications), `e14-uploads` on the database job
  (done), and every form's `action` checked with the host's routes
  (queued, with identity's).
- **2026-10-07, notifications (W3).** The typed principal is ruled here, a
  language decision, and W3 builds it as its first commits, before the
  notifications on it.
  - **The reader's user is the host's**: `context.current_user()` becomes a
    platform operation the host answers from the session's principal, its
    `user`, or the session's guest where no one signed in, labelled
    `User<UserId>`. `identified_by` goes once nothing reads a user through
    it.
  - **A user's handle is the host's alone to make**: a program constructs
    no `User<..>`, and a data layer answers a user's id (who wrote a post),
    never another user's handle. So a private query keyed by a handle
    serves its reader alone.
  - **An event naming a user reaches each of their sessions**: an event's
    value of a user's id reaches a query parameter of type `User<..>` over
    that id, and the host drops the entries at that user in every
    session's partition (ADR-0256) and tells their open pages (ADR-0219).
    This changes ADR-0091's relation of a listener to its event, a rule
    other code relies on: W3 brings the change to the integrator before it
    lands.
  - **Which type a user's id is** is W3's first question, with research:
    the platform's `capability.UserId` throughout the feed (which
    fabricates `UserId("you")` for an optimistic author today), or the
    program's own type, named by the principal it reads; with what the
    compiler holds of generic opaque types and of `User<U>`'s
    representation.
  - **A notification is a row** that a like, a reply or a follow writes in
    its own transaction, beside what it writes, as an image is its post's
    (ADR-0260). No materialization: materializations made real stay
    queued, the integrator's. One's own act notifies no one, and a deleted
    post's notifications go with it.
  - **Private to its user, by cache and by session** (charter §15.6): its
    queries keyed by the reader's handle, `cache private`; a test holds
    another user's session, signed in and not, to seeing none of it, by
    page and by `/pw-read`.
  - **An unread count on each page that shows its reader**, live, and a
    page of them; reading them is a command that invalidates the reader's
    count and list, which reaches each of the reader's sessions.
- **2026-10-08, at notifications' merge (ADR-0270, ADR-0274).** W3 stopped
  at the account's weekly limit after its last push, its work complete; the
  integrator merged it from its tip, f20ed75. Its three questions:
  `e14-notifications` runs on the database job (`NEEDS_DATABASE`); the root
  justfile keeps its import line, as identity's and uploads' are; and
  `identity.rs`'s test-only `signed_in_for_test` stays. Queued from its Not
  claimed: telling by principal (an event naming a user reads again every
  open page of the query), `identified_by` gone (the feed's reads by
  session moved to `current_user()`), and a stream's `current_user()`. Its
  finding, a speculated row's Like acting on an id no server has, is the
  feed's to fix (NEXT), as ruled.
- **2026-10-08, direct messages (W4).** A conversation is two users'
  private data. Nothing in the language holds a value private to two yet,
  and nothing needs to: each reads it as their own.
  - **A conversation is read from one side.** Each participant reads it
    through a private query keyed by their own handle and the other's id:
    `Conversation(reader: User<UserId>, with: UserId, ..)`, `cache
    private`, `key reader, with`. The same rows are read twice, each read
    private to its reader, as a notification is its user's (ADR-0274). A
    label naming two principals, a value either may read, would need a
    reader set in every label and a check at every flow; no read here is
    on two users' behalf, and the host binds the one it is on. A program
    makes no handle (ADR-0263), so a session reads only its own side.
  - **A list and a count on the same footing**: `Conversations(reader)`,
    newest first, each with the other user, the last message and its
    unread count; `UnreadMessages(reader)`, "Messages: N unread", on each
    page that shows its reader, as notifications' count.
  - **An event naming both reaches each of their sessions**: `send` emits
    `Messaged(from, to)`. The conversation listens `invalidates_on
    Messaged(reader, with), Messaged(with, reader)`, and the list and the
    count `Messaged(reader, _), Messaged(_, reader)`: ADR-0270's listener
    rule binds each `User<..>` parameter to the event's id, so the sender's
    other sessions and the recipient's are told, and no third user's entry
    is dropped. Telling only their sessions waits with telling by
    principal (NEXT).
  - **Who may message whom is X's rule**: the recipient follows the
    sender, or has sent the sender a message before. X's help center
    ("About Direct Messages", read 2026-10-08) lets one start a conversation
    "with anyone who follows you", and lets one you do not follow message
    you where you have messaged them before. Its opt-ins (messages from
    anyone, from verified users) are not claimed. No one messages
    themselves.
    - `send` `requires SignedIn, MayMessage(to)`, evaluated by
      `Identity::requires` through the command's own operations, in its
      transaction, as `OwnsPost(post)` is. A refusal is the 403
      `{"committed":false,"refused":"MayMessage"}`.
    - A page shows the composer only where its reader may send: a value
      the conversation's query carries, read again on `Followed(with,
      reader)`, `Unfollowed(with, reader)` and `Messaged(with, reader)`.
      Where the reader may not, the page says why.
  - **A message is a row of its own**, in both of the feed's layers
    (migration `0007`): its sender, its recipient, its text and its time,
    written in the command's transaction; and a read mark per reader and
    other user. A message is not a post, and writes no notification; its
    count is its own.
  - **Reading is a command**: `read_conversation(with)` sets the reader's
    mark, and invalidates the reader's conversation, list and count, in
    each of their sessions. Sending marks the conversation read for its
    sender. A page runs no command when it is shown, so the conversation
    page has a "Mark read" button, as notifications' page has; a command a
    page runs when it is shown is the integrator's, queued.
  - **A message is shown before the server answers** (ADR-0222): the
    conversation speculated on, `optimistic Conversation(current_user(),
    to, _)`, the message last and keyed by an id the page made,
    `pending-..`. Anything on it waits until the server's row stands in
    its place, by ADR-0275's rule; a conversation is a flat list, so a
    link waits too, where ruling 0073-a stops a reply's.
  - **Its text**: `MessageText`, 1 to 10,000 code points, refused past
    them as a post's text is (ADR-0225). 10,000 is X's limit since August
    2015, as reported; W4 checks it or states it as its own.
  - **A third user, signed in and not, is held by test**:
    - by page: `/messages` lists only their own conversations, and
      `/messages/{a}` shows only their own with `a`, never a message
      between `a` and `b`;
    - by `/pw-read`: a read of `Conversation`, `Conversations` or
      `UnreadMessages`, for any key the browser asks, is answered from the
      session's own principal, never from what the request names; and
      another session's page, asked for by its number, is no page of
      theirs: superseded or refused, as notifications' tests hold;
    - by the cache every reader shares: none of the three is in it
      (`cache private`); and by session, every entry read is keyed by its
      session's own user;
    - by stream: while `a` messages `b`, a third user's open conversation
      and list are sent no frame holding the text;
    - and a guest, who may not send (`SignedIn`), reads nothing of anyone
      else's.
  - **Not claimed, by ruling**: blocking, which on X ends a one-on-one
    conversation with the account blocked and which reaches follows,
    timelines and notifications too, queued after W4 as the integrator's;
    group conversations; message requests and messages from anyone;
    deleting a message, which X does for the one who deletes it alone;
    editing; read receipts; images; search.
  - **In order**: the rows, in memory and on PostgreSQL, and the commands;
    then the pages; then live delivery and the third-user tests; then the
    browser suite in three engines and the mutation controls,
    `e14-messages`.
- **2026-10-08, at messages' merge (ADR-0279).** W4 merged from its tip,
  `87dbea0`, rebased on ADR-0277 and ADR-0278; numbered ADR-0279 at the
  merge, as ADR-0274 was. Its finding, the database job past its limit,
  became ADR-0278, under which its recipe ran in a shard of its own. Its
  verification was red only on WebKit's "Load more", NEXT's open item.
  Blocking is the integrator's next of its leftovers.
- **2026-10-08, the DoorDash customer app's tracks (the owner's target,
  relayed 2026-10-08).** One worker at a time beside the integrator, each
  launched here when the slot frees, with its own rulings then; the order
  and why are NEXT's:
  - **W5, the store's data on PostgreSQL**, after W4's merge: the store
    behind the DataLayer seam as the feed is (ADR-0246), with its
    guarantees stated as ADR-0207 states a source's. Disjoint from the
    integrator's soft navigation, which is the runtime's; the integrator's
    accounts in the store wait for its merge, since they change its
    tables.
  - **W6, kiokun.com in Pleris**, beside W5, as the owner chose (the
    owner's production target, relayed 2026-10-08; NEXT): parity with the
    live site, served here first, with no deploy, cutover or DNS change
    without the owner's go. Its rulings are below.
  - **W7, TodoMVC**, in the next slot free (the owner, relayed
    2026-10-08; NEXT): the comparison's first app, an objective suite, and
    the tutorial's first lesson; then, its second milestone, the showcase
    that presents it and the comparison (NEXT). Its rulings are written
    here before it launches.
  - **W8, search and filters**, after W7: run in the database or a declared
    search source; no search engine is built here.
  - **W9, ratings and reviews**, after W8 and the integrator's accounts: a
    review is a user's.
  - **The integrator's**: the soft navigation, the store's accounts,
    delivery addresses, store hours, and checkout with payment, the last
    the integrator's because a payment capability is `secret<Payments>`
    and any crate it needs waits for the owner's approval.
- **2026-10-08, the store on PostgreSQL (W5's plan).** Track `store-pg`,
  branch `track/store-pg`, the store's data behind the DataLayer seam on
  PostgreSQL as the feed's is (ADR-0246), so that each DoorDash gap after it
  is built and tested on both layers. Its own ADR, `ADR-XXXX`, numbered at
  the merge. The rulings, from a map of the store's data (2026-10-08):
  - **Found, first: an order keeps its lines.** ADR-0193 rules that an
    order is the cart's lines and a status, but `orders#place` keeps the
    status alone, and the lines go with the emptied cart. Fix it in memory
    first, with its test, then carry it to PostgreSQL.
  - **All of the store's state through the seam**, in both layers. Today
    much of it bypasses the seam:
    - the menus: store 47's is written by `broadcast_menu` and a test
      route, and store 48's and the catalogue are code;
    - the stock, the recommender and the estimates, the notice, the
      preparation time and the categories, each written by a `/bench`
      route.

    Each becomes a write the layer stages and commits, its events in the
    same transaction. A route that changes data is a command's path, not a
    direct write. The fault hooks stay test controls, which the layer
    applies.
  - **Delivery: a session's cart reaches its pages on PostgreSQL too.** The
    host redraws a session's cart from the materializer's SQLite outbox
    (`drain_held`). A layer whose commit keeps its own outbox writes nothing
    there, and the cart would never be sent. W5 makes the events its commit
    delivers reach the materializer as the feed's do, and tests a cart
    change reaching an open page on PostgreSQL.
  - **Its own schema, migrations and lock** (`migrations/store/`, its own
    version table and advisory lock), never the feed's. Every command runs
    serializable, and `provides()` measures it as the feed's does.
  - **Its guarantees.** The store's `source StoreData`
    (`examples/lib/StoreData.pw`) is held to the database it opens. Its
    comment, "one SQLite database", is corrected. The grants no source
    holds (`Notices`, `Kitchen`, `Categories`) are held by a source or
    stated.
  - **Tests on both layers.** A store test that reads the in-memory
    layer's internals (`cart_value`'s `self.store.carts`,
    `materializer.state("cart:..")`, `s.store.orders`, and the rest) reads
    through the layer instead, so that it runs on both. The server's store
    tests run on PostgreSQL, each in a schema of its own, as the feed's do.
    Among them, on PostgreSQL:
    - a refused commit commits neither its state nor its event;
    - eight sessions adding five times each commit all forty;
    - a retried interaction runs its command once.

    Then parity in memory and on PostgreSQL, and the negative controls.
    The browser suite stays in memory, as the feed's does.
  - **Its recipe and mutation controls**: `e14-store-postgres`, in
    `NEEDS_DATABASE`. The thirteen scripts whose anchors are in `store.rs`
    are re-anchored and run whole. Its own controls include a commit
    without its events, an order without its lines, the outbox not read
    back, and the isolation not measured.
  - **Not claimed**, inherited from ADR-0246: a second host; idempotency
    committed with the writes (PW0348), queued before checkout as the
    integrator's; a measured serialization failure; and the browser suite
    on PostgreSQL.
  - **Disjoint from the integrator's work**: the soft navigation is the
    runtime's, and ADR-0280 changes the store's page and `store-ir.json`
    only, where a rebase meets it. The integrator's accounts in the store
    wait for this track's merge.
- **2026-10-08, W5's five design points (answered the same day).**
  1. **`commit_staged`, one commit path** for commands and the store's
     routes that change data: accepted. A layer that keeps its own outbox
     and has the session's entry (the store on PostgreSQL) records what its
     commit read back into the materializer after the commit. That record
     is the rows and the events in one materializer transaction, the shape
     the in-memory path commits, so `drain_held`, `committed_basis` and the
     cart's entry are as they are in memory. PostgreSQL stays the authority.
     A record that fails after the commit is logged, and the command is
     answered committed. Tested: a cart changed on PostgreSQL reaches the
     session's open page.
  2. **A route that changes data stages a store-only operation** through
     `data.begin` and commits it with the route's declared events in one
     transaction: accepted as "a command's path". The merchant's side is
     out of scope, so these are the host's operations, not the program's
     commands. Tested: a program that names one of them is not linked to
     it, since no grant gives it. The fault hooks stay test controls in
     `store::Faults`.
  3. **`StoreLayer` beside an unchanged `DataLayer`**, and
     `PW_STORE_DATABASE_URL`: accepted, with one change. The store's
     connections set `search_path` to the store's schema alone, never the
     feed's. Its own `outbox`, its own migration table and its own advisory
     lock live there, so no unqualified name can reach the feed's tables.
  4. **`feed_pg.rs`'s helpers made `pub(crate)`**, visibility only:
     accepted. If a shared helper changes behaviour later, the feed's
     mutation scripts run whole.
  5. **The whole server suite on both layers**
     (`PW_STORE_TEST_LAYER=postgres`, each server in a schema of its own,
     dropped with it): accepted, as the stronger evidence. `just ci` runs
     it in memory; `e14-store-postgres` runs it both ways, on CI in its own
     shard.
  - **Its finding, W5's to fix**, since it lies in the commit path it
    factors: `place_order`'s `OrderChanged` is never consumed from the
    materializer's outbox in memory, so each order placed leaves a row for
    good. Every event a commit stages is consumed once delivered. Tested
    in both layers: after orders are placed and delivered, the outbox
    holds nothing.
- **2026-10-08, kiokun.com in Pleris (W6's plan), launched beside W5.** Two
  workers at once is the owner's choice (relayed 2026-10-08), at the cost of
  usage, and no more than two. Track `kiokun`, branch `track/kiokun`, from
  the kiokun slice (ADR-0037, ADR-0041; `examples/kiokun/`). Its ADRs are
  `ADR-XXXX`, numbered at the merge. Its recipes live in `just/kiokun.just`.
  Its code block is **PW60** (`Owner::Kiokun => "PW60"`). Its hosts are at
  PORT+120..122 (`KIOKUN_PORTS`), and its Playwright runs use PORT=7341 (W5
  has 7141; 7241, planned first, overlapped W5's range, corrected
  2026-10-09 below).
  - **The data** is read through `KIOKUN_DATA`, the kiokun-data checkout's
    `output_dictionary` (1,485,890 raw-DEFLATE JSON files, 6.4 GB), as `just
    e10-kiokun` reads it. It is never copied here, and nothing is written
    into that checkout. A rule of kiokun's (a file name's escape, the shard
    rule, the search index's rows) is read from the builder's source,
    `src/main.rs` and `src/search_index_builder.rs`, before it is inferred
    from the output.
  - **First, the parity inventory.** It covers every route and feature of
    the SvelteKit site (`sveltekit-app/src/routes`: `[word]`, `api`,
    `blog`, `category`, `courses`, `custom-words`, `drill`, `frequency`,
    `game`, `homophones` and the rest). Each is marked built in Pleris,
    partial or missing, with what it needs of the language or the platform.
    The inventory is a document and the track's first ADR. The gaps are
    then built in the order the integrator rules from it.
  - **What a gap needs of the language, the runtime or the host goes to
    the integrator as a question.** It is not built around them in the
    track's files. The integrator rules on it, and usually builds it, as
    notifications' typed principal was.
  - **Parity is measured** against the SvelteKit app run here on the same
    data, where it runs with what is installed. Installing its packages is
    a download and waits for the owner. The live site is read only for a
    small sample, and gently.
  - **Served here first.** Any deployment waits for the owner's explicit
    go, asked in the integrator's session: a preview origin among them, a
    DNS change and a cutover. The track holds no credentials for any host.
  - **What needs the owner's local data is local.** A run over the whole
    dictionary is recorded on this machine, and its recipe says so
    (ADR-0281's fifth decision, as `LOCAL_ONLY`'s; this cited charter
    §13.5, which is macOS's concerns, until W6 found it). What a committed
    sample can show,
    as `scripts/kiokun_sample.py` makes one, runs on CI.
  - **In order**: the inventory and its ADR; then the gaps in the order the
    integrator gives; each with its tests, its browser suite in three
    engines, its mutation controls and its recipe.
- **2026-10-08, W6's inventory, answered.** Its order is accepted:
  (1) the word page from the entry; (2) the whole dictionary's search, held
  to `/api/search` with Korean, then the reading index, homophones and
  deinflection in Pleris; (3) the static-data pages; (4) accounts and the
  user's data; (5) browser capabilities; (6) outside services.
  - **Q1, rendering**: yes. Every page is rendered by the server and works
    with scripts off, with script only where a feature needs it, as every
    Pleris page is. Parity is by content and behaviour, not by rendering
    strategy. The live site renders nothing without script (`ssr = false`),
    a difference stated, not matched.
  - **Q2, the host**: the development server, a third program beside the
    store and the feed (ADR-0218: a host serves any program), with a
    read-only kiokun data layer. The slice's GET-only host stays as
    ADR-0037's and ADR-0041's evidence until the rewrite supersedes it.
  - **Q3, the app's own data**: yes, read-only through `KIOKUN_APP` (the
    checkout's `sveltekit-app`), as `KIOKUN_DATA` is. Nothing is copied here
    and nothing is written there. The course modules are converted at build
    time into a local cache git ignores, by a converter committed here: this
    repository is public, and the owner's content stays out of it unless
    the owner says otherwise.
  - **Q4, the search index**: (b), the builder's current output loaded
    into a declared search source, its guarantees stated as ADR-0207 states
    a source's; W8's search for DoorDash builds on it. The committed
    `output_search_index.sql` is older than the builder and lacks
    `jyutping_search`, so the CSV is read. (c) would copy the builder's
    rule into a second place, to drift; (a) stays the slice's until (b).
  - **Q5, the clone**: yes. A copy-on-write clone of `sveltekit-app` in the
    track's scratchpad, run there with what is installed, with no
    credentials and no `.env`, reaching no production service (R2, OpenAI,
    Workers AI, Google Input Tools). Only pages that need none are
    measured, and nothing is written to the checkout.
  - **Q6, browser capabilities**: each a platform operation the integrator
    rules when (5) reaches it: speech synthesis, device storage (with the
    Docs app's device-local storage), the clipboard, a canvas (ADR-0075's
    mounted resources) and sharing.
  - **Q7, outside services**: no capability yet for an outbound request
    with a secret; the host has no outbound client (the capability matrix's
    third-party row). Each service and its keys are the owner's to approve,
    asked when (6) reaches them.
  - **Q8, the slice's extras** (stroke count, grade, frequency rank, Korean
    meanings and pronunciation): kiokun.com's page is the reference, so
    parity matches it. Whether to keep the extras is the owner's call,
    asked with the word page's first review; until then they are not shown.
  - **The security findings** in kiokun.com's own source went to the owner
    in the integrator's session (2026-10-08). None is fixed in kiokun-data
    by this project, which writes nothing there.
  - **Corrections taken**: the local-data citation (above), and
    KNOWN_LIMITATIONS' pitch line, which now names kiokun.com's own pitch
    files.
- **2026-10-08, W6's routing question: the host's paths are reserved.**
  kiokun.com's words are at `/<word>`, so its page's route is `/{word}`. The
  development server matches its own endpoints before pages, and a page
  before a file, so that route would answer `/pw-runtime.mjs` and the
  runtime would never load. Ruled (b), as Next.js reserves `/_next/`,
  SvelteKit `/_app/` and Nuxt `/_nuxt/`:
  - **The host's own paths move under `/_pw/`**: the runtime's files and
    its protocol endpoints at the root (`/pw-read`, `/stream`,
    `/pw-handlers`). A page's route may not begin with `/_pw/`, refused at
    compile time. `/command/…` and the store's test controls follow in a
    second step, the rule refusing a page's route under them until then.
  - **Among declared routes, a literal segment beats a parameter at the same
    place** (`/sign-in` over `/{word}`), as routers do, and the routes check
    says so.
  - **Built by the integrator**, being the host's and the runtime's, after
    ADR-0282 lands. W6 builds its program and read-only layer meanwhile;
    its `TRACK SEAM (kiokun)` choosing `KiokunData` in `from_build_with` is
    accepted.
  - **Not (a)**, a file the build put at `dist`'s top winning over pages,
    which ties the URL space to whatever a build emits. **Not (c)**, the
    words at `/w/{word}`, which breaks every link to kiokun.com.
- **2026-10-09, W6's word page (milestone 1a), reviewed.** Track
  `kiokun` at `b59ebe0`: `examples/kiokun-site` on the development server,
  its read-only layer `server/src/kiokun.rs`, the route `/word/{word}`
  until `/_pw/` lands. Its claims were checked against kiokun's source:
  `escaped` is `create_safe_filename` (kiokun-data `src/main.rs:6228`), and
  the stub rule is `+page.ts:438-462`'s.
  - **One fix before the merge**: a word whose file name is longer than a
    file name may be (`<file>.json.deflate` over 255 bytes) failed the read
    with ENAMETOOLONG, which is no `NotFound`, so the query failed where it
    should answer a 404. `place` refuses such a name, tested at the limit.
  - **Q1, a host call through a helper**: the contract was wrong, and the
    build right. ADR-0283, the integrator's: a component imports what the
    code compiled into it calls.
  - **Q2, a fold's empty seed**: ADR-0284, the integrator's. The answer
    first relayed, that the checker's acceptance stood, was wrong: the
    checker passed ill-typed folds, and the backend was the only backstop.
  - **Q3, ports**: W6's Playwright runs use PORT=7341. The plan's 7241
    overlapped W5's range, which reaches PORT+122 (7263): W6's 7241..7243
    were W5's `MESSAGES_PORTS`. **Each worker's range is 200 wide**: W5
    7141, W6 7341, and the next worker 7541, where nothing listens on this
    machine (`lsof -iTCP -sTCP:LISTEN`; Raycast holds 7265).
  - **Q4, kiokun.com's label bug**: not reproduced. Its `getLabel` is
    `flatLabels[tag] || tag` (`sveltekit-app/src/lib/utils/japaneseLabels.ts:24-26`),
    and 53 of the table's 266 flat keys write `_` (`adj_na`, `n_suf`,
    `v5k_s`, ...) where the entries carry JMdict's `-`, so the live site
    shows `adj-na` as it is. The rewrite shows the label the table means:
    the code as written, else with each `-` read as `_`, else the code. It is
    stated in a list of differences from kiokun.com, each with its evidence,
    as Q1's rendering difference is; the comparison counts every other
    difference as a defect. The owner was told; the live site is theirs to
    fix.
  - **Merged 2026-10-09** as ADR-0285 (the inventory) and ADR-0286 (the
    word page), from `d19850f`, after the fix (a 503 before, a 404 now) and
    green runs (verify 37886442522, every engine). Next, 1b: Japanese
    labels through `KIOKUN_APP` with Q4's lookup, and the character header.
- **2026-10-09, W6's 1b and 1c merged; 1e's questions answered.**
  - **Merged** as ADR-0288 (kiokun's labels and the character header) and
    ADR-0289 (the loader's merges and the header's written forms), from
    `de0c4ca`: verify 37896778819, its one recipe shard passed, 45 of 45
    mutants killed; it failed only in WebKit's known flake and in the
    summary, the lone-shard bug fixed on `master` that morning.
  - **1e, the SEO head, waits on the merge** with its timing sample; asked
    three questions, answered:
    - **Q1, an oracle of kiokun.com's own code is the standard** for any of
      its pure functions the rewrite reimplements: its files copied from
      `KIOKUN_APP` into a fresh directory each run, never committed (Q3);
      over the real data on a stated, deterministic sample; kiokun's commit
      and Node's version recorded; every deliberate difference named and
      counted by name, any other failing the run; a committed fixture of
      its answers for the CI sample, which CI holds; hand-made cases for the
      edges the data lacks; an import the harness does not copy refused,
      never stubbed.
    - **Q2, a canonical link is a page address**, written as HTML writes it
      at the top of a page's view (`<link rel="canonical" href={…}/>`),
      checked as `navigate`'s address is (ADR-0280); the host writes it
      absolute with the deployment's configured public origin, never the
      request's `Host`, which a client controls (kiokun.com's uses the
      request's origin: a stated difference); a host whose pages state one
      and that has no origin refuses to start; `og:url` is the host's, from
      it, and a page's own refused (PW5034); no default self-canonical.
      After navigate's merge.
    - **Q3, JSON-LD is typed as JSON**, not as schema.org: a std `Json`
      (null, bool, number, text, list, an object's keys in order), written
      `<script type="application/ld+json">{data}</script>` at the top of a
      page's view, serialized and escaped by the host (ADR-0097); any other
      `<script>` still refused. The integrator's, after the canonical.
- **2026-10-09, W7's plan: TodoMVC, launched when W5's slot frees** (the
  owner's, relayed 2026-10-08: "something people are familiar with"; NEXT,
  "W7, TodoMVC"). Track `todomvc`, branch `track/todomvc`, from `master`.
  Its ADRs are `ADR-XXXX`, numbered at the merge; its recipes live in
  `just/todomvc.just`; its code block is **PW61** (`Owner::Todomvc =>
  "PW61"`); its Playwright runs use PORT=7541 (7541 to 7740, where nothing
  listens here; ranges are 200 wide), its hosts at PORT+140..142
  (`TODOMVC_PORTS`).
  - **The reference is TodoMVC's own**: its spec (`app-spec.md`) and its
    official behavioural tests (`tests/cypress/e2e/spec.cy.js`), run
    unchanged. Cypress, `todomvc-app-css` and `todomvc-common` (npm, MIT)
    are approved downloads (the owner, 2026-10-08), each one's source and
    size stated when fetched; nothing else is.
  - **A fourth program on the development server**, `examples/todomvc/
    app.pw`, beside the store, the feed and kiokun, with a layer of its own
    at a `TRACK SEAM (todomvc)` in `from_build_with`, chosen where the
    program imports `todomvc:`. A session's todos, as the spec allows "the
    framework's own persistence" in place of localStorage; the version on
    a device's own storage waits for the Docs app's device-local storage
    (charter §9.1's kind 4) and says so.
  - **Every behaviour the spec states**, each with its test and its
    mutation control: adding on Enter, trimmed, never empty; the counter's
    "1 item left" and "2 items left"; toggling one and all; editing on a
    double-click, the field focused, Enter and blur saving, Escape
    discarding, an edit trimmed to nothing destroying the item; clear
    completed, shown only where one is; the footer and the toggle-all
    hidden with no items; and the three filters.
  - **The filters' hash routes** (`#/`, `#/active`, `#/completed`) reach no
    server, so a page reads its address's fragment. That is the language's
    and the runtime's, the integrator's to rule when W7 asks, as the plan
    says. The direction: a page binds a signal to its fragment, whose type
    names the forms it takes; the runtime keeps it to `location.hash` at
    start and on each `hashchange`; the server renders its declared
    default. W7 builds the rest meanwhile.
  - **What a gap needs of the language, the runtime or the host goes to
    the integrator as a question**, as W5's and W6's did.
  - **In order**: the app with its tests in three engines and its recipe;
    the official Cypress suite, unchanged, as its acceptance; then the
    showcase, W7's second milestone (NEXT).
- **2026-10-09, W6's 1e merged** as ADR-0293 (kiokun's page head, held to
  kiokun.com's own code), from `4df071b`: verify 37911011082, its one recipe
  shard passed, 53 of 53 mutants killed; it failed only in WebKit's known
  "Load more" flake. W6's 1d (the Japanese examples and pitch accent),
  held until this merge, may go to CI. Its questions were answered before
  the merge (above); the canonical link and std `Json` are the
  integrator's.
- **2026-10-09, W5's store-pg packet, answered: master first.** Its tip
  `1a32fdc` (verify 37894506239: 28 of 29 recipes, `e14-store-postgres`
  among them on PostgreSQL) failed only in WebKit's "Load more" and in
  `e14-instance-changes`, whose survivor `master` fixed at `a609560`
  (move-in-place). Not merged as it is: it is based on `3036497`, before
  ADR-0287 to ADR-0295, and its merge into `master` conflicts in the
  host's choice of layer (`KiokunData::new()` is fallible since W6's 1c).
  W5 merges `master`, keeps its `(store, data)` tuple with master's `?`,
  re-anchors and re-runs the scripts that move, builds in its own target,
  and pushes; the integrator merges on that run, the ADR numbered then.
  W7 (TodoMVC) launches after it.
- **2026-10-09, the DoorDash track (W8's plan), first in the next free
  slot** (the owner, relayed 2026-10-09: DoorDash faster, and a third
  concurrent worker once the disk has room; the coordinator launches it at
  about 45 GiB free). The slots, once W5 merges: W6 (kiokun, continuing),
  **W8** (this track), then **W7** (TodoMVC, as ruled above) in the third.
  W8 launches after W5's merge, on which it builds. Search and filters
  become W9, and ratings and reviews W10.
  - **Track `store-accounts`**, branch `track/store-accounts`, from
    `master` after W5's merge. Its ADRs are `ADR-XXXX`, numbered at the
    merge; its recipes live in `just/store-accounts.just`; its code block
    is **PW62** (`Owner::StoreAccounts => "PW62"`); its Playwright runs use
    PORT=7741 (7741 to 7940, ranges 200 wide), its hosts at PORT+160..162
    (`STORE_ACCOUNTS_PORTS`); its migrations are the store's from
    `0003` (`spikes/own-renderer/server/migrations/store/`), each built and
    tested on both layers, in memory and on PostgreSQL, as W5's store is.
  - **Milestone 1, a cart and an order are a user's.** ADR-0258's identity
    and ADR-0270's principal, wired into the store: a signed-in reader's
    cart and orders are their user's, a guest's its session's guest's
    (ADR-0270's guest model); at sign-in the guest's cart joins the user's,
    by a rule the ADR states (each line added, quantities summed within
    each item's bounds, ADR-0179); an order is read by its user alone, as a
    notification is (ADR-0274); a tab whose reader signed out elsewhere is
    refused at its next press, and told so (the refusal ruling, on
    `track/refusals`, numbered at its merge). Its tests: two users' carts
    never share a line, in memory and on PostgreSQL; a guest's cart follows
    them in; an order is no other user's to read.
  - **Milestone 2, delivery addresses.** A user's saved addresses (add,
    rename, remove, one chosen; a guest's chosen one is its session's);
    each with coordinates from a fixed table in the repository, never a
    geocoding service, which would be a download and an account; **a
    delivery zone**, each store's radius from its location, a pure check;
    **an estimate and availability keyed by the chosen address**: the
    store's estimate gains its travel time, and a store that does not reach
    the address says so where the menu is, its Add refused by a declared
    predicate with its words (`predicate DeliversTo ... says "..."`, the
    refusal ruling's).
  - **Milestone 3, the owner's Next.js bug, as acceptance** (ADR-0280's
    brief): a reader saves an address, whose handler's `Ok` arm goes to a
    store (`navigate StorePage(id)`); the store's page shows the new
    address's estimate as served, with no cache-busting parameter and no
    second load of the document after it arrives, in three engines. "No
    header remount" needs the integrator's soft navigation, not yet built:
    until it lands the test asserts the estimate and one document load, and
    the remount assertion is added with it, by whichever of the two lands
    second.
  - **What a gap needs of the language, the runtime or the host goes to
    the integrator as a question**, as W5's and W6's did. A payment is the
    integrator's (checkout, below), and no crate is added without the
    owner's approval.
  - **The integrator's DoorDash items come first** in its own queue (the
    owner, 2026-10-09): soft navigation, store hours, then checkout, ahead
    of kiokun's canonical link, std `Json`, `/_pw/` and the
    infrastructure follow-ups, after the merges in CI (refusal,
    stream-records, W5).
- **2026-10-09, W5's store-pg merged** as ADR-0298, from `d043fa6`
  (master merged in at `67272f8`): verify 37932273524, 29 recipes green,
  `e14-store-postgres` among them; it failed only in WebKit's "Load more",
  fixed on `master` since (ADR-0296). Its slot frees: **W8, the DoorDash
  track, launches next** (above), the coordinator's to start when the disk
  allows.
- **2026-10-09, W8 launched** (the coordinator, from `28c8781`, in W5's
  slot beside W6), **and its three questions, answered:**
  - **Q1, the cart and the order are the user's in the program (b)**:
    `private query Cart(reader: User<UserId>)` and the order alike, keyed by
    `current_user()` as ADR-0270 rules a user's data, its events naming the
    user. Not (a), the session mapped to its owner inside the store's layer:
    that adds the store to `identified_by`, which ADR-0270 retires (NEXT),
    and leaves the program saying a cart is a session's when it is a
    user's. The layer keys rows by an opaque owner, so the benchmark's copy,
    which keys by session, runs unchanged on it. The compiler tests that pin
    the store's text, its committed handlers, its IR and its contracts move
    with it, each regenerated by its own recipe. Until telling by principal
    lands (NEXT), a cart's change is derived for every live session that
    reads `Cart`: a cost of derivation, not of exposure, bounded by
    ADR-0297.
  - **Q2, the guest's cart joins at sign-in, through one hook**:
    `on_sign_in(guest_session, principal)` in `identity.rs`, generic (the
    identity knows no store), called once the principal is opened and before
    the reply is written. The join is one store transaction: each guest line
    added to the user's cart, one item's quantities summed, past a bigint
    refused rather than wrapped, the guest's cart emptied, the user's
    `CartChanged` committed with it; run twice, the second finds nothing to
    join. A join that fails leaves the sign-in done and the guest's rows
    where they were, logged and stated in the ADR.
  - **Q3, a command is answered for the reader its page was shown to, as a
    rule of the platform, not a predicate a program declares**: every
    program needs it, and a predicate would be each program's to forget.
    The runtime sends its document's id with each command (`pw-document`);
    the host refuses, before the command runs, one whose document it served
    to another session ("You signed in or out in another tab. Reload this
    page to go on.") or no longer holds ("This page is out of date. Reload
    it to go on."), and the runtime tells it where the press was, as
    the refusal ruling's ADR (numbered at its merge) tells a refusal, the
    speculation taken back. A command with no document, from no page, is
    answered as before. W8 builds it in milestone 1 once the refusal ruling
    is merged, a `TRACK SEAM (store-accounts)` at each of the runtime's and
    the host's places; the integrator reviews the seam.
  - **The store's hosts on development accounts** at
    `STORE_ACCOUNTS_PORTS`, as allocated; the guest model stays the store's
    default everywhere else.
- **2026-10-09, W8's finding in `boundary.rs`, answered.** `TypeFacts`'s
  type-level scope ignored `private` (a type only `private` queries produce
  carried none, so a view's parameter of it could be captured into a public
  manifest, R-030's case at the user level), and where scoped producers
  disagreed the last declaration read won, by unit order. Ruled: `private`
  maps to `User`, as `manifest_scope` does; disagreeing producers' scopes
  are joined, never dropped, since the type is all a view's parameter says
  (R-030); a corpus fixture and a unit test for each, and their mutants. The
  store's own precision is the program's to give: one record type made by a
  session query and a user query cannot be proven to hold only the user's,
  so either `lib/Resources.pw`'s unused session `Cart` leaves the store's
  build (its fixtures on a module of their own) or the user's cart is a
  type of its own.
- **2026-10-09, W8's question: `private` is two things, answered (1).**
  `private` is both the user's scope (resume, boundary, the checks) and "not
  importable" (resolve, PW0023), so a user's query could not be imported. A
  `user` visibility is added, importable as `session` is and user-scoped as
  `private` is, a contextual keyword at a declaration's start; the store
  says `user query Cart` and `user page`. `private` keeps both meanings for
  now: whether it goes on implying the user's scope is the integrator's
  ruling (NEXT), a change to every program using it.
- **2026-10-09, W8: the superset telling corrected, telling by principal
  (A).** With `Cart` keyed by the user, a cart's change reached every live
  session reading `Cart`, and five browser tests failed on other sessions'
  empty patch sets. **Corrected:** the integrator's Q1 answer accepted that
  superset as "a cost of derivation, not of exposure"; it is an exposure, a
  frame to user B each time user A changes A's own cart. W8 builds NEXT's
  queued telling by principal: a private entry keyed by a user's handle
  reaches only that user's sessions (the guest model's included), a
  session's entry stays its session's, and a shared one reaches every
  reader with its empty set as before (ADR-0219).
- **2026-10-09, W6's 1d merged** as ADR-0299 (kiokun's examples and pitch
  accent), from `7e3cb51`: verify 37961821792, both recipes green (64 of 64
  mutants), only WebKit's "Load more" failing, fixed on master since. Its
  evidence is taken over master's though master's run is the later commit:
  that run (W5's) tested kiokun's older code, and this one 1d's. **W6's
  finding: `e14-kiokun-word` took 4 h 01 m on CI**, 220 s a mutant, each of
  its tests compiling the program into its own `TempDir`. Ruled: the
  build's files, as bytes, are kept in memory once per test process, keyed
  by a hash of the sources compiled (after a test's `change`), and written
  into each test's own `TempDir` as now. Not the refused static `TempDir`:
  nothing on disk outlives its test (ADR-0158), and a mutant's program is
  compiled once per distinct source instead of once per test, which also
  cuts how many compile at once (the 4 GiB bound). A mutant that still
  takes long is given the tests that can see its rule.
- **2026-10-09, W8's milestone 2 questions (delivery addresses),
  answered.** Q-M2a: whether a store delivers to the reader's chosen address
  is no host predicate (no TRACK SEAM in identity.rs): `requires` answers who
  may act, and this is the store's own rule about its data, a declared error
  the command answers from its transaction (as `ItemUnavailable` is), from the
  same `delivers_to` the estimate's NoCoverage uses, so the two cannot
  disagree; `place_order` refuses an address the store does not reach
  whatever `add_to_cart` does, since a cart outlives an address. Q-M2b: the
  estimator's test controls stay keyed by session, the travel term the
  model's, provided the Estimate entry is kept for no time and never shared
  across a user's sessions; places are a seeded fixture, not a geocoder,
  said in its Not claimed. W8's finding: a template's `<ready as={x}>`
  resolved to an imported module's function of that name (PW0401); the
  integrator's resolver fix, queued (NEXT).
- **2026-10-09, W6's cache accepted** (1f0be09): CPU-s 487 to 261 at one
  thread per core; but the whole run crossed the 4 GiB bound (19 of 69
  mutants' runs stopped by it), so the script runs on four threads again
  (269d9f3), the run's evidence discarded. The Wasmtime compile per server
  load is the integrator's (NEXT, infrastructure). W6 records
  e14-kiokun-word on its branch's verify, not locally: the disk fell to 3.8
  GiB with three sessions' targets (the shared one 35 GB), freed to 31 GiB.
- **2026-10-09, ADR-0300 (the build id) merged** before the refusal ruling,
  whose verify still ran: the numbers follow merge order, so the refusal
  ruling takes the next.
- **2026-10-10, W6's word page moves merged** as ADR-0301, from `4cbe3aa`, before
  the refusal ruling, whose PostgreSQL shard ran again:
  verify 38006256503 and ci 38006256479 green, `e14-kiokun-word` 69 of 69
  and `e14-redirects` 15 of 15 from that run, the moves' oracle (local)
  1,643 words with 3 moves. W6 next, in order: the whole-dictionary sample
  timed again at low load, Contains and Appears-in, then step 2, search.
  Step 4, kiokun's accounts (notes, review cards, custom words), stays W6's:
  kiokun's own data, on the platform's identity (ADR-0258, ADR-0263,
  ADR-0270) and W8's `user` visibility once it is merged; anything it needs
  of the identity layer or the principal is asked of the integrator first.
- **2026-10-10, the refusal ruling merged** as ADR-0302, from `6964ee7`:
  verify 37993980886 green, its PostgreSQL shard run again after a start
  that failed in 43 s; 30 of 30 mutants. Its mutation script took 3.5 h of
  CI (each core mutant ran pw-core's tests whole, each browser mutant built
  `run.sh` and `feed.sh` again): its core mutants are given the tests that
  can see their rule next, the integrator's, as W6's ruling of 2026-10-09
  says.
- **2026-10-10, ports and targets (the coordinator's).** A worker's port
  range avoids what the owner's own apps listen on (Spotify holds 7768 on
  this machine) and what any session left listening: a `python3 -m
  http.server 7801` from the integrator's 2026-10-04 layout-shift probe held
  7801 inside W8's 7741-7940 until the coordinator stopped it. A session
  stops the servers it starts; a probe's server runs in the background only
  with its stop in the same command. Workers build in their own targets
  (`.claude/worktrees/agent-*/target`); only the integrator's worktrees
  share master's `target/`, one build at a time.
- **2026-10-10, W6's questions, ruled.** (1) The Contains/Appears-in
  fixture: a NOTICE.md beside kiokun-oracle's `*-sample.json` (also closing
  the gap for `seo-sample.json` and `examples-sample.json`); the fixture
  trimmed to each list's first 20 and last 5 items with its length, the
  rule recorded in the fixture and the ADR. (2) The guards and oracles read
  kiokun-data's committed HEAD (`git ls-tree`, `git show HEAD:./path`), never
  its working tree: the evidence names a commit, and only a commit is
  reproducible; the owner's uncommitted work is named, not read (ADR-0285
  amended in the Contains ADR). (3) Licences, checked at each primary
  source: KRDICT's text CC BY-SA 2.0 KR (its media licensed per file, never
  committed), EDRDG's files CC BY-SA 4.0 (KANJIDIC2's contributors named
  where their fields are kept), Wiktionary CC BY-SA 4.0 with the GFDL,
  CC-CEDICT CC BY-SA 4.0, Tatoeba CC BY 2.0 FR (each sentence's author, by
  its id). Every committed fixture's NOTICE names each source present, from
  the builder's source, with its licence, version, address and the date
  checked; the fixtures are CC BY-SA 4.0; track `kiokun` merges with it.
  (4) Search: the committed CSV now (no Cantonese romanization, a data
  difference) and a current one asked of the owner; kiokun's two statements
  run in the kiokun layer over FTS5, declared as a source's operations, the
  program doing the rest; a platform full-text source and a route that
  answers JSON are the integrator's, later (NEXT).
- **2026-10-10, W6's finding: `fetched_as` decoded each TCP read alone**:
  a character a read ended inside became U+FFFD ("に��める" under load).
  Fixed on master: the incomplete sequence is carried to the next read,
  and bytes that are no UTF-8 are refused, not replaced.
- **2026-10-10, the layouts merged** as ADR-0303, from `328720e`: verify
  38006542533 green but for Firefox's navigate.spec "its answer first" (the
  command order's finding, named known), 23 of 23 mutants; the store's page
  plan made again over the build id (`just e10-component`). **W8:**
  `StoreLayout` binds `Cart(current_session())`; at `track/store-accounts`'
  next master merge, the layout's cart is its pages' (the user's), and the
  layout is declared `user` (PW5046 holds a page to its layout's audience).
