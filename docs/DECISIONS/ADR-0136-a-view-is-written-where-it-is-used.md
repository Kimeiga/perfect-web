# ADR-0136: a view is written where it is used

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward the DoorDash target. Builds ADR-0130's second step,
"view composition, with handlers and loops inside views", and replaces
ADR-0072's refusal of a view used in another.

## Context

A page could not be built from views. `<MenuRow item={item} />`, the
charter's own way to use one view in another (§8.1), was refused (PW5020,
ADR-0072) because nothing compiled it. So every page was one block of markup,
and a DoorDash-sized app would be written as a few very large pages.

ADR-0130 ruled how views compose, which is Marko 6's model: a view is
compiled once and its markup is spliced into each template that uses it.
Props, labels and placement are resolved per use site, at compile time.

## Decision

```pleris
view MenuLine(item: MenuItem) !{} {
    <li><span>{item.name}</span><AddButton entry={item} /></li>
}

view AddButton(entry: MenuItem) !{} {
    <button type="button" on:press={() => add_to_cart(entry.id, PositiveInt(1))}>Add</button>
}

page StorePage(id: StoreId) {
    let menu = query Menu(id)
    view { <ul>{#each menu as item (item.id)}<MenuLine item={item} />{/each}</ul> }
}
```

### Composition (`template_ir`)

1. **A view used in another is lowered in place**, in the template's one
   numbering. Each of its parameters reads as the path its prop gives:
   `{item.name}` in `MenuLine`, given `item={item}`, is the page's
   `item.name`.
   - The composed page is the document the same markup written in place
     makes, chunk for chunk, with the same schema.
   - Two uses of one view get parts of their own, numbered in document
     order. ADR-0130 named each part by an instance path and its number
     within the view. One numbering per template gives the same guarantee,
     and it is the project's rule that one traversal assigns identity.
     Instance paths come with run-time instances (below).
2. **Composition is hygienic.** A name the view binds hides its own
   parameter of that name. It must not hide what the page gave it, so it is
   renamed to one no source can write, `entry~1`, wherever it would.
   - Example: `{#each item.tags as entry}`, given the page's `entry` as
     `item`, would otherwise read the tag where it meant the page's `entry`.
   - Loops and `{#match}` arms are both handled.
3. **A handler written in a view keeps its own identity and module**, from
   the view's file and declaration (ADR-0135). The event part carries
   `renames`, which say where the page holds each name the handler
   captures: `entry` is the page's `item`.
   - The renderer reads `item.id` and writes it under `entry.id`, which is
     what the view's compiled handler reads.
   - `renames` is empty, and not serialized, where every name is the
     handler's own, so no existing template changes.
4. **The page plan and the speculation read the composed template.**
   - A text hole carries the path the template reads, through each view
     around it, and the declaration its expression is written in. So a
     query value shown through a view is planned as the page's own
     (ADR-0125).
   - A speculated part shown by a view is recomputed from the view's
     expression, given the binding whole (ADR-0122).
   - A view's handler is in the page's document, so its optimistic command
     is the page's to speculate for.

### What is checked where a view is used

5. **Props are checked as a call's arguments are** (PW0619, new). Each
   parameter is given, of its type, and nothing else is.
   - A prop is a value path, as every template value is (ADR-0073). A
     computed one is PW5020.
   - A prop written as text is refused with its repair.
   - A prop written twice is the markup rule every attribute has (PW0028).
6. **What a view's handler captures is checked where it is given.** A
   view's handler that captures a parameter, or a value read from one
   through the view's own loops and arms, writes the given value into the
   page's document.
   - The page checks that value as it checks its own captures: PW5007 for
     the shared shell, PW5018 for a private page that names no principal.
   - The view's own check could not: it sees a parameter, and a
     `session view` may hold a session's value itself.
   - `resume::captured_params` computes, over the whole program, which
     parameters each view's handlers capture, through the views it uses.
7. **A signal given to a view whose handler captures it is refused**
   (PW5301). A handler reads a signal through its context, as it is when the
   handler runs. A captured copy is the value the document was rendered
   with, and pressing after the signal changed would act on the old value.
   - A view that only shows a signal is fine: its part is the page's, and
     live as the page's own.
   - Changing a signal from a view waits for provided signals (ADR-0130,
     step 3).
