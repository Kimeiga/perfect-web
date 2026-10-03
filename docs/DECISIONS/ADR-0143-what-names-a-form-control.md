# ADR-0143: what names a form control

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14. A correction to PW5014 (`control_without_label`), found while
writing T06.

## Context

PW5014 refuses a form control that nothing names. The rule looked only at
the control's own attributes, and took any of `aria-label`,
`aria-labelledby` and `id` as a name. It was wrong both ways.

**It accepted controls with no name.** An `id` counted as a name on the
assumption that some `<label for>` pointed at it, and nothing checked that
one did. Neither did anything check that an `aria-labelledby` reached an
element, or that an `aria-label` had text in it. Each of these passed:

```
<input id="q" type="text" name="q" placeholder="Query" />
<label for="other">Query</label><input id="q" type="text" name="q" />
<input type="text" name="q" aria-labelledby="nowhere" />
<input type="text" name="q" aria-label="" />
```

T06's unsafe Pleris store is the first line, a field with an `id` and a
placeholder. It passed `pw check` and was caught only by the hidden browser
test. The render fixture `examples/render/tricky.pw` had shipped such a
field, `<input id="field" …>` with no label, since E7.

**It refused controls that are named.** A control wrapped in its `<label>`
was refused, because the rule never looked at the control's ancestors:

```
<label>Query <input type="text" name="q" /></label>
```

## What the sources say

- **The HTML standard, "the label element".** A label with `for` names the
  first element in tree order whose `id` is that value, when that element
  is labelable. A label without `for` names its first labelable
  descendant. Labelable elements are `button`, `input` unless `hidden`,
  `meter`, `output`, `progress`, `select` and `textarea`.
- **accname 1.2.** `aria-labelledby` is used when it holds "at least one
  valid IDREF" (step 2B). An `aria-label` that is empty or blank is skipped
  (step 2C). A hidden node gives no text unless it is referenced directly
  (step 2A).
- **WAI's forms tutorial, "Labeling Controls".** It describes both
  association by `for` and `id` and association by wrapping, and notes that
  explicit labels are "generally" better supported by assistive technology.
  It recommends against `title` as a label.
- **WCAG technique H44** describes only `for`/`id`. It is one sufficient
  technique, not the definition.
- **axe-core's `label` rule** passes a field named by any of: an implicit
  or explicit label, `aria-label`, `aria-labelledby`, a non-empty `title`,
  or a non-empty placeholder.

## Decision

1. **A control is named only by something in the same declaration's markup
   that reaches it.** This follows the HTML standard and accname 1.2:
   - a `<label for=X>` with text names the first element, in tree order,
     whose `id` is `X`;
   - a `<label>` without `for`, with text, names its first labelable
     descendant;
   - `aria-labelledby` names its element when one of its IDREFs is an
     element there with text;
   - an `aria-label` names it unless it is blank.

   An `id` names nothing by itself.
2. **Text is read as accname reads it.** Text means any of these, inside
   the label:
   - a non-blank text node or an interpolation;
   - a descendant's non-blank `aria-label` or `alt`.

   A descendant that is `hidden` or `aria-hidden="true"` gives no text. A
   control's own subtree is left out, so a `<select>` is not named by its
   options.
3. **Where the checker cannot know, it refuses, and says why.** Each of
   these is refused, with the note naming the case:
   - a `for`, an `id` or an IDREF the program computes;
   - an `id` inside `{#each}`: every row has it, so a reference reaches only
     the first row's element;
   - a wrapping label where which control comes first depends on what
     renders, because it is in a block or in a composed view's markup.

   Wrapping the control in its own label is the repair that works in every
   one of them, a row of a list included. The diagnostic's note names the
   case: no label for this `id`, a label with no text, an earlier element
   with the same `id`, and so on.
4. **The label belongs to the declaration that renders the control.** A
   view that renders a field names it, so the view is accessible wherever it
   is used. A label in the page for a field in a view is refused. That is
   valid HTML, but the view alone would ship an unnamed field to the next
   page that composes it.
5. **`title` and a placeholder still name nothing**, as before. axe-core
   accepts both. Pleris holds the stricter line because both fail people:
   - a placeholder disappears as someone types, so the field loses its
     visible label while it is being filled in;
   - WAI recommends against `title`, which some assistive technology does
     not read as a label.

## Alternatives

- **Keep `id` as a name, and check `for` separately.** Rejected. It is two
  rules for one question, and the gap is in their seam: an `id` with no
  label would still pass the first.
- **Accept only explicit labels**, as H44 describes. Rejected. The standard
  defines wrapping, and every engine the suite runs computes the name from
  it. The parsed-tree suite asserts this in Chromium, Firefox and WebKit.
  Refusing it would refuse correct programs for a reason the sources no
  longer give.
- **Follow a label into the views a page composes.** Deferred. The pass
  sees one declaration, and the ruling in 4 makes the view's own markup the
  unit that is accessible or not.

## Acceptance

- `compiler/pw-core/tests/labels.rs`: ten tests, one per part above, each
  with controls. All ten fail on the rule before this change.
- Generality witnesses, in `examples/generality/control_without_label/`:
  - three caught: an `id` alone, a label for another `id`, and an
    `aria-labelledby` that reaches nothing;
  - two neighbours: a wrapping label, and `aria-labelledby` to a heading.
- `scripts/labels_mutations.py`: 16 mutants, each undoing one part.
- `examples/render/tricky.pw`: its unlabelled field is wrapped in a label,
  and a field named by `aria-labelledby` is added. The parsed-tree suite
  asserts that every accepted way gives its control the name, in all three
  engines.
- T06's controls: Pleris's unsafe store is caught at `pw check`.

## Not claimed

- **Whether a computed name is empty.** `aria-label={x}`, and a label whose
  text an interpolation or a block decides, are taken as names, though
  they may be empty when the page runs.
- **Duplicate `id`s as their own invariant.** A label naming an earlier
  element with the same `id` is reported here only because it leaves this
  control unnamed.
- **A computed `for`/`id` pair**, such as `for="qty-{line.id}"`, is not
  matched. Wrapping is the repair.
