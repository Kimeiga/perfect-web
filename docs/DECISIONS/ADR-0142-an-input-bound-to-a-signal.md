# ADR-0142: an input bound to a signal

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward T06. Builds ADR-0131's fifth ruling for a `String`
signal, and what it needs: an attribute a signal decides, set in place, and
a block rendered again only for what cannot be.

## Context

ADR-0131 ruled `bind:value={s}` shorthand for `value={s}` plus an `on:input`
handler writing `s`, as Marko's `value:=` is. Nothing of it was built:
- `bind:value` was a directive no rule read, and the template IR blocked it
  at build;
- an attribute a signal decides was refused by the plan (ADR-0130,
  ADR-0137), because the browser did not set one again;
- a block a signal decides was rendered again for every signal read
  anywhere in it. A field bound to a signal inside one, a form in a dialog,
  would have been replaced at each key pressed, and lost its focus.

So a field that shows what it holds and tells the page what was typed could
not be written. That is the first half of every form.

## Decision

1. **`bind:value={s}` is lowered to the two attributes it is short for**:
   `value={s}`, and `on:input` with a handler that sets `s` to the event's
   value.
   - Every rule then reads them as it reads any attribute and handler: the
     handler's identity, module and event (ADR-0134, ADR-0138), where the
     signal is read and written (ADR-0133), and what the browser renders
     again (ADR-0137).
   - The handler's event has a name no source can write, so it hides
     nothing.
   - `Body::bound` says which handlers a binding wrote.
2. **A binding binds a signal of `String`, by its name, on a field**
   (PW5304, new). Each of these is refused, with one diagnostic:
   - a value that is not a signal: the page's body runs once, on the server,
     and what is typed would change nothing the browser holds;
   - a signal of another type: it needs a codec, a parse to `Result` and a
     format back (ADR-0131), which is not built;
   - an element that is not an `<input>`, a `<textarea>` or a `<select>`;
   - any other `bind:`.

   The rules that would read the lowered assignment or handler a second way
   leave a binding's to PW5304: PW0607, PW0611, PW5302, PW5013 and the
   capture checks.
3. **An attribute a signal decides is set in place**, outside any loop, at
   top level or in a block a signal decides:
   - `value` as the field's property;
   - a boolean attribute by its presence, and its property where the
     element has one;
   - any other by `setAttribute`.

   An attribute whose value is a URL or a style, or one written with holes,
   stays refused. The renderer checks a URL's scheme and refuses CSS it
   cannot escape, and doing either in the browser would check what the
   server checks a second way.
4. **A field's value is never set from its own typing.** What it holds is
   what was typed. The browser does not write back the value its own input
   handler set, which a handler that set it after an `await` could deliver
   behind later keys, moving the caret under the person typing. Any other
   change is shown, focus or not: a handler that clears the field on Enter
   clears it.
   - This refines ADR-0131's "an update never overwrites an input that has
     focus", which would also have hidden the deliberate change.
   - A bound handler is synchronous, so its own typing never arrives late.
     The rule guards a handler written to set its field's signal after an
     `await`, and no test exercises that case.
5. **A text part or an attribute is set in place inside a block a signal
   decides**, as at top level. The block is rendered again only for what it
   cannot set in place:
   - its own signal;
   - a signal read inside a loop in it, or by a part the browser does not
     set (`Live.reads`).

   A field bound inside a block keeps its element and its focus while typed
   in.

## Alternatives

- **Bind in the runtime**, the browser reading fields into signals by name.
  Rejected. It is code the compiler did not check: a typo in the signal's
  name, or a binding to a value the server holds, would be a field that does
  nothing.
- **Never set a focused field** (ADR-0131 as written). Rejected, as above: a
  handler clearing the field would show nothing until the field lost focus.
- **Render a block again and restore the focused element.** Rejected. The
  element is new, so the caret, the selection and an IME composition are
  lost, which is the thing being protected.

## Acceptance

- **`compiler/pw-core/tests/bind.rs`, 3 tests**, each with controls:
  - a binding is a value and a handler, set in place, run under Node;
  - a binding binds a signal of `String`, by its name, on a field, with one
    diagnostic for each wrong one;
  - a block is rendered again only for what it cannot set in place.
- **`signals_render_again.rs`:** an attribute a signal decides is set in
  place, and a URL is still refused.
- **`spikes/own-renderer/e2e/bind.spec.mjs`, 6 tests**, in Chromium,
  Firefox and WebKit, over `examples/demo/bind.pw`:
  - the server renders the first value;
  - typing sets the signal and what reads it;
  - a handler's change sets the field, even while it has focus;
  - the field's own typing is never written back over it;
  - a field in a signal's block keeps its element and its focus while typed
    in;
  - markup typed is text.
- ADR-0133's and ADR-0136's plan tests now expect a block's narrower reads.
- **Mutation controls:** `scripts/bind_mutations.py`, `just e14-bind`, 14
  mutants. `signals_mutations.py`'s "a block reads its own signal alone"
  moved here, with the code it tested.

## Not claimed

- **A codec** for a signal of another type, a checkbox's `checked`, a
  `<select>` of cases.
- **A live attribute whose value is a URL or a style**, an interpolated
  attribute a signal decides, and anything a signal decides inside a loop a
  query decides.
- **Forms** as one typed record (ADR-0131, ruling 6).