8. **What is not composed yet is refused by name** (PW5020, and a `Blocked`
   part if the checker is bypassed):
   - a view with bindings of its own (a `let` or a `signal`);
   - a view that contains itself, directly or through others;
   - a page or a component used as an element.

### The development server

9. **A page of signals takes its parameters from the address**:
   `/page/demo.pick.PickPage?first=espresso&second=cold-brew`, as text,
   percent-decoded. A parameter not given is not rendered with a guess.

## Alternatives

- **Render a child template in place at run time** (`Part::Component`, which
  the renderer already has). Rejected, as ADR-0072 and ADR-0130 rejected it:
  - the child's part and element numbers repeat the parent's in one
    document;
  - a loop's identity is lost at the boundary;
  - the resume gate and the browser would need instance-qualified
    addresses.
  Splicing needs none of that, and the renderer, the plan, the browser and
  the gate read a composed page as they read any page.
- **Refuse a binding that collides, instead of renaming it.** Rejected. Using
  a view would then depend on the names inside it, written in another file
  by someone else. Scheme's hygienic macros, and every component compiler,
  keep a component's names its own.
- **Check a view's captures with the labels of every page that uses it, at
  the view.** Rejected. A view's handler is checked against the page its
  markup goes into, and a view used in a session page and a public page has
  two answers. The use site is where the destination is known.
- **Allow literal props** (`label="Add"`), composed as constants. Not yet.
  A constant must reach every context a path reaches: text, attributes, URL
  components, captures, and nested views. Recorded below.

## Consequences

- No accepted, store or kiokun program changes, and no template the store
  builds changes: `renames` is empty there and not serialized.
- `examples/demo/panel.pw` shows its signals through two views, and its
  document is byte-identical to before. `examples/demo/pick.pw` uses one view
  twice, each use given another item, with a command that answers with what
  it was given.
- ADR-0072's mutants are re-anchored. `a view is not told from another
  declaration` is now about a page or component used as an element.

## Acceptance

- **`compiler/pw-core/tests/views_compose.rs`, 11 tests**, each with a
  control:
  - a composed page is the inlined page;
  - two uses have parts of their own;
  - a view's handler captures what the page gave it, renamed, with its own
    module;
  - hygiene, through loops, arms and a parameter the view shadows;
  - the plan of a composed page is the inlined page's;
  - a view's part and handler are the page's to speculate for;
  - refusals by name, a nested view's reported once, where it is used;
  - captures checked at the use site, directly and through the view's loop;
  - a signal a view shows is live;
  - a signal a view's handler captures is refused.
- **`compiler/pw-core/tests/view_elements.rs`, 4 tests** (ADR-0072's, now
  the composition rules), including a page used as an element.
- **`runtime/pw-render/tests/properties.rs`:** a capture a view names
  otherwise is read where it is, with a control.
- **`spikes/own-renderer/e2e/views.spec.mjs`, 5 tests**, in Chromium,
  Firefox and WebKit:
  - a composed view is markup, not an element;
  - a signal a view shows is rendered again, at top level and inside a
    block;
  - each use carries its own item under the view's name for it;
  - a press sends that use's item;
  - a parameter is the address's, escaped where it is written.
- The own-renderer suite passes across its browsers.
- **Mutation controls:** `scripts/views_compose_mutations.py`, `just
  e14-views-compose`, 17 mutants.

## Not claimed

- **A view with bindings or signals of its own**, and **provided signals**
  (ADR-0130, step 3).
- **A view that contains itself** (a tree, a thread of replies). ADR-0130
  rules it an instance made at run time, inside a block, and that needs
  instance-qualified addresses. Built by ADR-0203 (2026-10-05).
- **A literal prop**, `label="Add"`.
- **The Marko adapter** (ADR-0017) is unchanged. It writes a view used in
  another as a tag named after the view, and each view to a file of its
  name. Nothing here tests that Marko resolves the tag.
- **A view's own check keeps its destination**: a view's handler that
  captures a restricted parameter is refused at the view, even where every
  page using it could hold the value. The use site is the precise check; the
  view's is conservative.
