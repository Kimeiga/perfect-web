# ADR-XXXX: notifications, a row a like, a reply or a follow writes, private to its user

Status: proposed by track `notifications` (W3, `track/notifications`), under
the integrator's rulings of 2026-10-07 (docs/PARALLEL.md, "notifications
(W3)"), on the typed principal of the ADR beside it ("the reader's user is
the host's, and a listener's handle binds a user's id"). Date: 2026-10-08.
Milestone: E14, the owner's Twitter list.

## Context

- **The owner's list asks for notifications**: private, per-user live data
  from others' actions, a like, a reply or a follow involving you, an
  unread count and mark-read.
- **The rulings**:
  - a notification is a row that a like, a reply or a follow writes in its
    own transaction, beside what it writes, as an image is its post's
    (ADR-0260); no materialization;
  - one's own act notifies no one, and a deleted post's notifications go
    with it;
  - private to its user, by cache and by session (charter §15.6): its
    queries keyed by the reader's handle, `cache private`; another user's
    session, signed in and not, sees none of it, by page and by
    `/pw-read`;
  - an unread count on each page that shows its reader, live, and a page
    of them; reading them is a command that invalidates the reader's count
    and list, which reaches each of the reader's sessions.
- **What carries it**: `context.current_user()`, a handle the host alone
  makes, and a listener's handle binding the user's id an event carries
  (the ADR beside this one).

## Decision

### 1. The program (`examples/feed/app.pw`, its additions marked)

```text
type Notification = Notification {
    id: NotificationId, actor: User, act: Act, posts: List<PostId>, unread: Bool,
}
event Notified(user: UserId)

private query Notifications(reader: capability.User<UserId>, limit: Int) -> List<Notification>
    cache          private
    key            reader, limit
    invalidates_on Notified(reader), Followed(_, reader), Deleted(_)
    ..
private query Unread(reader: capability.User<UserId>) -> Int
    ..  (the same listeners)

command mark_read() -> Result<Int, FeedError>
    requires      SignedIn
    idempotent_by InteractionId
    invalidates   Notifications(current_user(), _), Unread(current_user())
    optimistic    Unread(current_user()) as n => none_unread(n)
```

- **`Act`** is `LikedYourPost | RepliedToYourPost | FollowedYou`; a
  notification is about the post liked, or the reply written (`posts`, none
  or one, as `images` is); none for a follow.
- **`like` and `reply` emit `Notified(owner(post))`**, `owner` a data
  operation answering who wrote a post, an id. A follow tells its user by
  `Followed(_, reader)`, its followee. A delete by `Deleted(_)`, every
  user's: a post's notifications are any of several users'.
- **What writes a notification says so**: `add_like`, `add_reply`,
  `add_follow` and `remove` each `database.write<Notification>`, and
  `source FeedData` holds `Notification`. PW5106 then holds each command
  that calls one to dropping the reader's notifications, which it does
  through the events above.
- **The home page shows its reader "Notifications: N unread"**, linked to
  `/notifications`, inside its signed-in block, live. The value is printed
  as the query answers it: a count in words would be a part the host
  computes, which `computed_holes.rs` holds the home page to none of.
- **The notifications page**, `/notifications`: the reader's, newest first,
  twenty at a time ("Load more", a signal the list is keyed by, so the
  browser reads it again through `/pw-read`), each with who, what, a link
  to the post, and "New" where unread; "N unread"; and Mark all read,
  shown at once as none unread.

### 2. The data, in its own transaction (`notifications.rs`, `feed.rs`, `feed_pg.rs`)

- **In memory**: a like, a reply and a new follow stage a notification
  beside what they stage, for the post's author, the replied-to post's
  author, and the one followed; committed with it or not at all. One's own
  act stages none (`noted`). Following again stages none. A delete takes
  the notifications about every post it removes. `notifications#read`
  marks the reader's read.
- **On PostgreSQL**, migration `0006_notifications`: a row per
  notification, written by the statement after the act's own in the act's
  transaction (`INSERT .. SELECT .. WHERE author <> $actor`); its
  recipient never its actor and a follow's about no post (`CHECK`s); its
  post a foreign key `ON DELETE CASCADE`, so a deleted post takes its rows
  in the delete's own statement; indexes by recipient, by unread
  recipient, and by post.
