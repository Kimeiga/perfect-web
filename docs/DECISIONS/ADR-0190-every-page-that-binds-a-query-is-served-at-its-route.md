# ADR-0190: every page that binds a query is served at its route

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, E14-Q's next slice. A store like DoorDash's is several pages
that read queries: a list of stores, a store, a cart, an order. Only one
could be served.

## Context

- **What the development server served.**
  - **The store's page**, at `/stores/{id}`. Its queries are read by the
    page's plan, its document is kept current, and each change is sent as
    the difference from what the document shows (ADR-0125, ADR-0145,
    ADR-0161).
  - **Any other page, only if it bound no query with `let`**, at its route
    or at `/page/<path>`, as a page of signals and streams (ADR-0130,
    ADR-0148). One that bound a query was refused: "binds a query; this
    route renders a page's signals and streams". KNOWN_LIMITATIONS said so.
- **Why.** The server read the store's plan, `self.plan`, in 21 places and
  its template in 8: a binding's arguments, a part's value, the lists a
  document keeps current, the blocks and attributes it shows, the patches
  derived for it. Nothing recorded which page a document was.
- **What a stream could do instead.** A `<stream>` region is rendered once
  per document, and a change to its query after it settles is not sent. A
  cart page has to change as the cart does.
- **What `pw build` already wrote for every page**: a plan with its
  bindings, parts, collections, blocks, attributes and title (ADR-0125,
  ADR-0145), and a speculation module (ADR-0172). A host could serve any
  page from them. This one did not.

## Decision

1. **The host serves every page that binds a query as it serves the
   store's**, at its route or at `/page/<path>`:
   - **its values are read by its own plan**, and it is rendered by its own
     template;
   - **its document records its page**, beside its parameters, and is
     forgotten with them;
   - **it is kept current**: a change a session makes is derived for each of
     the session's documents from that document's own plan, against what it
     shows, and addressed to its own template.
2. **What is the store's stays the store's.**
   - **The menu's public fragment**, its broadcast, and its refresh before a
     document is read. A document of another page is told nothing of the
     menu, though its parameters name the store.
   - **The speculation.** The server loads the store's speculation module
     alone, so no other page carries one, and no other document is sent the
     speculated values.
3. **A query's policy is found on whichever page binds it.** Invalidation
   read the store's plan alone.
4. **A page that binds a query and states no title is titled by its name**,
   as a page of signals is. The store's page keeps "Store" for the
   benchmark's copy (ADR-0183).
5. **The store has a second page: the cart, at `/cart`.** It shows the
   session's lines, changes a line's quantity, removes a line and clears the
   cart. The store's page links to it. A change made on either page reaches
   the other while both are open, because each reads
   `Cart(current_session())`.

## Alternatives

- **A server per page.** A session's documents would no longer share the
  frames that keep them current, and a press on one page would not reach
  another.
- **Keep such pages reading queries through streams.** A stream is
  rendered once. The cart page would show the cart as it was when the page
  loaded.
- **A public fragment for every shared list on every page**, as the menu's.
  The menu is the store's program's one shared list. A fragment for another
  page's shared list is the step when a page lists one, such as a list of
  stores.
- **Route by specificity**, a literal segment before a parameter. Two pages
  of one route's shape are refused already (PW0341), and no page of the
  store's program has one route that could shadow another.

## Acceptance

Recorded by `just e14-pages` in `docs/evidence/E14/pages.txt`:

- **The development server's tests**:
  - the cart page served at `/cart` and at `/page/store.page.CartPage`, by
    its own template and plan, with no speculation, showing the session's
    cart and not another's;
  - a press reaching the store's page and the cart page of one session, each
    patched against its own template, with the speculated value sent to the
    store's alone;
  - a cart page told nothing of the menu.
- **`e2e/pages.spec.mjs`**, in three engines: the cart's page at its route;
  a change made on either page reaching the other, open beside it; the
  store's page linking to the cart's.
- **`e2e/accessibility.spec.mjs`**: the cart's page keeps every rule, empty,
  with a line added on the store's page, with a quantity raised, and
  cleared.
- **`scripts/pages_mutations.py`**: 9 mutants.

## Not claimed

- **A page other than the store's speculates on nothing.** The compiler
  writes every page a speculation module, and the server loads the store's
  alone, so the cart page shows a press when the server answers.
- **A change to public data other than the menu** reaching the pages open on
  it.
- **The cart's markup is written twice**, on the store's page and on the
  cart's. Sharing it would take a view whose handlers call commands and set
  the page's signals.
- **A query no page but another binds.** None in the store's program, so
  finding its policy on that page is held by its reading alone, and by no
  test.
