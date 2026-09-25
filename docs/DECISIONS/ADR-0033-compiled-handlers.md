# ADR-0033: compiled resumable handlers — JavaScript modules, carried captures, typed arguments

Status: accepted under the owner's instruction of 2026-09-24 ("finish the rest
of E10"); implemented with the evidence linked below. Decisions marked
**(ruling needed)** were made without an architect ruling and are offered for
reversal.
Date: 2026-09-25. Milestone: E10 gate item "the store builds from source to
browser artifacts … without Koka or Marko", and charter §14 M10 task 2, whose
first item is "generated modern JavaScript modules for handlers".

## Context

After E10-I the store's commands were compiled components, but the page's
behaviour was not compiled. For
`on:press={resumable(captures = { item }) => add_to_cart(item.id, PositiveInt(1))}`
the development server:

- answered `/handler/<identity>.mjs` with a module it wrote itself;
- received the pressed element's instance address and the literal quantity;
- resolved the address back into an item with the derivation that rendered it
  (`menu_item_at`).

ADR-0032 §8 recorded this as a ruling-needed stopgap. The handler's body, the
call and the arguments it computes, existed only in the `.pw` source.

## Decision

### 1. A handler's body compiles to an ES module (`backend/js.rs`)

`pw emit-handlers --out DIR` writes `DIR/<identity>.mjs` for every resumable
handler in a checked program. The store's `add_to_cart` handler becomes:

```js
export async function run(context) {
  return await context.command("store.page.add_to_cart", [context.captures["item"]["id"], 1]);
}
```

The supported set is one call to a command, whose arguments are:

- captured values and their fields;
- literals;
- opaque constructors over a primitive, which send their representation.

Every parameter of the command must be a primitive or an opaque type over one.
Anything else is refused by name, and so are a handler with parameters, a body
that is not one call, and a call to a query or a function.

`emit-handlers` is all or nothing: one refusal writes no file and fails the
build. A partly compiled page would render buttons that cannot work.

### 2. Every fact the backend needs is read from the stage that owns it

| fact | owner | before this ADR |
|---|---|---|
| which handler | `resume_artifacts::located` | (unchanged) |
| its name | `template_ir::called_name` | (unchanged) |
| which declaration a call names | `values::named`, extracted from the typer | private to the typer |
| a command's component id | `contract::component_id` | formatted in three places |
| what the handler reads of its captures | `resume::capture_paths` | did not exist |

The component id had three derivations. Two were inline in `contract.rs`. A
third was in `backend/lower.rs`, and a map in `wit.rs` was keyed by the same
format. All now call `contract::component_id`. The handler backend is its
fourth reader, and a second answer would send a press to a component no host
has a contract for.

### 3. Captures are carried as the paths the handler reads (ruling needed)

`template_ir::Part::Event` gains `captures`: the field paths the handler body
reads (`["item.id"]`). The renderer serializes those values onto the element:

```html
<button data-pw="0" data-pw-captures="{&quot;item&quot;:{&quot;id&quot;:&quot;espresso&quot;}}">
```

It serializes nothing else from the item, and a handler that reads nothing
carries no attribute. The compiled handler reads `context.captures["item"]["id"]`.
One derivation feeds both, so the document cannot carry a different set than
the module reads.

This is charter §8.5: "SSR should emit semantic HTML plus only the metadata
needed to resume interactions", and a handler may capture "serializable
immutable values; resource keys". Narrowing to the paths the handler reads is
what "only" means here. It also keeps the common case from going stale: the
store reads `item.id`, the key of its keyed loop, which a keyed patch never
changes.

**Ruling needed:** the 2026-08 architect ruling "Never put raw domain keys into
identity markup merely for renderer convenience" is read as not covering
`data-pw-captures`. The attribute is not identity markup, and a key is there
because the author declared the capture. `store.spec.mjs`'s identity-markup
check now excludes that one attribute by name. The alternative is
capture-by-reference: the browser sends the address and the server resolves
it, which is what ADR-0032 §8 did by hand. That cannot be compiled without the
server re-deriving the page's render environment per press.

### 4. What the browser sends is not trusted

The module runs in the user's browser, so its arguments are a claim.
`POST /command/<component id>` takes a JSON array. `pw_host::engine::Prepared::arguments`
converts it by the parameter types the ARTIFACT declares, read through Wasmtime
47's type reflection. It refuses, before anything runs:

- the wrong count or the wrong kind;
- an integer outside its type's range;
- a number with a fraction where an integer is declared;
- every non-scalar parameter type, by name.

A refusal is HTTP 400. A command that ran and did not commit is 202
`{"committed":false}`, as before. The browser runtime throws on a non-2xx
response, so a refused press is visible on the element (`data-pw-handler-error`)
rather than silent.

This checks the ABI's types, not what an opaque type's name promises.
`PositiveInt` is `opaque type PositiveInt = Int` and carries no invariant the
host could check (KNOWN_LIMITATIONS).

### 5. One command path in the development server

`Server::command(component_id, session, args)` runs any hosted command. It does
not know which command it runs. The deployment's data layer implements all of
`store:data/carts` (`add`, `clear`, `current`), and the linker gives a
component only what it was granted and imports. The staged cart and its event
are committed together when the component returns without an `Err`.

Deleted: `add_to_cart`, `clear_cart`, `menu_item_at`, `percent_decode`, the
`?item=`/`?instance=` route and the server-written handler modules. A
structural test keeps them deleted.

**Found on the way:** `clear_cart` committed its event but never its state, so
the materializer's `cart:<session>` kept the old total after a clear. Nothing
read it, so nothing failed. The one path commits both, and a test holds it.

### 6. A string literal has a value only where no escape rule is needed (ruling needed)

`Literal::Str` holds the source token: its quotes, and escapes as written. The
language does not say what an escape means. The lexer knows only that a
backslash keeps the next character in the string. The Koka and Marko backends
pass the token to targets with different escape rules.

`Literal::string_value` gives a value only for `"..."` with no backslash. The
handler backend and the Wasm lowering both use it, and both refuse any other
string.

**Found on the way:** the Wasm lowering put the token, quotes included, into
`Const::Str`. No artifact carried it, because the encoder refuses constants
that need memory.

**Ruling needed:** the escape set. Until it is decided, a backend refuses rather
than choosing its target's rules.

### 7. Integers a JavaScript number cannot hold are refused on both sides

Beyond ±2^53 a JavaScript number is not the integer it was written as. The
handler backend refuses such a literal, and the renderer refuses to carry such
a captured value. The host accepts an `s64`'s whole range, because it parses
the JSON text exactly and the refusal belongs where the rounding would happen.

### 8. The build and the server agree on which handlers exist

`run.sh` emits the template IR and the compiled handlers from one unit set: the
store with the standard packages, the set `pw check` accepts. The identities
were already equal for the store without them, but that was a coincidence of
this program. The development server refuses to start if any handler identity
in its templates has no compiled module. It serves `/handler/<identity>.mjs`
from `dist/handlers/` only for identities its templates name.

## Acceptance

- `compiler/pw-core/tests/handlers.rs` (12 tests):
  - the store's modules, exactly;
  - each module agrees with its event part on identity, name and paths;
  - every command a handler calls has a contract;
  - an unchecked program compiles nothing;
  - the refusal matrix.
- `runtime/pw-render/tests/properties.rs`: the paths carried, nested and
  escaped; a missing path, an inexact integer and raw HTML block the render.
- `runtime/pw-host/tests/pleris_component.rs`: JSON typed by the committed
  artifact's own parameters, and each refusal.
- The dev server's unit tests:
  - arguments typed by the component;
  - state and event committed together;
  - uncompiled handlers refuse startup;
  - the structural deletion test.
- `e2e/compiled-handler.spec.mjs`:
  - the loaded module is the compiled body;
  - each button carries only `item.id`;
  - a press sends that button's own item;
  - malformed arguments get 400 and move nothing;
  - the old route is gone.
- The three-engine browser suite, recorded.
- Evidence: [handlers-2026-09-25.md](../evidence/E10/handlers-2026-09-25.md).

## Consequences

The store's behaviour is compiled from `.pw`: the page's handlers, and the
commands they reach. ADR-0032 §8 is superseded. What is not claimed:

- **Captures are not a patched part.** A captured field that a patch changes
  without re-rendering its element (`ReplaceText` on a keyed item's name) stays
  as rendered. The store reads only the key.
- **Handler parameters, local computation, branches and multi-statement
  bodies** are refused.
- **Opaque invariants are not checked at the boundary.** The language does not
  state them yet.
- **The data layer is still the deployment's** (NEXT step 10).