- **Read by the reader's handle**, which is their id on the wire: a layer
  answers a user's notifications for the handle the host made, and never
  maps a session.

### 3. Private, by cache and by session

- **The key is the reader's handle**, which only `current_user()` makes
  from the session. A page's `current_user()` is the host's, and
  `/pw-read` sends only a signal's value, `listed`; a document of another
  session's is no page of this one's, and reads nothing.
- **`cache private`**: every entry is in a session's partition, keyed by
  the session and then its user; nothing is kept for every reader.
- **Each of the reader's sessions**: an event naming them drops the
  entries at them in every partition, and their open pages are read again
  and sent what changed (ADR-0219). Mark-read invalidates the reader's
  count and list by their handle, which reaches each of their sessions
  the same way.

## Alternatives

- **A materialization of each user's notifications**: the ruling's "no
  materialization"; materializations made real stay queued, the
  integrator's.
- **A notification decorated onto the act by a wrapping layer**: the
  integrator refused it for images (ADR-0260): a row kept apart from the
  act it tells of is lost or kept when the act is not.
- **Notifications keyed by the session**: one user's two sessions would be
  two readers, and mark-read in one would not reach the other.
- **Marking read on visiting the page**: a query must not write, and a
  command is a press. A button.
- **A follow's notification taken back by an unfollow**: Mastodon keeps
  it; so does this.

## Consequences

- **Every like, reply and new follow writes one more row**, in its own
  transaction; a delete removes the rows about what it removes.
- **A like reaches every open page that reads the count** (the superset
  of the ADR beside this one); another user's derives from its own kept
  entry and is sent nothing.
- **`add_like`'s callers drop notifications**: `every_entry.rs`'s two test
  commands, which call it, now invalidate `Notifications(_, _)` and
  `Unread(_)`, as PW5106 asks.

## Acceptance

Recorded by `just e14-notifications` in
`docs/evidence/E14/notifications.txt`, PostgreSQL 18.6 named locally:

- **The compiler**: `tests/principal.rs` (the ADR beside this one).
- **The server**, in memory and on PostgreSQL (`notifications.rs`'s unit
  tests and `tests/notifications.rs`):
  - a like, a reply and a follow each tell the user they involve, a row
    each, shown on their page and counted on their home page; following
    again tells no one; on PostgreSQL each row was committed in its act's
    transaction (`committed_in`);
  - one's own like and reply notify no one; on PostgreSQL no row names its
    actor;
  - a deleted post's notifications go with it, and its replies'; a
    follow's stays; on PostgreSQL no row names a post that is gone;
  - an event naming a user drops that user's entries in each of their
    sessions, and another user's are kept;
  - a like reaches each of its user's open pages, and another user's page
    is sent nothing of it;
  - reading them reaches each of the reader's sessions, and another user's
    stay unread;
  - another user's session, signed in and not, sees none of it: by page;
    by `/pw-read` of its own list; by `/pw-read` naming her open page,
    which reads nothing; by the cache every reader shares, which holds
    nothing of either query; and by session, every entry read being in a
    session's partition and keyed by that session's own user.
- **The browser, in Chromium, Firefox and WebKit**
  (`e2e/notifications.spec.mjs`, 3 tests, on NOTIFICATIONS_PORTS with the
  development identity provider): see its header.
- **`scripts/notifications_mutations.py`**: see the report.
- **The whole workspace's server and compiler tests**, PostgreSQL's among
  them, after the rebase onto de500be.

## Not claimed

- **Notifications past the newest twenty on the home page's count**: the
  count is every unread one; the page lists twenty at a time.
- **A notification's text in other languages**, grouping ("Ben and 3
  others liked your post"), push to a device, or email.
- **A like's notification taken back by unliking**: the feed has no unlike.
- **Mentions**: a post naming `@someone` notifies no one.
- **Telling by principal** and **`identified_by` gone**: the ADR beside
  this one.
