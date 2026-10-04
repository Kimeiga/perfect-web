# ADR-0171: an attribute at the top of the page that reads a query's value is set again when it changes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Found while designing the store's per-line cart: a message
that shows while the cart is empty is an attribute, `hidden={cart.lines}`, at
the top of the page.

## Context

A host sets again what a change reaches:
- a text part at the top of a page (ADR-0125);
- a list's rows, their text and attributes alike (ADR-0145, ADR-0168);
- a block a query decides, whole (ADR-0146).

An attribute at the top of a page is in none of these. Measured on the
benchmark's store on 2026-10-03, with `<p id="empty"
hidden={cart.lines}>Your cart is empty.</p>` added: the message stayed shown
after a line was added. The document kept the attribute's first value for
its life, and nothing refused the page.

## Decision

1. **The plan names each attribute at the top of a page that reads a
   query's value** (`attributes`, by part). One written with several values
   is named once. One in a block is rendered with its block, and one in a
   row with its row, so neither is named.
2. **A host sets one again when its value changes**, as it sets a text part:
   - `SetAttribute` with the value as the page's render writes it
     (`pw_render::attribute_value`, ADR-0168);
   - `RemoveAttribute` for a boolean one that goes.

   One whose value did not change is not sent.
3. **The runtime applies it as it applies one in a row** (ADR-0168), at an
   address with no instance.

## Found on the way

- **A test of ADR-0152 checked "never shown" by looking twice.**
  `keyed.spec.mjs`'s test 7 gave Cold's answer one second to show, and then
  looked once more after Hot's answer was due.
  - Under the full suite's load, Firefox's read of Cold took longer than the
    second, and the test failed. It passed five runs of five alone.
  - It now records every list the page shows, and asserts that none was
    Hot's answer. Cold's answer gets Playwright's own wait.
  - The claim is the same, and it is now tested at every moment.

## Alternatives

- **Refuse such an attribute.** The page would lose a plain form, such as a
  checkout button disabled while the cart is empty, for no reason a host
  cannot meet.
- **Render the element again.** That loses its focus and its state. ADR-0168
  set attributes in place for the same reason.

## Acceptance

- `compiler/pw-core/tests/query_attributes.rs`:
  - an attribute at the top that reads a query is named;
  - one written with several values is named once;
  - one in a block or a row is not named.
- The development server's test: the message is hidden with the first line,
  not set again while it stays hidden, and shown again when the cart is
  cleared. A document served then shows it.
- `scripts/query_attributes_mutations.py`: 6 mutants, recorded by
  `just e14-query-attributes`.
- The browser suite: 529 passed, 2 skipped.

## Not claimed

- **In a browser at the top of a page.** The canonical store has no such
  attribute yet. Its cart is speculated, and ADR-0170 refuses an attribute
  that reads it, until the browser renders one again (the next ruling). The
  runtime's path is the one ADR-0168 runs in three engines, at an address
  without an instance.
