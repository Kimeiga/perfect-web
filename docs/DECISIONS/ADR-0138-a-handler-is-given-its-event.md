# ADR-0138: a handler is given its event

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward T06 and T11. Builds the first slice of ADR-0131: an
event's record, a handler's typed parameter, and modifiers.

## Context

ADR-0131 ruled how a handler reads its event:
- an event is a closed record of plain data;
- the runtime reads it in the listener, synchronously;
- what must happen before any code loads is declared as a modifier,
  `on:submit|prevent`.

None of it was built. Building it found seven defects, each of which checked
or built clean:

1. **Every handler listened for a click.** The runtime bound every event part
   to `click`, and the parts manifest did not say which event a part handles.
   `on:input` ran its handler when the input was clicked, and never when it
   was typed in. KNOWN_LIMITATIONS recorded it, and `pw build` accepted it.
2. **A lambda's written parameter type was dropped by the HIR**, in every
   lambda: `fn(y: Nada) 1` checked clean, and `fn(y: Int) y.value` read a
   field an `Int` does not have.
3. **`(e: InputEvent) => ..` did not parse**: an unclosed parenthesis.
4. **A handler with a parameter was refused at build**, so no handler could
   read what was typed.
5. **`on:submit|prevent={..}` parsed as three things**: the attribute
   `on:submit`, a stray `|`, and an attribute `prevent`.
6. **Two handlers on one element wrote two `data-pw-captures` attributes.** A
   browser keeps the first, so the second handler would have read the first's
   captures.
7. **The runtime bound one listener per element.** A second handler on the
   same element, `on:keydown` beside `on:input`, was never bound.

## Decision

1. **A handler takes nothing, or its event:** `on:input={(e: InputEvent) =>
   q = e.value}`, or `(e) => q = e.value`, where `e` is typed as the event's
   record.
   - The record is the parameter type of the platform's `events.<event>`
     (`packages/pw-platform-web/events.pw`).
   - A written type that is not the event's is PW0602, and so is a handler
     taking more than one parameter. A field the record does not have is
     PW0610.
   - This refines ADR-0131's fourth ruling, which asked for the type to be
     written. It may be, and is checked when it is. Where it is not, the
     event gives it, as a call gives a closure's parameters their types.
2. **A lambda's written parameter type is kept and checked, in every
   lambda.** It is kept in a side table beside the pattern
   (`Body::param_types`). Inference and the value relations type the
   parameter by it, and a type that names nothing is PW0026.
3. **`(e: InputEvent, n: Int) => ..` parses** as the list `fn(e:
   InputEvent, n: Int) ..` writes. Nothing else in an expression begins `(
   name :`.
4. **The runtime reads the event's record in the listener**, before the
   handler's module has loaded:

   | event | record |
   |---|---|
   | press | `{ x, y }`, the pointer's position |
   | input, change | `{ value }`, the element's value |
   | keydown | `{ key }` |
   | submit | `{ prevented }` |

   The module decodes it as it decodes a capture. `SubmitEvent`'s `prevented`
   is a `Bool` now: whether the form's own submission was stopped. It was a
   `String` nothing filled or read.
5. **Each part listens for its own event.** The parts manifest carries
   `event` and `modifiers`. The runtime maps a semantic event to the
   browser's: `press` to `click`, the others to themselves.
6. **Modifiers are `prevent` and `stop`**, applied in the listener before any
   code loads. Any other is PW5027. A press no longer prevents the browser's
   action by default. What a press does is the element's, unless `|prevent`
   says otherwise. The store's buttons are `type="button"`, which have no
   action to prevent.
7. **An element's handlers carry one `data-pw-captures` attribute.** They are
   lowered after its other attributes and together. The renderer writes their
   captures as one object, since in one element's scope a name is one value,
   and refuses one name read from two places.
8. **The runtime binds each handler of an element**, once each.

## Alternatives

- **Give the handler the DOM event**, as Qwik and Marko do. Rejected, as
  ADR-0131 rejected it. The DOM event is spent by the time a lazily loaded
  module runs, and it is not data a recorded interaction could replay.
- **Require the written type.** Rejected, as above. The attribute already
  says which event, and an agent writes `(e) =>` first, as it would in
  TypeScript or Svelte. Both forms are checked.
- **Keep a press preventing the browser's action.** Rejected. A press on a
  link would then never navigate, unless the author found a way to say
  otherwise. A modifier says it, and says it where the element is.
- **Write each handler's captures under its part's number.** Rejected. A
  handler's module reads its captures by its own names, and in one scope a
  name is one value. The union is what both handlers read, and one name read
  from two places cannot happen without a defect, which is refused.

## Acceptance

- **`compiler/pw-core/tests/events.rs`, 6 tests**, each with controls:
  - a handler is given its event's record, typed or inferred, run under Node;
  - a handler for another event, or taking two parameters, is refused, and
    so is a field the record does not have;
  - a modifier is one the runtime applies;
  - the manifest says each part's event and modifiers;
  - a lambda's written parameter type is checked, in any lambda;
  - an element's handlers are lowered last and together.
- **`pw-syntax`'s grammar, 2 tests:** a modifier is part of its attribute,
  and an arrow lambda's parameters may be typed, with a control.
- **`runtime/pw-render/tests/properties.rs`:** an element's handlers carry
  one attribute between them, and one name read from two places is refused.
- **`spikes/own-renderer/e2e/events.spec.mjs`, 5 tests**, in Chromium,
  Firefox and WebKit, over `examples/demo/events.pw`:
  - an input's handler runs on input, not on a click;
  - what was typed reaches the handler as text;
  - a key's handler and an input's are both bound on one element;
  - a form's submission is stopped before the handler's code loads;
  - the manifest says each part's event and modifiers.
- The platform contract's hash moves, for `SubmitEvent`
  (`tests/platform_contracts.rs`).
- **Mutation controls:** `scripts/events_mutations.py`, `just e14-events`,
  12 mutants.

## Not claimed

- **An element's refinement of its event:** a checkbox's `change` giving
  `checked: Bool` (ADR-0131, ruling 1).
- **`bind:value`**, forms as one typed record, and a command's answer read
  by a handler (ADR-0131, rulings 5-7).
- **Events the platform does not declare**, `focus` and `blur` among them.
  The Marko adapter maps both, and `events.pw` declares neither.
- **A modifier in the Marko adapter**, which refuses one by name.
