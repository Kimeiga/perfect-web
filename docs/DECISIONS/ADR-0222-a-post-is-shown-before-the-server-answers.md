# ADR-0222: a post is shown before the server answers

Status: accepted under the owner's delegation of 2026-10-02. It is the feed
reference app's optimistic posting, which the owner's scope names (NEXT 24),
and builds ruling 0105-a's unnamed key. Date: 2026-10-05. Milestone: E14.

## Context

- **Speculation was the store's cart's** (ADR-0122, ADR-0172, ADR-0191).
  - The compiler builds a module for any page whose presses call a command
    with an `optimistic` clause. It matches the clause's target to the page's
    binding by resource, and by keys that are invocation-context calls.
  - The browser's runtime holds any binding's value, by its entry. It shows
    pending transitions over it, and drops each by the interactions a value
    names or the version a commit's answer names.
  - The server sent one value, the session's cart, from the materializer's
    entry. ADR-0191 did not claim "a speculation on a value other than the
    session's cart".
- **The feed's timeline has a key the command cannot name.** It is
  `Timeline(current_session(), shown)`, its length the page's, a signal "Load
  more" grows. `post(text)` knows nothing of `shown`.
- **Ruling 0105-a names `_` as a target's key**: a command's invalidation
  key must be its optimistic target's key, "the same parameter, the same
  invocation-context call, or `_`". `_` did not resolve.

## Decision

1. **A target may leave a key unnamed.**
   - `optimistic Timeline(current_session(), _) as feed => ..`: each argument
     of the target's call written `_`.
   - It matches whatever the page's binding has there: the timeline the page
     shows, at the length it shows it.
   - Anywhere else, in a transition or inside a key's own call, `_` is a name
     and does not resolve.
2. **The server speculates on any binding a page's manifest names.**
   - The store's cart is kept as it was: the materializer's entry and
     version.
   - **Any other binding's value is read with what its document shows**
     (`Shown`).
     - It is carried into the served page.
     - It is sent as an `entry_value` frame when a commit's change, or a
       longer read, makes it differ from what the page holds, and only then.
     - It goes in the same hold of the subscriber table as the patches it
       was read with, at a version read from the clock in that hold.
     - So a page holds the value its patches were derived from, in the
       order its versions say.
     - Each frame names the presses the value includes (ADR-0172).
   - **A commit's answer names each such value at the version it was last
     sent.** A commit sends its value before it is answered, so the page
     keeps its speculation until it holds that value. It does not drop the
     post and show it again.
   - Each value is an entry of its own, by session, page and binding.
3. **The feed posts optimistically.**
   - `Timeline` answers `Item`s: a post as a timeline shows it, with who
     wrote it, what it says, and its likes. Its replies are its thread's,
     and the data layer always sent an empty list.
   - `post` declares `optimistic Timeline(current_session(), _) as feed =>
     pending(feed, text)`.
   - `pending` puts the post first, by "You", keyed `pending-N`, N the
     timeline's length, so two posts pressed together are two rows.
   - The browser renders the row from the speculated value (ADR-0172). The
     server's row takes its place when the value that includes the post
     arrives.

## Found

- **A value of a type that contains itself cannot be speculated on**
  (ADR-0205 §5): the server would send it knowing no type. The feed's first
  `Timeline` answered `Post`, which holds its replies, so it was refused at
  build. A timeline's item holds none, and is its own type now. The refusal
  stands, by name.
- **A longer read can be applied after a commit it did not see.** A keyed
  read (ADR-0152) fetches outside the session's hold. Its value, read before
  a commit, can be applied after the commit's change was sent. The page then
  shows the list without the commit until the next change. It predates this
  ADR, and is ADR-0224's to fix.

## Acceptance

- **`compiler/pw-core/tests/optimistic_keys.rs`:**
  - the feed's target checks, builds, and names its binding by the key the
    page writes, `["current_session()", "shown"]`;
  - `_` in a transition and inside a key's call does not resolve;
  - a key the page does not show the entry by is refused at build.
- **The development server's tests:**
  - `a_post_is_shown_before_the_server_answers_and_the_page_told_when_it_holds_it`:
    the page holds the timeline. A post sends the new value with its press,
    at a newer version, and the answer names that version.
  - `a_speculated_value_that_did_not_change_is_not_sent_again`: a like of a
    reply sends nothing, and a like of a shown post sends the timeline.
  - `a_page_that_reads_more_holds_what_it_shows`: 22 posts after "Load
    more".
  - The store's `a_speculating_page_is_sent_its_carts_value`, unchanged, one
    value.
- **`e2e/feed.spec.mjs`, two more tests in three engines:**
  - a post held at the network shows first, by "You". It is the server's
    after, by its guest's name, in one row, with the draft cleared.
  - A post whose request fails is taken back, and its draft kept.
  - Added with ADR-0225: 25 posts by another session, then "Load more"
    shows more than twenty. A post held after that shows over the longer
    page, one row more, its post first. The page holds the longer value, as
    `a_page_that_reads_more_holds_what_it_shows` says of the server.
- **`scripts/optimistic_posts_mutations.py`: 11 mutants**, recorded by `just
  e14-optimistic-posts`. `patch_set_mutations.py`'s "what was sent is not
  remembered" is re-anchored, its patches derived with the speculated values
  now.
- **The workspace, 2,055 tests; the browser suite, 743 in three engines.**
  - One run's `slots.spec.mjs` failed: the recommendations slot was not
    filled within five seconds, under another tree's load.
  - It failed the same way once before (ADR-0220's runs), and is looked into
    on its own.

## Not claimed

- **The author's own name on a pending post.** A transition sees the
  binding's value and its command's arguments. The page's own user is
  neither, so a pending post is by "You" until the server's arrives.
- **Likes before the server answers.** `like` is not optimistic yet.
- **A speculation on a value of a type that contains itself** (above).