- **`e14-notifications` on CI's database job**: `NEEDS_DATABASE` in
  `scripts/ci_plan.py` is the integrator's to name it in (Questions).

## Questions for the integrator

1. **`e14-notifications` and the database job**: add it to
   `NEEDS_DATABASE`, as `e14-identity` and `e14-uploads` are, so its
   PostgreSQL tests and mutants run on CI.
2. **The root justfile's import** of `just/notifications.just`: one line,
   beside identity's and uploads', which the justfile says the integrator
   alone edits. Kept here so the recipe runs on this branch's CI; replace
   it with your own at the merge if you prefer.
3. **`identity.rs` gains a test-only `signed_in_for_test`**: one user in two
   sessions without the sign-in flow. A shared file; the flow itself stays
   `tests/sign_in.rs`'s to hold.

## Report

**Built.**
- **The typed principal's host half** (the ADR beside this one): the
  server answers `pw:host/principal#read` and a page's `current_user()`
  from the session's principal. The feed's user id is `capability.UserId`.
  ADR-0091 is amended for a listener's handle.
- **Notifications**: the row and its reads in `notifications.rs`, written
  with each act in `feed.rs` and `feed_pg.rs` (migration
  `0006_notifications`); the program's `Notification`, `Notifications`,
  `Unread`, `mark_read`, the home page's count and `/notifications`; the
  browser suite; the mutation controls; `just e14-notifications`.

**Tests and mutants.** `docs/evidence/E14/notifications.txt`, recorded by
`just e14-notifications` at 83992dc (on 328c306), PostgreSQL 18.6 named:
- the compiler's 3 tests;
- the server's 18: 4 unit tests, 7 in memory and 7 on PostgreSQL;
- the browser's 9 of 9 (3 tests in three engines);
- **23 of 23 mutants killed**: 4 of the listener rule, 14 of the server,
  3 of PostgreSQL and 2 in the browser, among them each the ruling named:
  - one's own act notifying;
  - another user's notifications visible;
  - mark-read not reaching a second session;
  - a notification surviving its post's deletion;
  - the count cached shared;
  - and the integrator's: the event's user dropping the entries of a user
    it does not name.

The first recorded run killed "following again notifies again" by its not
building. Its mutant was rewritten to build, and the second run kills it
by a test.

`just ci` and `just audit` are green at 83992dc. The server's whole
suite, 270 tests with PostgreSQL's, passed after the rebase onto de500be.

**Not claimed.** See Not claimed here and beside: telling by principal,
`identified_by` gone, a stream's `current_user()`, and
`e14-notifications` on the database job.

**Merge notes.**
- Commits, in order:
  - the host answers the reader's user;
  - the feed's user id, the platform's;
  - the ADR-0091 amendment;
  - the rows in both layers;
  - the program and the server's tests;
  - the browser suite, the mutants and the recipe;
  - one mutant made to build;
  - these ADRs and the evidence.
- **Rebased onto ADR-0263** (de500be): context.pw is master's, and the
  platform hash is master's, 0x37769cace0f350fa. This track's first commit
  had made the same declaration, and gave way. Then rebased onto 328c306.
- **Shared files**:
  - `compiler/pw-core/src/codes.rs`: `Owner::Notifications`, PW57, with no
    codes;
  - `compiler/pw-core/src/values.rs`: `handle_over`, and one match arm;
  - `examples/feed/app.pw`, the additions marked;
  - `feed.rs`, `feed_pg.rs`, `identity.rs` (test-only);
  - `main.rs`, at lines marked `TRACK SEAM (notifications)`: the module,
    the two host operations, a plan's `current_user()`, the tests' module;
  - `tests/every_entry.rs`'s `forgetting`;
  - `playwright.config.mjs` (`NOTIFICATIONS_PORTS`, PORT+90..92);
  - the root justfile's one import line.
- **Ports**: the recipe's browser suite, run here, was given `PORT=6100`,
  away from the integrator's suites on the default; the mutation script's
  browser mutants take `PW_NOTIFICATIONS_PORT`, by default 6100.
