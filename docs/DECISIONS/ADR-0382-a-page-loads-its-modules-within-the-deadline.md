# ADR-0382: the modules a page loads when it first needs them are loaded within the deadline, and a failed load is not kept

Status: accepted by the integrator under the owner's delegation, 2026-10-21.
Milestone: E14. Extends ADR-0381 from a press's command to the modules the
page loads first; keeps E7-L's lazy loading and ADR-0122's speculation.

## Context

- **A page loads what it needs when it first needs it** (E7-L): a press, its
  handler's module (`/handler/<identity>.mjs`) and, where its command
  speculates, the page's speculation module and, for a speculation that
  renders a region, the renderer (`/pw-render.wasm`, ADR-0172); a signal's
  change, the page's computed values (`/computed/<page>.mjs`, ADR-0227).
- **One failed load was kept** (found 2026-10-21, reading `speculations()`
  beside `loadHandler`'s comment, then reproduced in Chromium).
  `speculations()` kept its promise with `??=`, a rejected one too, so every
  later press that speculates failed with the same error until the page was
  reloaded, and the network was not asked again: one dropped request on the
  store's first Add, and no Add worked again. `renderModule()` and
  `computedValues()` kept a failure the same way. `loadHandler` had met this
  for handlers: the browser's module map keeps a failed import, so it asks
  under a new specifier (`?attempt=N`). A probe confirmed it here: in
  Chromium, importing the speculation module again under its first
  specifier, after a failed load, fails again.
- **No module load had a deadline.** A press waits for its turn (ADR-0152)
  only after its handler's module has loaded; one that never loaded held its
  press, and every press made after it. ADR-0381 bounded a command's sends,
  and named this as not yet bounded.
- **`reapplySpeculations()` awaited a load under way** after each batch of
  the server's frames: a load that failed rejected the batch's follow-up,
  and the focus it restores.

## Decision

1. **A module the page loads on demand is loaded within the deadline**:
   ADR-0381's ten seconds, lowered by `?deadline=`. That is a handler's
   module, the speculation module, the renderer with it, and the computed
   values. A load given up fails as a failed load does, and is no longer
   waited for; the import itself cannot be stopped, and what it brings late
   is not used. So a press made after it is not held by a module that never
   comes.
2. **A failed load is not kept.** The speculation module, the renderer and
   the computed values are forgotten when they fail or are given up, and the
   next need asks the network again; the modules under another specifier,
   as a handler's is, so that no failure the browser keeps answers it. A
   computed values load that fails says so (`computed values not loaded`).
3. **A press whose speculation cannot load is sent unspeculated.** A
   speculation shows a press before its answer (ADR-0122); it changes no
   outcome. Its press is sent all the same, and the page shows the server's
   value when its frames come.
4. **A batch reapplies only a speculation module that has loaded.** Before
   it loads, no press has speculated; a load that fails cannot fail a batch.

## Alternatives

- **Fail the press whose speculation cannot load.** The press would be lost
  for what only shows it early, and the reader told "This did not work" for
  a command that was never sent.
- **Retry the import within the press.** A second wait inside one press,
  and a module that fails at once would be asked for in a loop; the next
  press asks again, which a reader's own retry also is.
- **Preload every handler at boot.** E7-L's claim is that a page carries no
  handler's bytes until it is pressed.

## Acceptance

`just e14-press-modules` records `docs/evidence/E14/press-modules.txt`:

- `e2e/press-modules.spec.mjs`, 5 tests on the store, and
  `e2e/computed-modules.spec.mjs`, 2 on a feed page, in Chromium, Firefox
  and WebKit:
  - a press whose speculation module failed to load is sent unspeculated,
    and the next press loads it again and is speculated; the same where the
    renderer failed;
  - a press whose speculation module never loads is sent unspeculated at
    the deadline;
  - a press whose handler's module never loads fails at the deadline, and
    the next press loads it again;
  - a clear pressed after an order whose module never loads runs once that
    load is given up, and no order is placed;
  - on a feed page, computed values that failed to load, or never loaded
    and were given up at the deadline, are loaded again at the next change
    of the draft, and what is left of it is computed.

  The five press tests fail at `bfda9ae5`, and the two computed ones at
  `5fc2fde5`, each at an assertion; all pass here.
- `scripts/press_modules_mutations.py`: nine strict controls, among them a
  failed load asked for again under the same specifier, which the browser
  keeps failed.

## Not claimed

- **A press's other awaits**: a handler's own code, and its command's,
  bounded by ADR-0381. A navigation's page load is the browser's.
- **What a failed load is told**: a press's, "This did not work. Try again."
  (ADR-0302's `failed`), whether its module failed or never came; a
  computed value's, nothing the reader sees. The signal's change is held,
  and its value is computed at the next change.
- **The rest of a change that met a failed computed values load**: parts the
  same flush would have rendered after it are rendered at the next change.
