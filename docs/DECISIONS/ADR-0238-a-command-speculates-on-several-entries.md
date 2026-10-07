# ADR-0238: a command speculates on several entries, a page on those it shows

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. The rest of the owner's first Twitter gap: a like on the
thread page, which ADR-0236 left refused.

## Context

- **A command's `optimistic` clause named one entry** (ADR-0122), and a
  command has one such clause (PW0028).
- **The feed's `like` speculates on the timeline**: `optimistic
  Timeline(current_session(), _) as feed => liked(feed, post)`. A like shown
  before the server answers on the home page (ADR-0228).
- **The thread page could not have a Like button.** A page whose handler
  calls a command whose clause names an entry the page does not show was
  refused at build: "a speculation on an entry the page does not show by the
  same key".
- **A post is shown in two places**: in the timeline, as an `Item`, and on
  its own page, as the thread's `Post`. A like changes both.

## Research

- **Apollo Client updates every query that shows the object.** An optimistic
  response is written to the normalized cache, and "Apollo Client notifies all
  active queries that include the modified comment"
  ([optimistic UI](https://www.apollographql.com/docs/react/performance/optimistic-ui)).
  A list and a detail view both change, by the object's identity.
- **TanStack Query updates an entry by its key**, a detail entry by its id:
  `setQueryData(['todos', newTodo.id], newTodo)`
  ([optimistic updates](https://tanstack.com/query/latest/docs/framework/react/guides/optimistic-updates)).
  Each entry a mutation changes is the program's to name, in code.
- **Here each entry is named, and checked at build.** Pleris has no
  normalized cache: a timeline's `Item` and a thread's `Post` are two types.
  So the clause names each entry it changes, an arm each. Each arm's
  transition is typed against its own entry's value, and a page speculates
  on the arms whose entries it shows.

## Decision

1. **An `optimistic` clause has an arm for each entry it changes**, separated
   by commas:

   ```text
   optimistic    Timeline(current_session(), _) as feed => liked(feed, post),
                 Thread(post) as thread => liked_thread(thread, post)
   ```

   - each arm's name is its own target's value, and its transition is typed
     against it;
   - an arm with no comma before it is PW0016, reported and read (ADR-0237);
   - a command has one `optimistic` clause still (PW0028).
2. **A page speculates on the arms whose targets it shows.** Each is matched
   to a binding as one clause was (ADR-0122, ADR-0222, ADR-0236).
3. **An arm whose target's resource the page does not show is not the
   page's.** The page waits for the server on that entry, as a page calling a
   command with no clause does.
   - **An arm whose resource the page shows under another key is refused**,
     as before: the speculation would change an entry the page does not
     show.
4. **A page that shows none of a command's targets has no speculation**, and
   no module.
5. **The feed: a like on the thread page.** `like`'s second arm,
   `Thread(post) as thread => liked_thread(thread, post)`, counts the like on
   the thread before the server answers. The thread page has a Like button,
   which passes the page's `id` (ruling 0122-d, ADR-0236).

## Acceptance

- **`compiler/pw-core/tests/speculated_arms.rs`, 5 tests:**
  - the home page speculates `like` on its timeline's arm alone, the thread
    page on its thread's;
  - an arm whose target the thread page shows under another post's id is
    refused;
  - a page that likes a post and shows neither entry has no speculation;
  - each arm's name is its own target's value: the thread arm given the
    timeline's transition is refused;
  - without its comma, the thread's arm is PW0016, the one error, and read.
- **`pw-syntax`'s unit test**: a clause's arms, each a node of its own; a
  missing comma between them the one error, the arm read; and the comma
  after an arrow with no transition kept for the next arm.
- **`speculated_routes.rs`**: the thread page speculates `like` beside
  `reply` (ADR-0236's test, changed with the feed).
- **`e2e/feed.spec.mjs`, in three engines**: "a like on the thread page
  shows before the server answers", with the request held, and the server's
  after.
- **`scripts/speculated_arms_mutations.py`: 8 mutants**, recorded by `just
  e14-speculated-arms`:
  - 7 against the compiler's and the parser's tests;
  - 1 against the feed in three engines.
- **Three older mutants re-anchored**, the code they undo having moved:
  `read_whole_mutations.py`'s two on an arm's arrow, and
  `computed_rows_mutations.py`'s "a like is not shown before the server
  answers".
- **Run whole, re-anchored or near what this changed**:
  `read_whole_mutations.py` 18 of 18 killed, `computed_rows_mutations.py`
  12 of 12, `speculated_routes_mutations.py` 10 of 10 (its test changed),
  `clause_key_mutations.py` 10 of 10.
- **One server test of host behavior changed with the feed**: its variant
  drops `like`'s thread arm beside `reply`'s clause, since an attribute
  computed from a speculated value is refused (ADR-0235).
- **The workspace, 2,132 tests; the browser suite, 782 in three engines.**

## Not claimed

- **Several clauses on one command.** One clause with several arms says it.
- **An entry a page shows under a key no arm names**, such as a thread the
  like's `post` does not key. It is refused, as one clause's target was.
