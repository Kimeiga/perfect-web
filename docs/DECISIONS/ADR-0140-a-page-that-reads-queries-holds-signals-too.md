# ADR-0140: a page that reads queries holds signals too

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward T11. Extends ADR-0133 from a page of signals alone to
the store's page.

## Context

ADR-0133's signals reached the browser on one route, `/page/<path>`, which
serves a page whose values are its signals alone. The store's route rendered
its page from its queries and ignored its signals:
- the renderer had no value for a signal, so a store page that declared one
  did not render;
- the document carried no signals, so the browser had nothing to hold or
  render again.

So T11, a dialog on the store's page, could not be written in Pleris.

## Decision

1. **The store's route renders its page at each signal's first value**, as a
   page of signals alone is rendered (`with_signals`, one function for both
   routes).
2. **Its document carries the signals' manifest**: each signal's first
   value, the parts they decide, and each block's template. It is the same
   manifest a page of signals alone carries (`signal_manifest`, one function
   for both documents).
3. **What a block may read is ADR-0137's.** The browser renders a block a
   signal decides from the signals alone, so a store page's dialog holds
   signals and its own markup. A query's value inside it is refused at build.
4. **The browser's runtime is unchanged**: it holds the signals of any
   document, and a document that reads queries still subscribes.

## Alternatives

- **A route per page, rendering any page's queries and signals.** This is the
  server generality E14-Q owes, and is larger: the store's updates are
  written for the store's parts. The store's route is the one the benchmark
  serves, and T11 changes the store's page.

## Acceptance

- **`the_store_renders_its_signals_at_their_first_values`** (server unit
  test). The store's page is given a block its signal decides; it renders at
  the first value, and not at a shut one.
- **`the_store_s_document_carries_its_signals`**: the document carries the
  signals, the parts they decide and the blocks' templates. Control: a page
  with none carries none of it.
- **Mutation controls:** `scripts/store_signals_mutations.py`, `just
  e14-store-signals`, 2 mutants.
- In the browser, T11's Pleris reference patch is the test: the store's page
  with a dialog.
