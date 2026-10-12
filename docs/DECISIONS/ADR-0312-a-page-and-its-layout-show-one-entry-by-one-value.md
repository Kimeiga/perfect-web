# ADR-0312: a page and its layout show one entry by one value

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Found 2026-10-11, recorded
in ADR-0191's amendment and in NEXT.md ("one entry for a query a page and
its layout both bind"). Builds on ADR-0122 (optimistic transitions),
ADR-0172 (what a speculation renders again), ADR-0222 (a value of any query
speculated on), ADR-0303 (a page shown in its layout) and ADR-0306 (soft
navigation).

## Context

- **ADR-0303 reads a query a page and its layout both bind once for the
  document.** The store's four pages show the session's cart, and their
  layout, `StoreLayout`, shows its count in the header. The plan names the
  layout's binding `cart~StoreLayout`, and the host reads `Cart` once.
- **The speculation did not.** Each page's speculation module listed two
  bindings for the one entry, `cart` and `cart~StoreLayout`:
  - each had its own decoder and its own transition for each command, so a
    press ran the transition twice, once for each name;
  - the host sent each its own value, the materializer's entry for `cart`
    and a speculated value for `cart~StoreLayout` (ADR-0222's path), each at
    its own version, so a change was sent twice;
  - the runtime held two states for the one entry, each reconciled by its
    own frame.
- Nothing showed a wrong count, since the two states moved together. But a
  change cost two frames and two transitions, and two versions of one entry
  could be told apart only by the order their frames arrived in.

## Decision

1. **One entry, one binding.** Where several bindings a page shows, the
   page's and its layout's, read the entry a speculation targets, the first
   is the entry's binding. That is the page's own, as the page's bindings
   come before its layout's. The others are its **aliases**.
   - An alias has no decoder, no transition and no value of its own.
   - Its parts, its computed values and its regions are rendered from the
     binding's value, under its own name, the one its template reads.
2. **The module says it**: `export const aliases = { "cart~StoreLayout":
   "cart" }`. The manifest's binding lists its aliases (`aliases`), and no
   alias is a binding of its own.
3. **The host sends the entry once.** Its speculated values are the
   manifest's bindings, so nothing is sent by an alias's name. The store's
   cart is the materializer's entry, as before. The document carries one
   entry for it.
4. **The runtime renders an entry by every name.** Rendering a binding
   renders its own parts and regions and each alias's, from one value, held
   and reconciled once. The same holds before a batch's patches, where each
   list is shown as held.

## Alternatives

- **Key the speculation by entry, not by binding.** The module's parts,
  computed values and regions are the template's, which reads by name. An
  alias keeps those as they are and adds one table.
- **Read the layout's count from the page's binding in the plan.** That
  would make the layout's template depend on the page that composes it,
  which ADR-0303 rules out: a layout is numbered and planned the same on
  every page.

## Acceptance

- **`pw-core`, `tests/layouts.rs`**: the store's page's module has one
  binding, `cart`, with the layout's name as its alias. Each command's
  transition is the binding's alone, and the layout's count is a part it
  renders.
- **`pw-conformance`, `tests/speculation.rs`**: the module run under Node.
  The alias is no decoder's, no command moves it, and its part reads the
  cart's value.
- **The development server's test**
  `a_page_and_its_layout_show_one_entry_by_one_value`: the store's
  document carries one entry, `cart`, and nothing is speculated by the
  layout's name.
- **`e2e/layouts.spec.mjs`**, in three engines: the layout's count moves
  with the page's before the server answers, from one speculation, and the
  server's value reconciles it once.
- **Mutation controls**: `scripts/one_entry_mutations.py`.

## Not claimed

- **A layout's region over a speculated entry**: a block, an attribute or a
  list a layout renders from it. It is rendered by the same names, but no
  program here has one. The store's layout shows its count, a text part, so
  only the shared path is exercised in a browser.
- **Two pages' bindings of one entry in one document**: a page shows one
  page. A view's instance reads its page's binding (ADR-0234).
