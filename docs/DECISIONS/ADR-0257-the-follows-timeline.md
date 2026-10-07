# ADR-0257: the follows timeline

Status: accepted under the owner's delegation of 2026-10-02; the feature
of ADR-0195's ruling 10 ("A feed needs this"), the integrator's track
(ADR-0253). Date: 2026-10-07. Milestone: E14, the owner's Twitter list.

## Context

- **The owner's order**: after map keys and `let _`, "a timeline built from
  who you follow, with follow and unfollow commands and a profile page with
  follower counts" (NEXT). The feed had one timeline, every post, the same
  to every reader, and no user had a page.
- **ADR-0255 made a materialization read another, and ADR-0256 made
  `invalidates Q(a, _)` every entry at the rest**: what the timeline is
  checked by. A materialization still has no body or generator, so the
  timeline is a query, as the home timeline is.
- **What the platforms do** (read 2026-10-07):
  - Mastodon's `POST /api/v1/accounts/:id/follow` and `unfollow` are
    idempotent: "Successfully followed, or account was already followed",
    and "Successfully unfollowed, or account was already not followed".
    Its `FollowService` refuses a self-follow as not found
    (`ActiveRecord::RecordNotFound` where the target is the source).
  - Mastodon's `FanOutOnWriteService` puts a status in its author's own
    home feed (`deliver_to_self!`), and its followers'.
  - An account's `followers_count` and `following_count` are on its page.

## Decision

1. **A follow is a row**, `type Follow`, the follower first, which the
   feed's `source` holds: a follow's write is `database.write<Follow>`, and
   PW5106 holds every command that writes one to every cached reader of
   one.
2. **`follow(user)` and `unfollow(user)`**, each `requires SignedIn`,
   idempotent by interaction, and idempotent in what they write: following
   one already followed writes what is there, and unfollowing one not
   followed is no change. Each commits, so its event refreshes every page
   that shows it. Following oneself, or one the feed does not know, is
   `FeedError.NotFound`, as Mastodon's self-follow is.
   - Each emits `Followed(follower, followee)` or `Unfollowed(..)`, and
     invalidates the follower's `FollowingTimeline(current_session(), _)`
     at every limit and its `Relation` to the user.
   - Shown before the server answers: the button, `Relation`, and the
     user's follower count, `Profile`.
3. **`FollowingTimeline(session, limit)`**: the posts of those the
   session's user follows, and its own, that reply to none, newest first,
   as the home timeline is read; at `/following`, beside the home page,
   which stays everyone's.
4. **`Profile(id)`**: a user's name and handle, how many follow them and
   how many they follow, and their 20 newest posts; every reader's, shared,
   listening for `Followed(_, id)`, `Followed(id, _)`, their unfollows,
   their posts and likes. At `/user/{id}`, not found for one the feed does
   not know. A timeline's handle leads there.
5. **`Relation(session, user)`**: `Yourself`, `Following` or
   `NotFollowing`, the reader's own, which decides the page's button.
6. **A user the feed knows** has a row, or wrote a post, follows or is
   followed: a session's guest is someone once it has done one.
7. **The browser suite's hosts keep the feed in memory**, whatever the
   environment of whoever runs it (`playwright.config.mjs`, `IN_MEMORY`).
   Playwright gives a server the caller's environment with its own over
   it, and a `PW_FEED_DATABASE_URL` left there put every engine's feed host
   on one database, and each run's posts on the last's: three mutation
   runs' browser baselines here failed so. A suite on PostgreSQL is the
   server's tests'.
8. **PostgreSQL**: migration `0004_follows`, the follower and the followee
   the key and not equal, an index by the followee, and one of each
   author's posts that reply to none, newest first. A page is read in one
   statement, so one snapshot.

## Acceptance

- **The server's tests** (`src/tests/follows.rs`), in memory and on
  PostgreSQL: following brings their posts into your timeline and
  unfollowing takes them out, everyone's keeping them; a page counts who
  follows and whom, and a follow reaches another reader's open page;
  following again is no change; the button is the reader's relation; and
  following oneself or no one is not found, and commits nothing.
- **The browser, in three engines** (`e2e/feed.spec.mjs`): following
  shows their posts in Following and unfollowing takes them out; a follow
  shows before the server answers and is the server's after; a follow
  reaches another reader of the page without a reload; your own page says
  it is yours; a page of no one is not found.
- **The feed checks as it did**, and PW5106 holds the new commands to every
  reader of a follow. The tests that edit the feed by its text find what
  they did: a page's counts are `#follow-counts`, the following page's
  rows read `item` and a page's posts `post`, so the home page's row and
  the thread's counts are each written once; `speculated_arms.rs` counts
  `like`'s three arms, and `computed_rows.rs` edits the home page's row
  alone.
- **`scripts/follows_mutations.py`: 12 mutants**, recorded by `just
  e14-follows`, and PostgreSQL's three in `feed_postgres_mutations.py`
  (`just e14-feed-postgres`, the database job's): the timeline's join, a
  follow's write, and its refusal of oneself; 7 of 7 there.
- **The scripts whose anchors a timeline row's or `like`'s change moved
  run whole**: `speculated_arms` and `computed_rows`, their browser mutants
  among them.
- **The whole server's tests, 174, and the chain on the push** (ADR-0245).

## Not claimed

- **A materialization of the timeline**: it is a query, read on each
  render. The ruling's fan-out is "materializations made real".
- **Another session of the follower's user** is told of its follow by the
  session, not by the user: the identity track's principals.
- **Blocks, mutes, follow requests, private accounts, lists of followers**,
  and a page's posts past its newest 20.
