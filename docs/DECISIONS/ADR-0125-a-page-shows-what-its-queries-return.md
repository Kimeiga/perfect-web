# ADR-0125: a page shows what its queries return

Status: accepted under the owner's delegation of 2026-10-02 ("start E14-Q";
"I will leave the decisions to you for now as long as you log them").
Date: 2026-10-02. Milestone: E14 (E14-Q, first slice).

## Context

ADR-0123 made the development server run the commands `pw build` built. What
the page *showed* was still the server's own: `store.name` was the literal
"Blue Bottle", the menu was its list, and `cart.line_count` was a sum in
Rust. The store's three queries, `Store`, `Menu` and `Cart`, compiled to
audited components that nothing ran. E10's build evidence said so ("the
queries compile and are audited; the server still computes the page's values
itself"). So a Pleris program could change a query's body and see no
difference, and six of E14's twelve tasks could not be graded on Pleris.

One value is not a field: `cart.line_count` reads `line_count`, a member
function of `Cart` (ADR-0048). Running the queries alone would leave the
server to compute it.

## Decision

1. **The compiler plans what each page shows** (`page_values`): each binding
   (`let cart = query Cart(current_session())`) with the query component it
   runs and its arguments, a page parameter or an invocation-context call;
   each text part outside a block as steps from its binding's value, each a
   record field or a member function; and each binding a block iterates.
   `pw build` writes the plan to `pages/<page>.json`. A page that cannot be
   planned is a build refusal, with the reason.
2. **A member function a plan names is a component.** It gets a contract of
   kind `function` and is lowered, compiled and audited like a query
   (`domain.line_count`: 2,203 bytes, no imports, placeable anywhere). It is
   not a dependency target: Pleris code calling it still compiles it into the
   caller.
3. **The server follows the plan.** It runs each binding's query component
   against its data layer, reads each part by its steps, calling a member's
   component where one is named, and renders the result. The cart patch, the
   speculation's value (ADR-0122) and the page all come from the same reads.
4. **The data layer answers what the queries import.** The server implements
   `store:data/stores#get` and `store:data/menus#for-store` beside the cart
   operations, as a deployment implements a database: one store, 47, and the
   keyed menu E7-P mutates.

## Alternatives

- **The page as one component returning all its values.** The WIT already
  models the page as a world importing its three queries. It is the cleaner
  end state, and it needs component-to-component linking in the host and a
  record type per page's values. Deferred, not rejected: it is where this
  goes once view composition (ADR-0072) is decided, because a composed view's
  parts would join the same record.
- **An interpreter for the backend IR inside the server.** Rejected: a second
  implementation of Pleris semantics, and the server would link the compiler,
  which ADR-0020's boundary forbids.
- **Keep computing values in Rust.** The defect this ADR removes.

## Acceptance

- `pw build` writes `pages/store.page.StorePage.json` (3 bindings, 2 parts,
  1 collection) and `components/domain.line_count.wasm`, audited.
- The development server's 20 tests pass on the plan, including
  `the_pages_values_are_its_queries`: no page value is written in the server
  (structural), the store name is the `Store` query's, and the count is
  `domain.line_count`'s.
- The own-renderer suite passes in WebKit and in Chromium (109 of 109 each),
  the E14 contract passes on Pleris (9 of 9), and T01's and T08's controls
  still hold on Pleris.
- `evidence_is_current` holds the three query components, `domain.line_count`
  and the page plan to the compiler's current output.

## Not claimed

- **Query policies.** Freshness, cache, key, concurrency, `on_key_change` and
  timeouts are not yet honoured: every render runs every query. E14-Q's second
  slice.
- A member read inside a block (refused by the plan), a binding keyed by
  anything but a page parameter or an invocation-context call, and any page
  but the store's: the server's route and its parameter are still the store's.
