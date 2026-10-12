# ADR-XXXX: a block is told part by part while its arm stays

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Builds on ADR-0146 (a block
a query decides is rendered and kept current), ADR-0168 and ADR-0178 (a
row's parts set where they are), ADR-0172 (what a speculation renders),
ADR-0231 (a page's parameter is the document's for its life) and ADR-0307
(what a page asks of its reader). Found by W9 (ADR-0308's amendment,
"Found"; ruled the host's to fix at its merge, 2026-10-12).

## Context

- **A part inside a block no query decides was never told.**
  - ADR-0146 plans a block at the top of the page when a query's value
    decides it, and a host renders it again when what it renders changed.
  - A block a page's parameter decides, or what the page asks of its
    reader, was planned by nothing. A part inside it that read a query kept
    what the page was served with.
  - W9's search page: the status sentence inside the `Some` arm of
    `{#match q}`. A menu change sent the open page the dish's row and not
    the new sentence. W9 moved the sentence out of the match.
  - The same held for every part that can read a query: a text, an
    attribute, a loop's rows, a block inside, a view's instance, and what a
    handler captures, whose press then sent what the page was served with.
- **A block a host renders was rendered again whole for any change inside
  it.**
  - ADR-0146's Alternatives deferred patching inside a block part by part:
    "a block's arm can change, and then nothing inside it is the same
    element; rendering it again is right then".
  - A live region inside such a block was a new element at each change, and
    a screen reader need not say a new region's text. A field inside it
    lost its focus: the defect ADR-0142 fixed for signals and ADR-0178 for
    a row's blocks.
  - A row has been told part by part since ADR-0168, and inside a
    conditional that decides as it did since ADR-0178.

## Decision

1. **The plan names each block at the top of the page that renders a
   query's value, whatever decides it.**
   - As before: a block a query's value decides (ADR-0146), or a value a
     host computes from one (ADR-0229).
   - Now also any block where something inside reads a query's value, at
     any depth: a text part, an attribute, a loop's list, a block's
     subject, a view's instance, a value a host computes, or what a
     handler captures.
   - A block inside one is still rendered with it, and not named.
2. **A host tells a block whose rendering changed part by part, while it
   shows the arm it showed.**
   - `pw_render::block_changes` reads the block as a row's change is read
     (ADR-0168), and returns:
     - each text part's text;
     - each attribute;
     - what each element's handlers capture;
     - each keyed list's operations.
   - It looks inside a conditional that decides as it did, and inside a
     `{#match}` that shows the arm it showed, its names bound to what each
     side gives them.
   - The block is rendered again whole, as before, where it shows another
     arm, or where something inside it changed that is not set in place:
     a view's instance, a raw value, an unkeyed list.
   - A block a page's parameter or its reader's answer decides shows one
     arm for the document's life, so it is always told part by part.
   - Inside a row, a `{#match}` that shows the arm it showed is now looked
     into too, as a conditional is (ADR-0178).
3. **A block a speculation renders is rendered again whole.** The browser
   renders it from the value it speculates on (ADR-0172), and may be
   showing it as it speculated. Only the whole block replaces what the
   browser shows with what the host derived.
4. **A host keeps the values a document's blocks were rendered from**, with
   what the document shows, where it has a block it tells part by part. It
   reads what the block showed from them. A block whose rendering did not
   change is sent nothing, as before.

## Alternatives

- **Plan the block, and render it again whole, as ADR-0146 does.**
  Rejected. The text is right, but W9's status region becomes a new element
  at each change, and a field inside loses its focus.
- **Plan the parts inside an arm the document holds as parts at the top of
  the page.** Rejected:
  - a second mechanism beside the row's;
  - a host would have to know which arm each document shows;
  - a block whose arm can change would still be rendered whole.
- **Compare a block's two renderings as HTML, and patch the difference.**
  Rejected: a host would parse what it has just rendered. The template
  already says where each part is.
- **Render a speculated block part by part in the browser too.** Deferred
  (Not claimed). The browser would read a block's change from the held
  value and from the speculated one, and its restore before a batch would
  do the same.

## Acceptance

- `runtime/pw-render/tests/block_changes.rs`, 7 tests:
  - a part inside an arm that stays is set where it is;
  - a keyed list inside it is changed where it is;
  - what the arm binds is read on each side;
  - a block that shows another arm is rendered again;
  - a block inside one is looked into while it decides as it did;
  - nothing changed is no change;
  - a block that is not there is refused.
- `tests/instance_changes.rs`: a `{#match}` in a row that shows the arm it
  showed is set where it is.
- `compiler/pw-core/tests/blocks_that_render_a_query.rs`:
  - the search's `{#match q}` is planned, and not where its arm holds text
    alone;
  - a block the reader's answer decides is planned for a text part, a
    value the host computes, a loop's list, an attribute and a handler's
    captures, and not where it renders no query's value;
  - a block inside one is rendered with it;
  - the cart's page plans what it did.
- The host:
  - `a_block_that_shows_the_arm_it_showed_is_told_part_by_part`;
  - `a_block_a_speculation_renders_is_rendered_again_whole`;
  - W9's `an_open_search_page_is_told_of_a_menu_change`, and
    `the_sentence_inside_its_arm_is_told_where_it_is` (a `ReplaceText`, no
    `ReplaceRange`). The sentence is back inside the `Some` arm, after the
    form, with the intro in the `None` arm.
- `e2e/store-search.spec.mjs`, three engines: the status region is the
  element it was across a menu change and back. With no `q` there is no
  status.
- `scripts/block_parts_mutations.py`, 14 mutants over the plan, the
  renderer, the host and the browser.
- Re-anchored and run in full, their code moved:
  - `query_blocks_mutations.py` (one mutant now needs a speculated block
    that renders as it did, which the host's tests hold);
  - `computed_conditions_mutations.py`;
  - `page_parameters_mutations.py`;
  - `template_mutations.py`;
  - `template_match_mutations.py`.

## Not claimed

- **A block a speculation renders is told whole**, by the host and by the
  browser.
- **A block whose arm changes is rendered again whole**, with every block
  inside it.
- **A view's instance, a raw value or an unkeyed list inside a block** that
  changed renders the block again whole.
- **The host keeps a copy of a document's values** where it has a block it
  tells part by part. It is cloned once per change, and not shared.
