# ADR-0236: a speculation on the entry a page's parameter keys

Status: accepted under the owner's delegation of 2026-10-02. It is ruling
0122-d (ADR-0210), the first of the Twitter gaps the owner ordered on
2026-10-07. Date: 2026-10-07. Milestone: E14.

## Context

- **A speculation's target names a resource and its key** (ADR-0122): `optimistic
  Timeline(current_session(), _) as feed => ..`. A page shows the target when one
  of its bindings reads the same entry, and only such a page speculates.
- **A key was matched two ways**: an invocation-context call, matched to the
  page's own (`current_session()`), and `_`, which matches any (ADR-0222).
- **A thread is keyed by the page's parameter.** The feed's thread page binds
  `Thread(id)`, and its form passes `id` to `reply(to, text)` (ADR-0231). A clause
  `optimistic Thread(to)` matched no binding: the page was refused with "speculates
  on an entry `feed.app.PostPage` reads by no binding whose key resolves to the same
  invocation context".
- **Ruling 0122-d**: "A speculation target's key also matches when the handler
  passes, unchanged, the page parameter (or invocation-context call) that the
  binding's key reads to the command parameter the target names. Any other flow
  stays refused by name." Its reason: "`/post/:id` speculation is the common case
  for the app layer."
- **The server named a speculated entry by the session, the page and the
  binding.** Two threads one session has open would have been one entry.

## Research

- **TanStack Query keys an optimistic update by the mutation's variable**, matched
  exactly against the cached entry's key: `setQueryData(['todos', newTodo.id], ..)`
  ([optimistic updates](https://tanstack.com/query/latest/docs/framework/react/guides/optimistic-updates)).
  Here the same match is proven at build: the variable is the page's parameter, by
  its binding.

## Decision

1. **A target's key may name one of the command's parameters.** It matches a
   page's binding whose key reads, at the same place, one of the page's parameters,
   when every handler on the page passes that parameter to that command parameter
   unchanged:
   - in each call the page's body makes, at the parameter's position or by its
     name;
   - the page's parameter by its binding: a name the handler binds to it, or any
     other value, is not it;
   - in at least one call.
2. **A call in a view the page composes is not followed.** Its use may give it
   anything, so the key is not matched.
3. **Any other flow is refused by name**, as before: "or to the page's parameter
   each of its handlers passes the command unchanged (ruling 0122-d)".
4. **A region a speculation renders reads the page's parameters.** The
   document carries them, as its address gives them, and the browser renders
   such a region with them. ADR-0231 refused one read there, since the browser
   held none. Once the thread was speculated on, every block the thread decides
   was such a region, and reading `id` in one refused the page.
5. **The server names a speculated entry by the page's parameters its key reads**
   too: the session, the page, the binding and the thread's `id`. A key of
   invocation-context calls and signals reads none, and its entry is named as
   before. Each frame, each document's entries and a commit's basis name it so.
6. **The feed: an optimistic reply.** `reply` speculates on `Thread(to)`, and the
   thread page shows the reply at once:
   - last among the thread's replies, by "You";
   - counted in `#counts`, and "No replies yet." gone;
   - the server's when it answers;
   - taken back, the draft kept, when its request fails.

## Acceptance

- **`compiler/pw-core/tests/speculated_routes.rs`, 2 tests:**
  - the feed's thread page speculates `reply` on `thread`, keyed by `id`;
  - refused by name: the parameter passed through a name the handler binds,
    another post's id, and a view's call.
- **`speculated_routes.rs`'s third test**: a block the thread decides reads
  the page's `id`, and is a region.
- **The development server's tests:**
  - `a_speculated_thread_is_named_by_its_page_parameter`: two threads one
    session has open are two entries, each named by its `id`. A reply sends its
    thread's new value, as its nodes, to that entry alone, and the commit's
    basis names that entry at that version.
  - `a_speculating_document_carries_its_parameters`: the thread page's carries
    `{ "id": "p1" }`, the home page's none.
- **Six earlier tests changed with the feed**, its thread page speculating now:
  - ADR-0231's four read `id` in a block or a row the thread decides, which
    builds again by decision 4. One reads the page's visible text, since the
    block's template is in the document's manifest;
  - two server tests of host behavior drop `reply`'s clause in their variants.
    One stops `Thread` listening for posts, which PW5107 refuses of a query a
    command speculates on. The other computes an attribute from the thread,
    which is refused (ADR-0235).
- **`e2e/feed.spec.mjs`, in three engines**:
  - "a reply shows before the server answers, and is the server's after";
  - "a reply whose request fails is taken back".

  They are the browser tests ADR-0233 to ADR-0235 waited for. The thread is a
  value of a type that contains itself, sent as its nodes. It is shown through a
  view that contains itself, and "No replies yet." is decided by a subject the
  module computes.
- **`scripts/speculated_routes_mutations.py`: 10 mutants**, recorded by `just
  e14-speculated-routes`:
  - 9 against the compiler's and the server's tests;
  - 1 against the feed in three engines.
- **`optimistic_posts_mutations.py`'s two key mutants** are re-anchored on the named
  key places. Run whole, as are `page_parameters_mutations.py`, whose tests changed:
  11 of 11 and 8 of 8 killed.
- **The workspace, 2,117 tests; the browser suite, 779 in three engines.**

## Not claimed

- **A like on the thread page.** `like` speculates on the timeline, which the thread
  page does not show, and a clause whose target a page does not show refuses the
  page: next, its own ADR.
- **An invocation-context call passed to a command parameter**, which the ruling also
  names. No program passes one; `current_session()` is the command's own to call.
- **The parameter through a name bound to it**, `let to = id`: refused, conservatively.
- **A region reading the page's parameter, in browsers.** No page of the feed
  reads one there. The browser's renderer is given the document's parameters
  beside the speculated value.
