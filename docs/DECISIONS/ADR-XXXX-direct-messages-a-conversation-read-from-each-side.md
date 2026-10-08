# ADR-XXXX: direct messages, a conversation read from each side

Status: proposed by track `messages` (W4, `track/messages`), under the
integrator's rulings of 2026-10-08 (docs/PARALLEL.md, "direct messages
(W4)"). Date: 2026-10-08. Milestone: E14, the owner's Twitter list.

## Context

- **The owner's list asks for direct messages**: a conversation private to
  its two users, a list of them with unread counts, a conversation page that
  sends, live to both, to someone who follows you or has messaged you.
- **Nothing in the language holds a value private to two**, and the
  rulings need none: each participant reads the conversation as their own,
  through a private query keyed by their own handle and the other's id. A
  program makes no handle (ADR-0263), so a session reads only its own side.
- **What carries it**: the typed principal and ADR-0270's listener rule (a
  `User<..>` parameter binds an event's id), notifications' model of a row an
  act writes and a private query keyed by the reader (ADR-0274), and
  ADR-0275's waiting row.
- **X's rule.** X's help center ("About Direct Messages", read by the
  integrator on 2026-10-08) lets one message anyone who follows them, and
  lets someone they do not follow message them where they have messaged
  that someone before. Its opt-ins (messages from anyone, from verified
  users) are not claimed.
- **X's limit, checked**: 10,000 characters. Twitter's v1.1 reference for
  `POST direct_messages/events/new` (developer.twitter.com) gives
  `message_data.text` a "Max length of 10,000 characters", as its search
  index quotes it; the page itself renders by script, and did not render for
  W4's fetch on 2026-10-08. The v2 API's `POST
  /2/dm_conversations/{id}/messages` (docs.x.com, read 2026-10-08) states
  `minLength: 1` for `text` and no maximum. The help center refused the
  fetch (403). So the limit is stated here as Pleris's own, matching the
  v1.1 reference and the 2015 change's reporting (from 140 to 10,000,
  August 2015): 1 to 10,000 code points.

## Decision

### 1. The program (`examples/feed/app.pw`, its additions marked)

```text
opaque type MessageText = String where String.length(value) >= 1 & String.length(value) <= 10_000
type Message = Message { id: MessageId, from: User, text: String, unread: Bool }
type Conversation = Conversation { with: User, me: User, messages: List<Message>, closed: Bool, yourself: Bool }
type ConversationItem = ConversationItem { with: User, last: String, unread: Int }
event Messaged(from: UserId, to: UserId)

private query Conversation(reader: capability.User<UserId>, with: UserId, limit: Int) -> Result<Conversation, FeedError>
    cache          private
    key            reader, with, limit
    invalidates_on Messaged(reader, with), Messaged(with, reader), Followed(with, reader),
        Unfollowed(with, reader)
private query Conversations(reader: capability.User<UserId>) -> List<ConversationItem>
    invalidates_on Messaged(reader, _), Messaged(_, reader)
private query UnreadMessages(reader: capability.User<UserId>) -> Int
    invalidates_on Messaged(reader, _), Messaged(_, reader)

command send(to: UserId, text: MessageText) -> Result<Message, FeedError>
    requires      SignedIn, MayMessage(to)
    emits         Messaged(author(current_session()), to)
    optimistic    Conversation(current_user(), to, _) as talk => sent(talk, text)
command read_conversation(with: UserId) -> Result<Int, FeedError>
    requires      SignedIn
    invalidates   Conversation(current_user(), with, _), Conversations(current_user()),
        UnreadMessages(current_user())
```

- **A conversation is read from one side**: its reader's handle and the
  other's id; the same rows read twice, each read private to its reader. No
  label names two principals.
- **The conversation carries whether its reader may send**: `closed`, true
  where the other neither follows the reader nor has messaged them, or is
  the reader. It is read again on `Followed(with, reader)`,
  `Unfollowed(with, reader)` and `Messaged(with, reader)`.
- **`closed`, not `may_send`**: the composer is `hidden={talk.closed}`. A
  page's attribute from a value it speculates on is a path, never a value
  computed from it (ruling 0073-a), and a signal bound in a block a query
  value decides is not rendered again (ADR-0137): the composer cannot sit in
  an `{#if}` on the conversation, and `!talk.may_send` is a computed value.
  So the form is always in the page, hidden where closed, and the page says
  why in a block of its own (`#may-not-send`).
