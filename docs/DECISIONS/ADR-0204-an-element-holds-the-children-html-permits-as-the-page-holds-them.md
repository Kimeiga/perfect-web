# ADR-0204: an element holds only the children HTML permits, as the page holds them

Status: accepted under the owner's delegation of 2026-10-02. Date:
2026-10-05. Milestone: E14. Found by ADR-0203's first test.

## Context

- **PW5012 read each constrained element's children as written**: of
  `<ul>`, `<ol>`, `<dl>`, `<table>`, `<thead>`, `<tbody>`, `<tfoot>`, `<tr>`
  and `<select>`, only an element written directly inside.
- **So it was wrong both ways.**
  - **A view's use was read by its name.** `<ul><Thread c={c} /></ul>` was
    refused, "`<Thread>` is not permitted as a child of `<ul>`", though
    `Thread` renders an `<li>`. A view could not be used directly in a list
    or a table.
  - **A block's rows were not read.**
    `<ul>{#each items as i (i.id)}<div>..</div>{/each}</ul>` passed, every
    row a `<div>` in a `<ul>`. One of the compiler's own test fixtures had
    such rows: a `<button>` in a `<ul>`.
- **HTML's content model**, which validators and assistive technology read
  a page by: a `<ul>`'s is "zero or more `li` and script-supporting
  elements" (WHATWG HTML, the `ul` element). What the page holds is what is
  checked against it, not what the source names.
- **The rule's explanation said a browser "silently reparses" invalid
  nesting.** Partly true. In a table, content the table does not permit is
  foster parented, placed before it (WHATWG HTML, "in table"). A `<div>`
  start tag in a `<ul>` is inserted where it is (WHATWG HTML, "in body").

## Decision

1. **An element's children are what the page holds there:**
   - each element written there;
   - each row and branch of a block written there, `{#each}`, `{#if}` or
     `{#match}`, and of the blocks inside it;
   - **the elements a view used there renders at its top**: through the
     blocks at its top, and the views it uses there, each resolved where
     that view is declared. A view met again on the way is read once, so
     one that contains itself (ADR-0203) ends.
2. **Each is checked against the element's content model**, as before. A
   view's is named as what it renders: "`<Cells>` renders `<td>`, which is
   not permitted as a child of `<tbody>`".
3. **The rule runs over the whole program**, beside the check of view
   elements, since a view used in one module may be declared in another.
4. **The explanation says what a browser does**: moves some of what HTML
   does not permit, as a table's stray content, and keeps the rest where
   the content model says it does not belong.

## Alternatives

- **Leave a view's use unchecked.** A view whose `<div>` lands in a `<ul>`
  would pass.
- **Check the page as composed.** A view that contains itself is never
  composed (ADR-0203), so what its instances render would go unread, and
  each refusal would point at composed markup, not where the source wrote
  the use.

## Acceptance

Recorded by `just e14-rendered-children` in
`docs/evidence/E14/rendered-children.txt`:

- **`compiler/pw-core/tests/children_as_rendered.rs`**, 5 tests:
  - a view read by what it renders, accepted and refused;
  - a block's rows and each branch;
  - a view read through its blocks and the views it uses;
  - a view from another module, through a view it uses that the using
    module does not import;
  - a view that contains itself read once.
- **`compiler/pw-core/tests/views_compose.rs`**: the fixture's rows are
  `<li>`s.
- **R-019 and the generality pair `invalid_nesting`**: unchanged.
- **`scripts/rendered_children_mutations.py`**: 8 mutants.

## Not claimed

- **Text in a constrained element**, `<ul>text</ul>`: the rule reads
  elements, as before.
- **The rest of HTML's content models**: the nine elements above, as
  before.
