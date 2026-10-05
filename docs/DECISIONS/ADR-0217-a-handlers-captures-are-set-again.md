# ADR-0217: what a handler at the top of the page captures is set again when it changes

Status: accepted under the owner's delegation of 2026-10-02. It fixes
ADR-0210's urgent defect 2, under the rule the owner gave with it: "every
captured path is either re-rendered by each patch that can change it, or
refused at check". Date: 2026-10-05. Milestone: E14.

## Context

- **A handler's captures are written on its element** (ADR-0172), and the
  compiled handler reads them when it runs (ADR-0134).
- **A row's were set again with the row**, by the server and by a
  speculation (ADR-0172).
- **A handler at the top of the page was in no plan.** Reproduced: a button
  on the cart's page capturing `cart` (`resumable(captures = { cart }) =>
  kept = cart.lines`) checked and built. Its page's plan listed the
  attribute, the text parts and the blocks, and nothing for the button.
  - After a line was added, the server's patch left its captures as the
    page was first rendered, and a press sent the old cart.
  - So did a speculation, between the press of Add and the server's answer.
- **A top-level handler capturing what the page shows is common.** A feed's
  post page with a share or reply button captures its post. Refusing it would
  refuse ordinary programs, so this re-renders it.

## Decision

- **A handler's captures are a read of the template**, `ReadKind::Captures`,
  at its part, through each view around it (ADR-0136).
- **The page's plan lists each element at the top whose handlers capture a
  query's value**, by its first handler's part (`captures`). The server
  renders its captures with the page's values (`pw_render::captures_at`), and
  patches them where they change, as it patches an attribute (ADR-0171).
- **A speculation renders them again in the browser.** A `captures` region,
  one for each element, is given the element's handler parts. The browser's
  renderer writes the attribute as the server does (`captures_value`), and
  the runtime sets it.
- **Or refused at check, as the rule allows.** A handler inside a block that
  a speculation does not render again, capturing the speculated value, is
  refused as any read there is (ADR-0172). One inside a region that captures
  what the browser does not hold is refused by name, as before.

## Acceptance

Recorded by `just e14-top-captures` in `docs/evidence/E14/top-captures.txt`:

- **The dev server's test** `a_handlers_captures_at_the_top_of_the_page_are_set_again`:
  the cart's page given the button; after Add, the patch sets its captures
  to the cart with the line.
- **`compiler/pw-core/tests/top_level_captures.rs`**, 2 tests:
  - the button is planned, and the cart's speculation has its `captures`
    region;
  - the store's own cart page, whose handlers are in rows, plans none.
- **`runtime/pw-render-wasm`**: an element's captures are written as the
  server writes them, every handler's in one attribute.
- **`scripts/top_captures_mutations.py`**: 5 mutants.
  `row_reads_mutations.py` and `query_attributes_mutations.py` are
  re-anchored.
