# ADR-0168: a change reaches every part that reads it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Part of the audit's ninth gap (§15.6 test 14): each Add named
by its item, and the cart's count said when it changes. Naming the buttons
found that a change did not reach an attribute.

## Context

- **Every Add button was named "Add".** A screen reader heard three buttons
  of one name, and nothing said which item each would add. The cart's count
  changed without a word.
- **Naming each by its item exposed a defect in patching.** The store's button
  became `aria-label="Add {item.name}"`, which the page renders as "Add
  Espresso". When E7-P renamed the item, the visible name changed and the
  button stayed "Add Espresso". Measured in the browser on 2026-10-03.
  - The development server patched a renamed item by one `ReplaceText` of its
    name's text part, a rule it held in its own code.
  - Its list diff (ADR-0145) set an item's text parts in place, and rendered
    again an item that held anything but text.
  - Neither set an attribute. The protocol declared `SetAttribute`, and the
    runtime did not apply it.

## Decision

1. **The store names each Add by its item and says its count.**
   - Each button is `aria-label="Add {item.name}"`. The name contains its
     visible label, "Add" (WCAG 2.5.3, label in name).
   - The count is "Items in cart: N" in a polite, atomic live region (WCAG
     4.1.3, status messages).
2. **What changed in an instance is the renderer's to say**
   (`pw_render::instance_changes`). From one value of a keyed loop's item to
   the next, it gives each text part's new text and each attribute's new
   value, in the order the template writes them. It gives `None` when
   anything else changed: a block, a component, a raw value, or what a
   handler captures. The instance is then rendered again. Each attribute's
   value comes from `attribute_value`, the one function the page's own
   render writes it with.
3. **A host sets those changes where they are.** A text part is set with
   `ReplaceText`, and an attribute with `SetAttribute` (`RemoveAttribute` for
   a boolean one now absent), each addressed inside the instance. The
   instance keeps its nodes, so focus and any state on them stay.
   - The development server's list diff uses it.
   - So does E7-P's rename, which now sends one patch set.
4. **The runtime applies an attribute patch.**
   - It finds the part's element through the parts manifest's owner, in the
     instance the address names.
   - The value is as the document's HTML writes it, and the browser's own
     parser reads it. A patched attribute therefore means what a rendered
     one does, `&amp;` included.

## Found on the way

- **A move to where an instance already is broke the list.** E7-P's runtime
  moved such an instance anyway. Its anchor was its own first node, and the
  atomic `moveBefore` placed its nodes before their own start. The list's
  comment anchors then no longer nested, and the next patch to the instance
  addressed nothing. Each keyed-list test's own restore made such a move.
  - It stayed hidden because a single `ReplaceText` that addressed nothing
    was ignored.
  - With a rename sent as a patch set, which refuses and reloads, the
    keyed-list suite failed about one run in four.
  - An instance already in place is not moved now; a test makes the move
    and then patches the instance.
- **The menu control decoded only `%20`.** `name=Cortado%20%26%20Milk`
  named an item "Cortado %26 Milk". It decodes as the other controls do now.
- **A block-planning test anchored its change on the store's markup**, which
  this ruling moved. It runs on the benchmark's frozen store now
  (ADR-0156), as the stream rules' tests do.

## Alternatives

- **Render the instance again whenever it holds more than text**, the list
  diff's rule. It is correct, and it destroys the item's nodes, focus with
  them. E7-P exists to keep them.
- **Send the attribute's value unescaped**, for `setAttribute` to take as it
  is. The page writes a static segment as the source does (`&amp;` stays
  `&amp;`), so an unescaped value would need a second decoding on the
  server. Parsing the written value in the browser has one reading only.
- **Name each Add with visually hidden text** inside the button. The label
  is the item's and does not need a hidden span; `aria-label` keeps the
  markup the template's.

## Acceptance

- `runtime/pw-render/tests/instance_changes.rs`:
  - nothing changed;
  - a rename sets the text and the attribute that reads the name;
  - an attribute is set as written, and a boolean one comes and goes;
  - a block that changed renders the instance again.
- The development server's tests:
  - a rename sets the name's text and the button's name, in one patch set;
  - the list diff sets a renamed item where it is, nodes kept.
- `e2e/stores.spec.mjs`, three engines: each Add is named by its item, and
  renamed with it, `&` and all.
- `e2e/keyed-list.spec.mjs`, three engines: a move to where an instance
  already is moves nothing, and the next patch to it applies.
- The browser suite: 526 passed, 2 skipped.
- `scripts/instance_changes_mutations.py`: 9 mutants, recorded by
  `just e14-instance-changes`.

## Not claimed

- **A single patch that addresses nothing is still ignored.** A patch set
  that does refuses and reloads. Making the single form refuse too is its own
  ruling.
- **Test 14's automated audit** (axe-core or similar) is not run yet.
  ADR-0182 runs one.
- **A row's block, or what its handler captures, changing in place.** The
  row is rendered again, as before.
