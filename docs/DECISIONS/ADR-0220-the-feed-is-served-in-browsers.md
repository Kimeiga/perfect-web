# ADR-0220: the feed is served in browsers by the host that serves the store

Status: accepted under the owner's delegation of 2026-10-05. It is the feed
reference app's third step (ADR-0218, ADR-0219, NEXT 24). Date: 2026-10-05.
Milestone: E14.

## Context

- **The feed was served to tests, not to browsers.** ADR-0218's server tests
  served its home page and committed a post. ADR-0219's showed the post
  reaching another session's document. No browser had opened it.
- **A browser does more than those tests did.** It opens the page's stream
  as soon as the page has loaded, and it renders the page's head.

## What a browser found

1. **The feed's page reloaded itself for ever.**
   - A stream's request drains the session first (ADR-0176).
   - The drain regenerated the store's cart entry for every program: it read
     the store's `cart.line_count` on whatever page the session had open.
   - The feed's home page has no such part. The host found it unshowable and
     sent the page `recovery: reload`. The page read itself again, opened its
     stream again, and was told to reload again.
   - ADR-0218's tests served the page and never opened its stream.
2. **Every program's page carried the store's style.** The host wrote the
   store's menu's containment (`#menu > ul > li…`, ADR-0187) into every
   page's head. The feed has no menu.
3. **A guest's post read "You" to every reader.** The feed's data layer named
   a session's user "You" whenever the session had not signed up, which is
   every session. Another reader saw "You" on someone else's post.

## Decision

- **A data layer with no session entry has nothing to drain.** `drain_held`
  returns at once when the layer keeps no entry for the session
  (`DataLayer::session_entry`, ADR-0218). Its commits already send the
  session's documents at the host's clock. The store's cart, which is such
  an entry, is drained as before.
- **A page's style is its data layer's** (`DataLayer::style`).
  - The store's layer gives its menu's containment.
  - A layer with none writes no `<style>` element, not an empty one.
  - The constant stays where ADR-0187's mutants find it.
- **A guest is named for its session, the same to every reader**: `@{session}`,
  "Guest {session}". Signing up, and a name of one's own, are not in the
  feed's scope.
- **The feed has its own hosts in the browser suite**, one per engine, on
  ports 60 to 62 past the suite's: a post is every reader's.
  - `feed.sh` builds `dist-feed` from `examples/feed` and the platform's
    packages, and records its sources, as `run.sh` does.
  - The suite serves it only when it is built, with the current runtime,
    from its sources as they are, as it serves `dist-keyed`. Otherwise it
    warns and skips the spec.
  - `just e10-browser` builds it.

## Acceptance

- **The server's tests:**
  - `the_feeds_page_opening_its_stream_is_not_told_to_reload`: the feed's
    page polls its stream over HTTP and is answered without a reload.
  - `a_page_carries_its_data_layers_style_and_no_other`: the feed's home page
    has no `<style>`, and the store's page has the menu's.
  - `a_guests_post_names_the_guest_to_every_reader`: a post by session `a`
    reaches session `b` naming "Guest a", and nothing names "You".
- **`e2e/feed.spec.mjs`, three tests in each of three engines:**
  - the timeline, a thread with its replies as deep as they go, and a post
    that is not there answered 404;
  - a post reaching its author's timeline and another open reader's, with no
    reload in either, the draft cleared, and the author named alike in both;
  - a like's count reaching every reader.
- **`scripts/feed_served_mutations.py`: 5 mutants**, each killed:
  - the drain regenerating the store's cart for every program;
  - every page carrying the store's style;
  - the store's page carrying none;
  - a page with no style written an empty one;
  - a guest "You" to every reader.

  `just e14-feed` records them.
- **The workspace: 2,044 tests, the server's 130 among them.**
- **The browser suite with the feed's spec: 737 in four runs of five.**
  - The other run had one failure, a `toHaveText`, and three tests that did
    not run. It ran while another tree's mutation controls loaded the
    machine.
  - It was not the feed's spec: a failure in its three serial tests leaves
    at most two unrun.
  - The full output of every run is kept from now on, so a recurrence names
    the test.
- **Two older mutants re-anchored** to the shell's new form: the style moved
  into the body (`stable_layout_mutations.py`), and the store's page written
  without its metadata (`metadata_mutations.py`).

## Found, and not fixed here

- **A `<textarea>`'s bound value is rendered as an attribute.**
  `bind:value={draft}` renders `<textarea value="…">`, and a textarea has no
  `value` attribute. Its text is its content. A non-empty first value shows
  nothing until the runtime sets the element's value. The feed's draft
  starts empty, so it is not seen here. Next, in its own ADR.
- **Every page's resume manifest names the document schema `cart-doc`**, and
  so does the browser's resume decision.
  - The two agree for every program, so the check passes.
  - It compares a constant with itself, and tells no page's document from
    another's.
  - Recorded in KNOWN_LIMITATIONS, for the resume decision's own ADR.

## Not claimed

- **Pagination.** "Load more" asks for twenty more posts. The feed seeds one
  post that is not a reply, so nothing more is shown. Pagination is its own
  step, with enough posts to page.
- **Optimistic posting, a post's length, computed holes**: the feed's next
  steps (NEXT 24).