- **`send` writes as the session, reads as the user**: `add_message`
  takes `current_session()`, as `publish` does, so a signed-in sender's
  handle and name are written with what they write; every read takes
  `current_user()`.
- **`emits Messaged(author(current_session()), to)`**: the sender's id,
  which is `current_user()`'s on the wire. Each listener binds one of its
  `User<..>` parameter to one of the event's ids (ADR-0270), so the sender's
  sessions and the recipient's are told, and no third user's entry is
  dropped.
- **A message is shown before the server answers**, last, keyed
  `pending-{n}` by `sent()`, from `talk.me`: the conversation carries the
  reader as the feed shows them, so the speculated message fabricates no
  user. It waits by ADR-0275's rule: `message_waits(id)` is `waits`, so its
  sender's name links nowhere and it says "Sending" until the server's row
  stands in its place.
- **Reading is a command behind "Mark read"**; sending marks the sender's
  side read.
- **The pages**: the home page's "Messages: N unread", linked to
  `/messages`; `/messages`, the reader's conversations newest first, each
  with the other, the last message and its unread count; `/messages/{with}`,
  the newest 50 messages oldest first ("Load earlier" reads 50 more through
  `/pw-read`), "New" on each unread, Mark read, and the composer; the
  profile page's Message link. A signed-out reader is told to sign in.

### 2. The data, in the command's transaction (`messages.rs`, `feed.rs`, `feed_pg.rs`)

