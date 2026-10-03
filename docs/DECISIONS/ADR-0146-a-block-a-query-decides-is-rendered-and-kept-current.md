# ADR-0146: a block a query decides is rendered and kept current

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (E14-Q, fourth slice), for T04 and T10.

## Context

ADR-0145 found that the page plan left out a block a query's value decides,
`{#if cart.lines}` or `{#match order.status}`. A host had no value for its
subject, so the store with one checked, built, and failed at the server's
first render. ADR-0145 refused such a block in the plan until a host could
render it.

T04, an order's new state, needs `{#match}` over an order's status on the
store page. T10, an error state, needs `{#match}` over a query's result. Both
need such a block rendered, and kept current when its query changes.

Two related gaps:
- the server gave the renderer only the values of planned parts, so a field
  read inside a block had no value either;
- a component's case, a WIT variant, enum, option or result, reached the
  renderer as text, so no `{#match}` arm could match it.

## Decision

1. **The plan names each block a query's value decides, at the top of the
   page.** A block inside one is rendered with it, and is not named.
2. **The server gives the renderer each binding's whole value**, so any
   field read inside a block resolves. A component's case reaches the
   renderer as a case:
   - `Some`, `None`, `Ok` and `Err`, as Pleris writes them;
   - a declared case by its WIT name (ADR-0061);
   - a payload of several fields as a list.
3. **A block is rendered again where its rendering changed.** The server
   records each named block's rendering with what the document shows
   (ADR-0145). After a command it renders each block again, and sends a
   `ReplaceRange` patch in the change's `patch_set` for each one that
   differs; one that renders the same is not sent. The browser replaces the
   block's range, and reads its index again. A block is compared by what it
   renders, not by which bindings it reads, so a change that leaves it the
   same costs nothing.
4. **What a loop's row reads of another query is refused** (in the plan). A
   row is rendered again when its own item changes (ADR-0145). A block, or a
   value, in the row that reads another query would stay as it was when that
   query changed. Refused, naming the part, as the plan names what it cannot
   do.

## Alternatives

- **Render a block again whenever a binding it reads changes.** Rejected: a
  block over the cart would be sent at every press, and its nodes lost, when
  what it shows is the same.
- **Patch inside a block, part by part.** Deferred. A block's arm can change,
  and then nothing inside it is the same element; rendering it again is
  right then, and is right whenever it is cheap.

## Acceptance

- `page_blocks.rs`:
  - a block a query decides is planned;
  - one inside it is not;
  - the store as it is has none;
  - a row reading another query, by a block or by a value, is refused.
- Server tests:
  - a block renders from the cart's value;
  - it is sent again when it changes, and not when it renders the same;
  - a component's case is a case.
- Browser: `resource-path.spec.mjs` renders a range again where it is, in
  Chromium, Firefox and WebKit.
- `scripts/query_blocks_mutations.py`.

## Not claimed

- **A loop's row reading another query** (4).
- **A query's error.** The server still answers a query's `Err` as a failure,
  and T10 needs a page to show it.
- **Any page but the store's.**
