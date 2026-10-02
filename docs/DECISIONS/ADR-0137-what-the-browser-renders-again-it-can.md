# ADR-0137: what the browser renders again, it can

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14. A correction to ADR-0133, found writing ADR-0136.

## Context

The browser holds a page's signals and nothing else (ADR-0133). It renders
again two kinds of part when a signal changes:
- a text part outside any block, by setting its range;
- a block a signal decides, whole, by the server's renderer built for the
  browser, from the signals' values.

ADR-0133's limits said three other places a signal could be read were
"refused by the plan". Probing each with `pw build` found that one was:

| Written | ADR-0133 said | What `pw build` did |
|---|---|---|
| `<p title={title}>` | refused | refused |
| `{#each menu as item}{count}{/each}` | refused | built; `count` would show its first value forever |
| `{#if open}{#each menu as item}..{/each}{/if}` | refused | built; the browser cannot render the block again, holding no `menu` |

Neither is served today: a page of signals reads no query, and the store's
page has no signal. But `pw build` accepted both, and the limits said it did
not. A page that showed either would have failed silently in the browser:
- the first with a number that never moves;
- the second with a block whose render throws on the first press.

Probing found three more of the same kind, which ADR-0133 did not list:
- a block a signal decides inside a query's `{#each}`. Its instances are
  addressed with a frame, and the browser looks for it without one, so it is
  never found.
- an `{#each}` over a list a signal holds, outside any block a signal
  decides. It is rendered once.
- a handler inside a block a signal decides that captures a value the page
  gives. Rendered again from the signals alone, the capture has no value.

## Decision

**The plan refuses each part a signal decides that the browser does not
render again, and each part of a block the browser renders that reads what
the browser does not hold** (`page_values::rendered_again`). One walk over
the composed template (ADR-0136), with where each part is:

- **outside any block:**
  - a text part reading a signal is live by its range;
  - an `{#if}` or `{#match}` a signal decides is live, rendered whole;
  - an attribute a signal decides is refused, as before (ADR-0130);
  - an `{#each}` over a signal's list is refused.
- **inside a block a value other than a signal decides:** a part reading a
  signal is refused.
- **inside a block a signal decides:** every part reads a signal or a name
  bound inside the block, an arm's or a loop's, or it is refused. That
  includes what a handler there captures.

## Alternatives

- **Render the refused cases again instead.**
  - A signal read inside a query's loop reads the same value in every
    instance, so the browser could set each instance's range.
  - A query's value could be sent to the browser with the page.
  Both are worth doing when a page needs them. Neither is needed to stop
  building pages that fail silently, and each widens what the browser holds,
  which a label must then follow (ADR-0130, R8).
- **Refuse at `pw check`.** The plan is where it is known which parts a
  signal decides: through composed views (ADR-0136) and in the template's
  own numbering. A check-time rule would be a second derivation of the same
  fact.

## Acceptance

- **`compiler/pw-core/tests/signals_render_again.rs`, 5 tests**, each with
  controls:
  - a signal inside a query's block, as text and as a block, is refused,
    and the same reads outside any block and inside a signal's block build;
  - a query's loop inside a signal's block is refused, and arm names, a
    signal's list and signals build;
  - a signal's list is refused outside its block and builds inside one;
  - an attribute a signal decides is refused as before;
  - a handler in a signal's block that captures the page's parameter is
    refused, and builds outside it.
- The panel, the store and the pick page build; the own-renderer suite
  passes across its browsers.
- **Mutation controls:** `scripts/signals_render_again_mutations.py`, `just
  e14-signals-render-again`, 8 mutants.

## What the plan needs for it

The plan lowers the page with each handler's capture paths
(`resume::capture_map`), derived as the resume artifacts derive them, so it
knows what a handler in a block writes into the document. Until now it
lowered the page with none, which was harmless while it read only text.
