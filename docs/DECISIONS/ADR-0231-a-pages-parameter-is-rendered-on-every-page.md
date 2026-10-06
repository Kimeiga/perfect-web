# ADR-0231: a page's parameter is rendered on every page

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-05.
Milestone: E14. Found building the feed's replies (NEXT 22). Ruling 0122-d's
route-keyed speculation is to make a reply optimistic, and a reply form
passes the page's parameter.

## Context

- **A page that binds no query is rendered with its parameters** (ADR-0130):
  each one text, as the address gives it. A route's parameter is text
  (ADR-0160, PW0621).
- **A page that binds a query was not.** Its host rendered it with its
  queries' values, what it computes and its signals, and no parameter. On
  the feed's thread page, `PostPage(id)`, which binds `Thread(id)`:
  - `<title>Post {id}</title>` checked and built. Then every request for the
    page was answered 503: "the title: MissingValue { path: \"id\" }";
  - so did `<a href="/post/{id}">`, and a handler that captures `id`;
  - `<p>{id}</p>` was refused at build: "part 1 reads `id`, which no query
    binds".
- **KNOWN_LIMITATIONS said otherwise**: "A title reads a page's parameters
  and its queries' values" (ADR-0183). No test read a parameter on a page
  that binds a query.
- **The feed needs one.** A reply form on a thread page sends the thread's
  `id`: its handler captures the page's parameter.

## Research

- **SvelteKit gives a page its route's parameters** in its `load` function,
  `params`, and in the page itself, `page.params`
  ([loading data](https://svelte.dev/docs/kit/load)). A page reads its
  address's parameters wherever it reads a value. Here the browser holds
  only the signals, so a page reads them where a host renders.

## Decision

1. **A page that binds a query is rendered with its parameters**, as one
   that binds none is (ADR-0130): each one text, as the address gives it.
   - Its document.
   - Each block a host renders again, and each row, when a query's value
     changes (ADR-0145, ADR-0146).
2. **What reads one renders**: a text part, a title, an attribute, a
   handler's captures and an instance's argument. A text part was refused
   at build; the rest built, then failed.
3. **A parameter is the document's for its life.** Another address is
   another document (ADR-0162). No host sets a part again for it, and the
   plan names none.
4. **Still refused, by name:**
   - a parameter read inside a block a signal decides (ADR-0137). The
     browser renders that block again from the page's signals alone.
   - one read in a region the browser renders again with a speculation
     (ADR-0172), which holds the value it speculates on and the signals;
   - a value computed from a parameter: "and a host computes one from a
     query's value alone (ruling 0073-a)". It said "which is no query's
     value".
5. **The feed: replies.**
   - `command reply(to: PostId, text: PostText)`, through
     `feed:data/posts#reply`. A reply is a post: it emits `Posted`, and
     every open thread is read again.
   - The thread page's form passes `id`, the page's parameter, which its
     handler captures. It clears when the server answers, and its button is
     disabled for an empty draft or one too long.
   - A reply is shown in its thread, to every reader, and in no timeline. A
     reply to a post that is not there is not found, and nothing commits.

## Found

- **An opaque value's representation does not render in a template.**
  `{p.id.value}` in the home page's row, and `{id.value}` on the thread
  page, check and build. Then the page is answered 503, `MissingValue {
  path: "p.id.value" }`: the renderer reads `.value` as a field of the text.
  Not this decision's. ADR-0232 is to fix it.
- **A like on the thread page is refused at build.** `like` speculates on
  `Timeline`, which the thread page does not show: "speculates on an entry
  `feed.app.PostPage` reads by no binding whose key resolves to the same
  invocation context". Ruling 0122-d's.

## Acceptance

- **`compiler/pw-core/tests/page_parameters.rs`, 2 tests:**
  - a text part, an attribute, a link and a block a host renders read the
    thread's `id`, and so does the title. Nothing a host sets again names it.
  - inside a block a signal decides, and a value computed from it, each
    refused by name; the control, in a block a host renders, builds.
- **`computed_holes.rs`**: the refusal of a value computed from `id` names
  the page's parameter and the ruling.
- **ADR-0227's tests**, `computed_signals.rs` and the server's: the thread
  page computes its reply button's `disabled` from the reply's draft now.
  Their control, a page that computes nothing from a signal, has no module.
- **The development server's tests:**
  - `a_page_that_binds_a_query_is_rendered_with_its_parameters`: the thread
    page's title "Post p1", an attribute, a link, a text part and the form's
    captures. A like's block rendered again reads it.
  - `a_row_a_change_renders_reads_the_pages_parameters`: a reply's row,
    sent to an open thread page, reads it.
  - `a_reply_is_its_threads_to_every_reader_and_no_timelines`.
- **`e2e/feed.spec.mjs`, in three engines**: `a reply reaches its thread and
  every reader of it, and no timeline`.
- **`scripts/page_parameters_mutations.py`: 8 mutants**, recorded by `just
  e14-page-parameters`:
  - 7 against the compiler's and the server's tests;
  - 1 against the feed in three engines.
- **`pages_mutations.py`'s "a change is sent to the store's documents
  alone"** is re-anchored on a document's parameters, which replace the
  store it was read with.
- **The workspace, 2,099 tests; the browser suite, 773 in three engines.**

## Not claimed

- **A value computed from a parameter** (ruling 0073-a, later).
- **A parameter the browser holds**, in a block a signal decides or a
  speculated region.
- **An opaque value's representation in a template**: ADR-0232.
- **An optimistic reply.** It needs ruling 0122-d, and speculation on a
  value of a type that contains itself (ADR-0205 §5).
