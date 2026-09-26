# ADR-0072: an element named with a capital letter is a view

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §8.1, the
authoring format).

## Context

The charter's authoring sketch uses one view inside another,
`<Money value={item.price} />` (§8.1). A probe run after ADR-0071 found that
nothing read such an element. On 2026-09-26, at c2dab92:
- **It built as an unknown HTML element.** In
  `view V(k: Int) !{} { <div><Child n={k} /></div> }`, where `Child` is a
  view, the template IR held the literal markup `<Child data-pw="0" `, and
  an attribute part `n`. `Child`'s markup was never rendered. HTML
  lowercases every tag it parses, so a browser reads the element as an
  unknown `<child>`.
- **Its props were checked by nothing.** A prop of the wrong type
  (`n={"a"}`), one left out (`<Child />`) and one it does not take
  (`m={k}`) each checked.
- **A tag that names nothing** (`<Chidl />`) checked and built the same way.

The renderer can render one template inside another (`Part::Component`),
with the child's parameters bound from the parent's values. The compiler
never builds that part.

## Decision

An element whose name begins with a capital letter is a view (PW5020, "an
element named with a capital letter is a view the compiler composes"):
- **One that names a view, a component or a page is refused.** A view used
  in another view is not compiled yet.
- **One that names another declaration, or nothing, is refused** as not a
  view.

HTML elements and custom elements (`<my-widget>`) are written in lowercase,
and nothing changes for them.

**(ruling needed)** How a view composes. Two designs:
- **Inline it at compile time.** The child's markup is lowered into the
  parent's template, and its parameters become the parent's value paths.
  - One template has one numbering, so every part keeps a unique address.
  - A loop in the child is a loop in the parent, with the parent's instance
    path.
  - A view that contains itself cannot be inlined.
- **Render it in place at run time,** as `Part::Component` does today.
  - The child's parts are numbered by its own template, so they repeat the
    parent's addresses (`data-pw="0"` twice).
  - The child renders in a fresh environment that keeps only the granted
    capabilities, so its loop instances lose the parent's identity domain
    and path.
  - The page's part manifest lists the page's own parts, so a handler in
    the child would not be attached.

The first fits the static document parts (charter §8.4) and the resume
model as they stand. It is the design this project would build next, pending
a ruling.

## Acceptance

- **`compiler/pw-core/tests/view_elements.rs`**, 3 tests, each with
  controls:
  - a view used in a view, with a prop of the wrong type, one left out, and
    one it does not take;
  - a view imported from another module;
  - a tag that names no view, and one that names a type.
  All three fail at c2dab92, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** No
  example uses such an element. The store, kiokun and the accepted corpus
  check clean.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/view_element_mutations.py`,
  `just e10-view-elements`, 6 mutants.
