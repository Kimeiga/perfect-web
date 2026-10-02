# ADR-0133: a page holds its own UI state (signals, first slice)

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward T06 and T11 and the DoorDash target. Builds step 1 of
ADR-0130's order of work: local signals on a page.

## Context

Pleris had no UI state. A press could change the server's state through a
command and nothing else, so a page could not open a panel, switch a tab or
count a press. ADR-0058 refused "a handler that calls no command", because
such a handler changed nothing. ADR-0130 ruled what UI state is; this builds
its first slice.

## Decision

```pleris
type Panel = Shut | Cart | Help

page PanelPage() {
    signal panel: Panel = Panel.Shut
    signal presses: Int = 0
    view {
        <button on:press={resumable() => presses = presses + 1}>Press</button>
        <p>{presses}</p>
        {#match panel}
          {:Shut} ..
          {:Cart} <aside><button on:press={resumable() => panel = Panel.Shut}>Close</button></aside>
          {:Help} ..
        {/match}
    }
}
```

### The language

1. **`signal name: Type = value`** in a page's body: a binding whose type is
   written, whose handlers change it, and that the browser holds.
   - It lowers to the `let` it is (`Body::signals` says which), so every rule
     about a binding reads it as one.
   - `signal` is a statement keyword only before a name on its line.
2. **Three rules** (`signals.rs`, the new PW53xx range):
   - **PW5300:** a signal is changed only in a handler. The page's body runs
     on the server, and a render changes nothing (charter §7.4).
   - **PW5301:** a signal is read only where the browser reads it again when
     it changes: a template part, or a handler.
   - **PW5302:** a handler changes a signal, not a binding of the body it is
     written in. That is ADR-0130's fourth ruling. A binding the handler
     declares itself stays legal.
3. **A handler reaches a signal through its context, uncaptured.** PW5025
   asks for every other binding a handler reads to be captured.
4. **A handler may change a signal instead of calling a command.**
   ADR-0058's refusal is now "a handler that calls no command and changes no
   signal".

### The build

5. **A handler's module reads and writes signals through its context:**
   `Instr::SignalGet` and `Instr::SignalSet`, emitted as `context.get` and
   `context.set`. The value is in the wire form the browser's modules
   already use, with a case written `{ $case, value }`.
   - The JavaScript decoder and encoder now take a declared sum type,
     `Option` and `Result`. Until now a captured case was refused.
   - A component refuses both instructions: a signal lives in the browser.
6. **The page plan carries the signals and the parts they decide**
   (`PageValues::signals`, `live`):
   - each signal's first value, as that same wire form;
   - each text part reading a signal;
   - each `{#if}` or `{#match}` a signal decides, with every signal read
     anywhere inside it.
7. **A first value is data:** a literal, a case, or a record or list of
   them, encoded from the lowered expression (`js_pure::constant`). A value
   that computes is a build refusal, by name, because the build does not run
   a program.

### The server and the browser

8. **The server renders a page of signals alone** (`/page/<path>`) at each
   signal's first value. The document carries the first values, the live
   parts, and each live block's template. The page reads with no script.
9. **The browser holds the signals and renders exactly what reads them:**
   - a text part, by setting its range;
   - a block, by rendering its template with **the server's renderer built
     for the browser** (`pw-render-wasm`), then binding the handlers it made,
     once each.
   - The two sides agree by being one implementation: the same escaping in
     every context, the same anchors.
   - A press is rendered once however many signals it changes.
   - Nothing is asked of the server but code, and a page of signals alone
     does not subscribe.

## Alternatives

- **A JavaScript port of the renderer for the browser.** Rejected for now. A
  second implementation of context-sensitive escaping is two things that can
  disagree silently, which is why `pw-resume-wasm` exists.
  - The cost of the WebAssembly is real: 286 KB, 90 KB gzipped, built for
    size (`[profile.browser]`). It is loaded with the first press that renders
    a block again.
  - The way to cut it is per-template compiled block renderers, held to
    `pw-render` by the conformance harness that already holds the JavaScript
    query modules to their components.
- **Render every arm on the server and toggle visibility.** Rejected: an arm
  that binds a case's payload has nothing to render until the case exists,
  and a hidden arm's handlers would still be live.
- **Evaluate a computed first value at build time.** Rejected: that is a
  second evaluator of Pleris. A first value that needs computing waits for
  a component the server runs, as ADR-0125 runs a member.

## Acceptance

- **`compiler/pw-core/tests/signals.rs`, 5 tests**, each with a control:
  - PW5300 and PW5301;
  - PW5302;
  - the plan's first values and live parts;
  - a handler module's `context.get` and `context.set`, for an `Int` and a
    case with a payload;
  - a computed first value refused.
- **`runtime/pw-render-wasm`, 3 tests:** escaping as the server escapes, a
  `{#match}` by case, and a malformed request refused.
- **`spikes/own-renderer/e2e/signals.spec.mjs`, 7 tests**, in WebKit and
  Chromium:
  - first values on the server, and readable with JavaScript off;
  - a press changing a text part, with nothing asked of the server;
  - a block rendered again, with its handler bound once each time;
  - a block re-rendered when a second signal it reads changes;
  - escaping of markup in a signal, at top level and inside a block;
  - no subscription.
- The own-renderer suite passes across its browsers, 342 tests with these.
- **Mutation controls:** `scripts/signals_mutations.py`, `just e14-signals`,
  9 mutants. Two earlier controls are re-anchored, and each is still killed:
  `handler_mutations.py`'s "a handler that calls no command is compiled" and
  `handler_capture_mutations.py`'s "a declaration's name is a binding".

## Found while building

- **Every new handler was refused in the browser:** the resume decision knew
  the store's two handlers by name. ADR-0132.
- **A handler that is not `resumable(..)` builds, and its button is
  inert.** `on:press={() => ..}` gets an event part with no handler identity
  and no module, and nothing refuses it. Fixed by ADR-0134.

## Not claimed

- **A view's signals**, a signal `provide`d to views, and two views
  composing: ADR-0130's steps 2 and 3, which need view composition.
- **A signal read inside an `{#each}`** a query decides, an attribute a
  signal decides, and an `{#each}` inside a block a signal decides. An
  instance token needs the server's key, which the browser must not hold.
- **Typed events and `bind:value`** (ADR-0131), and `persist url`.
- **A block re-renders whenever any signal it reads changes**, even when the
  arm shown does not read it.
