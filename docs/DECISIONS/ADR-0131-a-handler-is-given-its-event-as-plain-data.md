# ADR-0131: a handler is given its event as a record of plain data (ADR-0058's ruling)

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (T06, T11). Settles the two rulings ADR-0058 left open: the
event as a handler's parameter, and a command's answer read by its handler.
Nothing here is built yet; it lands with ADR-0130's order of work.

## The event

1. **An event is a closed record of plain data**, declared in
   `packages/pw-platform-web/events.pw`.
   - The element refines it: an `<input type="checkbox">`'s change gives
     `checked: Bool`, a text input's input gives `value: String`.
   - No DOM object reaches Pleris code. The record is all a handler can
     know about the event, and it is serializable, so a recorded
     interaction can be replayed.
   - Elm's decoders and Ur/Web's typed forms do the same; Qwik and Marko
     hand the handler the DOM event.
2. **The runtime reads the record synchronously, in the listener**, before
   the handler's module has loaded. The module is fetched on first press
   (E7-L), and by then the DOM event is spent.
3. **What must happen synchronously is declared, not computed:**
   `on:submit|prevent`. The modifier is static, so the runtime can act on it
   before any code loads.
   - Qwik's `sync$` asks a synchronous handler not to close over state and
     cannot check it. A modifier has nothing to check.
   - A synchronous predicate the compiler proves pure and capture-free is
     the extension, if a task needs one.
4. **A handler binds the event by a typed parameter:** `on:input={(e:
   InputEvent) => query = e.value}`. A handler with no parameter is
   unchanged, and the parameter's type must be the element's event, or a
   compile error names both.

## Two-way binding

5. **`bind:value={s}` is shorthand** for `value={s}` plus `on:input` writing
   `s`, as Marko's `value:=` is.
   - A `String` signal binds directly.
   - Any other type names a codec, a parse to `Result` and a format back:
     `bind:value={quantity, codec: PositiveInt.text}`.
   - Text that does not parse stays in the input and is not written: the
     signal keeps its last good value, and the codec's error is shown where
     the form says.
   - An update never overwrites an input that has focus. Phoenix LiveView
     learned that the hard way.

## Forms

6. **A submitted form arrives as one typed record.**
   - `on:submit={(f: Result<CheckoutForm, DecodeError>) => ..}`: each field
     of `CheckoutForm` must be a named input in the form, and each input a
     field. The compiler checks it, as Ur/Web does.
   - The command that receives it decodes again on the server, because
     browser input is untrusted (charter §7.10), as Swift requires client
     input to be endorsed.

## A command's answer

7. **A handler may read a command's answer**, typed by the command's result
   and decoded in the browser by the same codec the server encoded it with.
   The runtime ignored it until now. A handler that only needs the page to
   change still reads nothing: the resource changes it (ADR-0058).

## Order

These land with the signal work (ADR-0130), after local signals and view
composition: events first (records, typed parameters, modifiers), then
`bind:value`, then forms. T06 needs all three.
