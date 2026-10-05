# ADR-0191: every page speculates from its own module

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. ADR-0190's first "not claimed".

## Context

- **What `pw build` writes.** A speculation module for every page whose
  presses call a command that declares `optimistic` (ADR-0122, ADR-0172).
  Each module comes with a manifest naming its page, the bindings it
  speculates on, its commands, and the regions the browser renders again.
- **What the development server read: the store's alone**,
  `speculations/store.page.StorePage.json`, by that name. ADR-0190 served
  the cart's own page, and kept it from carrying the store's speculation.
  So the cart page showed a press only when the server answered, while the
  store's page showed it at once.
- **The cart page's module was there.** Its manifest names the `cart`
  binding of `store.page.Cart(current_session())`, the three commands its
  lines press, and three regions: the empty message's `hidden`, the lines,
  and the fees block.

## Decision

1. **The server reads every page's speculation manifest**, by the page it
   names, and serves any module a manifest names.
2. **A document carries its own page's speculation**: the module, the
   regions, and the entries it starts from.
3. **A change's speculated value is sent to each document whose page
   speculates on it**, by that page's own manifest.
4. **The cart's page shows a press before the server answers**, as the
   store's does. A refused press is restored to the value the page held.

## Alternatives

- **Speculate only on the page a command was declared beside.** A command is
  a program's, not a page's. Any page that presses it holds the value it
  changes.
- **Merge every page's speculation into one module.** A page would download
  the transitions of commands it never presses.

## Acceptance

Recorded by `just e14-page-speculation` in
`docs/evidence/E14/page-speculation.txt`:

- **The development server's tests**: the cart page carries its own module,
  which is served, while a module no manifest names is not; a press sends
  the speculated value to the store's page and the cart's.
- **`e2e/pages.spec.mjs`**, in three engines: a line's quantity on the cart
  page moves before the server answers; a refused press restores it.
- **`scripts/page_speculation_mutations.py`**: 4 mutants. The two mutants of
  ADR-0190 that sent the store's speculation to every page are these,
  turned around.

## Not claimed

- **A speculation on a value other than the session's cart.** The server
  computes one key, `current_session()`, as it computes an event's
  (ADR-0104).
