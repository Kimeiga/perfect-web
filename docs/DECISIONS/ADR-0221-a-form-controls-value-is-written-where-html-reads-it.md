# ADR-0221: a form control's value is written where HTML reads it

Status: accepted under the owner's delegation of 2026-10-02. Found by the
feed reference app (ADR-0220). Date: 2026-10-05. Milestone: E14. Amends
ADR-0142, which let `bind:value` bind a `<select>`.

## Context

- **`bind:value={s}` is lowered to `value={s}` and a handler** (ADR-0142).
  The renderer wrote `value` as an attribute on whatever element it was on.
- **A `<textarea>` has no `value` attribute.** HTML reads its value from its
  text: its default value is its child text content, as the
  [standard](https://html.spec.whatwg.org/multipage/form-elements.html#the-textarea-element)
  defines it.
  - The feed's draft rendered `<textarea value="">`.
  - It starts empty, so nothing showed it.
  - A draft with a first value would have shown nothing until the runtime
    set the element's value, and nothing with scripts off.
- **A `<select>` has none either.** It shows the `<option>` marked
  `selected`, or its first. `bind:value` on a select whose signal starts at
  another option showed the first.
- **A value in a textarea's text was broken too.** `<textarea>{x}</textarea>`
  wrote the text part between comment markers. A textarea's content is text
  the parser does not read as markup, so the markers would show as text.

## Research

- **The parser drops one newline after a `<textarea>`'s start tag**, "as an
  authoring convenience" (HTML's tree construction, start tag `textarea`;
  [whatwg/html#5829](https://github.com/whatwg/html/issues/5829)). A value
  that starts with a newline must be written with one more, as React's server
  renderer does for `pre`, `listing` and `textarea`.
- **Content may not hold its own end tag.** A textarea holds escapable raw
  text: text and character references, and no `</textarea`. Text escaping
  (`&`, `<`, `>`) is enough.
- **Svelte treats `<textarea>{x}</textarea>` as `value={x}`, and refuses
  both at once.** Pleris keeps one way to write it, `value`, and refuses the
  rest. A select's value needs the option marked `selected`, which is a
  comparison per option: a computed hole (ruling 0073-a), which a template
  cannot state yet.

## Decision

- **A `<textarea>`'s value is written as its text.**
  - `bind:value={s}`, `value={s}` with `s` a signal, and `value="text"` are
    all written between its tags, escaped as text, a leading newline doubled.
  - The part keeps its number and its `value` name. The runtime sets the
    element's value in place as the signal changes, as it does for an input
    (ADR-0142). The plan lets a signal set it (`set_in_place`).
  - A patch carries it as an attribute's value, which is how a patch is read.
  - The renderer's context is `Content`, in the compiler's IR and the
    renderer's.
- **PW5036 (`form_control_value`) refuses what HTML would not read:**
  - a `value` or `bind:value` on a `<select>`. Its chosen option is marked
    `selected={..}` instead;
  - a `<textarea>` with a value and text;
  - a value in a textarea's text;
  - a textarea's `value` that is no signal: a query's value or a binding of
    the body. A change to it would reach the browser as an attribute, which
    a textarea does not read, so nothing would set it again. A parameter's
    is refused too, as is a value written with holes.

  The template refuses each as well, behind the check. PW5304's message no
  longer offers `<select>`.
- **The demo's bind page binds a `<textarea>`** with a two-line first value,
  shown with scripts off.

## Acceptance

- **`compiler/pw-core/tests/form_controls.rs`, 4 tests:**
  - the value written as text, escaped, a leading newline doubled; a value
    written in the source escaped alike; an input's value still its
    attribute;
  - the browser setting it in place;
  - PW5036's six refusals, each said once, and five controls;
  - the template's refusals behind the check.
- **The renderer's escaping matrix** gains a row, a value that closes its
  textarea and opens a script, with its unescaped control. A test pins the
  text, the newline and a patch's value.
- **`e2e/bind.spec.mjs`, two new tests in three engines:**
  - the first value is the textarea's text, with no `value` attribute, and
    shown with scripts off;
  - typing sets the signal, and a handler sets it back.
- **Corpus C15**: A-031, R-057, and `form_control_value`'s GENERAL and
  NEIGHBOUR witnesses. Generality is 42 / 42. Each PW5036 carries its
  boundary, the element or the text a value meets, as charter §16.3 asks.
- **The workspace, 2,050 tests, and the browser suite, 737 in three
  engines.**
- **`scripts/form_controls_mutations.py`: 12 mutants.** `just
  e14-form-controls` records them, with the tests above. Two older mutants
  are re-anchored: `bind_mutations.py`'s URL attribute set in place, and
  `row_reads_mutations.py`'s attribute that is no read.

## Not claimed

- **A select bound to a signal.** It waits for computed holes (ruling
  0073-a), which can mark the option whose value equals the signal's.
- **A textarea whose value a query gives.** It needs a patch the runtime
  applies to a textarea's default value, and a page that shows one.
- **An attribute written across lines in the source.** The parser refuses
  one, so a value written in the source never starts with a newline.