- **In memory**: a message is staged with its place (`m{n}`, never another's)
  and committed with the command; its commit sets the sender's mark at it.
  A reader's mark is the last message of the conversation they have read;
  a message is unread by its recipient past their mark. Reading sets the
  reader's mark at the conversation's last message.
- **On PostgreSQL**, migration `0007_messages`: `messages` (sender,
  recipient, text, `sent_at`, `committed_in`; `CHECK (sender <>
  recipient)`; its text `char_length` 1 to 10,000, code points as
  `String.length` counts them) and `message_reads` (reader, other, seq).
  `send` is one statement: the row, and the sender's mark at it. A
  conversation is read in one statement, so one snapshot: both users, whether
  the feed knows the other, whether the reader may send, and the messages.
- **`MayMessage(to)`** is evaluated by `Identity::requires` at a line marked
  `TRACK SEAM (messages)`, through the command's own operation
  `feed:data/messages#may`, in its transaction, as `OwnsPost(post)` reads
  `posts#thread`. No principal may message no one.
- **A message writes no notification**; its count is its own.

### 3. Private, by cache and by session

As notifications' (ADR-0274, 3): each query keyed by the reader's handle,
which only `current_user()` makes from the session; `cache private`, so
every entry is a session's partition's; an event naming a user drops the
entries at them in each of their sessions and tells their open pages.

## Found

- **The composer cannot sit in a block the conversation decides.** The
  first build of `/messages/{with}` put the form in `{#if talk.may_send}`,
  inside `{#if me.signed_in}`, and `pw build` refused it twice:
  - ADR-0137: a signal bound (`bind:value={message_draft}`) inside a block
    a value other than a signal decides is not rendered again by the
    browser;
  - ruling 0073-a: `hidden={!talk.may_send}`, and a `disabled` reading
    both `talk` and the draft, are attribute values computed from a value
    the page speculates on, which the browser does not compute.
  So the conversation carries `closed: Bool`, a path an attribute may
  read; the form is always in the page, `hidden={talk.closed}`; the reason
  is a block of its own (`{#if talk.closed}`); and `send`'s 403
  `"refused":"MayMessage"` backs it. The integrator accepted it
  (2026-10-08, answer 3).
- **`message_waits` is `waits`.** Written as its own
  `String.starts_with(id.value, "pending-")`, it made
  `waiting_rows_mutations.py`'s three anchors match twice, which `just
  ci`'s mutation-anchors check refuses. Delegating keeps ADR-0275's rule
  single; waiting_rows' mutants of `waits` now reach the conversation's
  pending message, and this track's mutation controls run all three against
  its browser suite (the report).
- **A known user is one who posted, followed or was followed.** A
  signed-in user who has done none is not found by `/messages/{id}` (the
  feed's `known`, ADR-0257), as by `/user/{id}`; a message makes both its
  users known.

- **The database job no longer fits its limit.** With `e14-messages` in
  `NEEDS_DATABASE`, verify run 37831080654 (at c675180) ran its four
  database recipes in series on one runner: `e14-identity` passed in 1958
  s, `e14-messages` in 2695 s (25 of 25 mutants killed, the browser's 9 of
  9, CI's evidence artifact), and the job's `timeout-minutes: 120`
  cancelled it in `e14-notifications`, `e14-uploads` unrun; the summary
  failed with it. Not re-run. verify.yml is the integrator's; reported by
  message on 2026-10-08.
- **An intermittent, not this track's**: the first push's verification
  (run 37826467131, at b393cee) failed its `browser webkit` job (job
  113480498704) in `e2e/feed.spec.mjs:419`, "Load more shows the next
  page, and a post after it is shown over it": its rows stayed at 20 after
  5 s (1 failed, 274 passed). The same flake ADR-0274 reported, the
  integrator's to chase. Not re-run. `messages.spec.mjs` passed in that job.

## Alternatives

- **A label naming both principals**: refused by ruling. Every label would
  need a reader set and every flow a check, and no read here is on two
  users' behalf.
- **A conversation keyed by the pair, sorted**: one entry both read. It is a
  value two may read, the same thing by another name.
- **The composer in `{#if talk.may_send}`**: refused by the compiler, as
  above. `hidden` is the HTML Living Standard's attribute for content not yet
  or no longer relevant; a hidden form is not rendered and not in the
  accessibility tree, and the server refuses a send anyway.
- **`may_send` and `closed` both carried**: two fields that must agree.
- **Marking read on visiting**: a query must not write; a button, as
  notifications'.
- **The read mark as a flag per message**: a mark per reader and other user
  is one row a conversation, written once a read.

## Consequences

- **Every send writes a message row and the sender's mark**, in one
  transaction; a read writes the reader's mark.
- **A message reaches every open page that reads either user's list or
  count** (ADR-0270's superset); a third user's derives from its own kept
  entry and is sent nothing.
- **`waits` is the one rule for a row the page made**, the timeline's and
  the conversation's: waiting_rows_mutations' mutants of it reach both.

## Acceptance

Recorded by `just e14-messages` in `docs/evidence/E14/messages.txt`,
PostgreSQL 18.6 named locally:

- **The server**, in memory and on PostgreSQL (`messages.rs`'s unit tests
  and `tests/messages.rs`):
  - X's rule: the recipient follows the sender, or has messaged them; not
    the other way about; never oneself;
  - a message shown on both sides, counted unread for its recipient alone,
    oldest first; sending marks the sender's side read; a message writes no
    notification (on PostgreSQL, the follow's is the one row); each read mark
    written in the transaction of the message that set it;
  - a stranger refused 403 `"refused":"MayMessage"`, and the page hiding
    its composer and saying why; following someone is no leave, being
    followed is; one messaged may answer after the follow is gone; oneself
    refused, "You cannot message yourself."; a guest refused `SignedIn` and
    told to sign in; nothing refused written;
  - its text held where it arrives: 0 and 10,001 code points refused by
    `feed.app.MessageText`, 10,000 emoji taken;
  - reading reaches each of the reader's sessions, and another user's stay
    unread;
  - a message reaches both of the sender's sessions and the recipient's
    list and conversation, live, and neither a third user's open
    conversation and list nor a guest's is sent a frame holding its text;
  - a follow opens the composer of the open conversation, live, and an
    unfollow closes it;
  - a message drops the conversation from each side, and each user's list
    and count, in each of their sessions, and keeps a third user's and
    another conversation of the sender's;
  - a third user, signed in and not, sees none of it: by page (list and
    each conversation), by `/pw-read` of their own conversation, by
    `/pw-read` naming another session's page by its number (superseded or
    refused), by the cache every reader shares (none of the three queries
    in it), and by session (every entry keyed by its session's own user).
- **The browser, in Chromium, Firefox and WebKit** (`e2e/messages.spec.mjs`,
  3 tests, on MESSAGES_PORTS with the development identity provider): see
  its header.
- **`scripts/messages_mutations.py`**: see the report.

## Not claimed

- By ruling: blocking; group conversations; message requests and messages
  from anyone; deleting a message; editing; read receipts; images; search.
- **A message's time on the page**: kept in the row (`sent_at`, `at`), shown
  nowhere yet.
- **Telling only the two users' sessions**: ADR-0270's superset; telling by
  principal waits (NEXT).
- **A command run when a page is shown**: reading is a button, as ruled.
- **More than the newest 50 a page, then 50 more a press**; the list is
  every conversation.
- **`e14-messages` on CI's database job**: `NEEDS_DATABASE` in
  `scripts/ci_plan.py` is the integrator's (Questions).

## Questions for the integrator

Asked by message on 2026-10-08, and answered the same day:

1. **`e14-messages` on CI's database job?** Yes: W4 adds it to
   `NEEDS_DATABASE` in `scripts/ci_plan.py`, in a commit of its own, and
   the integrator keeps it at the merge. Done.
2. **The root justfile's import line?** Kept, as the other tracks' are.
3. **`closed`, the form hidden, the reason a block of its own?** Accepted;
   recorded in Found, with both refusals named.
4. **The shared files touched?** Fine as listed (Report, merge notes);
   `pw fmt` on the feed before each push.
5. **`message_waits` delegating to `waits`?** Good; recorded in Found, and
   the suite kills waiting_rows' mutants on the conversation (Report).
6. **10,000 as Pleris's own?** Right, citing v1.1 as indexed and v2 as
   giving no maximum.

And the integrator's port rule (2026-10-08): this track's Playwright runs,
`e14-messages` and its mutation script take a base of their own, now
`PORT=7141` (the script's default, `PW_MESSAGES_PORT`). The recorded runs
before the rule took `PORT=6300`, hosts 6300 to 6402: none of the
integrator's (3141 to 3243) nor notifications' recipe's (6100 to 6202), so
no kill of theirs was a port collision.

## Report

**Built.**
- **Direct messages in both of the feed's layers**: the row and its read
  mark in `messages.rs`, the operations in `feed.rs` and `feed_pg.rs`
  (migration `0007_messages`), and `MayMessage(to)` at the identity's seam.
- **The program**: `Message`, `Conversation`, `ConversationItem`,
  `Messaged`, the three private queries, `send` and `read_conversation`,
  the home page's count, `/messages`, `/messages/{with}` and the profile
  page's Message link.
- **The browser suite, the mutation controls and `just e14-messages`.**

**Tests and mutants.** `docs/evidence/E14/messages.txt`, recorded by
`just e14-messages` at d812952, PostgreSQL 18.6 named, `PORT=6300`:
- the server's 19: 4 unit tests, 8 in memory and 7 on PostgreSQL;
- the browser's 9 of 9 (3 tests in three engines);
- **25 of 25 mutants killed**, each by a test, none by its not building:
  15 of the server, 3 on PostgreSQL, 3 of the server in the browser and 4
  of the program in the browser, among them each the ruling named:
  - a third user seeing a conversation (in memory, on PostgreSQL, in the
    browser);
  - the conversation cached shared;
  - `MayMessage` inverted, and skipped (in memory and in the browser);
  - messaging yourself allowed;
  - the recipient's sessions not told (the count; the open conversation);
  - sending not marking the sender's side read (in memory, on PostgreSQL);
  - a message writing a notification;
  - and waiting_rows' three mutants of `waits`, on the conversation's
    pending message.

The workspace's compiler tests (202 binaries) and the server's 294, with
PostgreSQL's, passed after the rebase onto bba4ba4. `just ci`'s gates
beyond the workspace tests passed locally: the evidence gates' 41, the
mutation anchors (2007 mutants, each matching once), `cargo fmt --check`,
clippy on the server, and `pw fmt --check` on the feed. CI's `ci` run
37826466905 passed at b393cee.

**Not claimed.** See Not claimed.

**Merge notes.**
- Commits, in order:
  - the track's block, `Owner::Messages`;
  - the rows in both layers, and `MayMessage`;
  - the program and the server's tests;
  - the feed formatted by `pw fmt` (ADR-0276);
  - the browser suite, the mutants and the recipe;
  - `e14-messages` on the database job;
  - the pending message seen waiting, and the program's mutants;
  - the port base, this ADR and the evidence.
- **Rebased onto bba4ba4** (ADR-0276): the feed formatted, `10_000`.
- **The four status documents are untouched**, as PARALLEL.md asks.
- **Shared files**:
  - `compiler/pw-core/src/codes.rs`: `Owner::Messages`, PW58, no codes;
  - `examples/feed/app.pw`, the additions marked: the track's block, the
    source's `Message`, the home page's count and the profile page's link;
  - `feed.rs`, `feed_pg.rs`, each addition marked;
  - `identity.rs`, at lines marked `TRACK SEAM (messages)`;
  - `main.rs`, the two `mod` lines marked `TRACK SEAM (messages)`;
  - `playwright.config.mjs` (`MESSAGES_PORTS`, PORT+100..102);
  - `scripts/ci_plan.py` (`NEEDS_DATABASE`);
  - the root justfile's one import line.
