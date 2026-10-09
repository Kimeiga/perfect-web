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
  - **W6, kiokun.com in Pleris**, after W5 (the owner's production target,
    relayed 2026-10-08; NEXT): parity with the live site, on a preview
    origin, with no deploy, cutover or DNS change without the owner's go.
    Its rulings are written here before it launches.
  - **W7, search and filters**, after W6: run in the database or a
    declared search source; no search engine is built here.
  - **W8, ratings and reviews**, after W7 and the integrator's accounts: a
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
