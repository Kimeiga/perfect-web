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

1. **A module a press needs is loaded within a deadline**: ten seconds, as
   a send's (ADR-0381), lowered by `?load-deadline=` (never below half a
   second). That is a handler's module, the speculation module, and the
   renderer it loads with it. A load given up fails as a failed load does,
   and is no longer waited for; the import itself cannot be stopped, and
   what it brings late is not used. So a press made after it is not held by
   a module that never comes. The computed values have no deadline: a press
   does not wait for them, and a page's boot does, where a value the device
   keeps is migrated by them (track `device-store`), and a slow load is
   better waited for than failed.
2. **A failed load is not kept.** The speculation module, the renderer and
   the computed values are forgotten when they fail or are given up, and the
   next need asks the network again; the modules under another specifier,
   as a handler's is, so that no failure the browser keeps answers it. A
   renderer still loading when a press gives up on it is forgotten too, so
   the next press does not wait on the same load. A computed values load
   that fails says so (`computed values not loaded`).
3. **A press whose speculation cannot load is sent unspeculated.** A
   speculation shows a press before its answer (ADR-0122); it changes no
   outcome. Its press is sent all the same, and the page shows the server's
   value when its frames come. Only the load is caught: what fails after the
   module has loaded fails the press, as before.
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

- `e2e/press-modules.spec.mjs`, 7 tests on the store and the panel demo,
  and `e2e/computed-modules.spec.mjs`, 1 on a feed page, in Chromium,
  Firefox and WebKit:
  - a press whose speculation module, or renderer, failed to load or never
    loaded is sent unspeculated, and the next press loads it again and is
    speculated;
  - a press whose handler's module never loads fails at the deadline, and
    the next press loads it again;
  - a clear pressed after an order whose module never loads runs once that
    load is given up, and no order is placed;
  - a renderer that failed to load is loaded again for the next block a
    signal renders (`demo.panel`), off any press's path;
  - on a feed page, computed values that failed to load are loaded again at
    the next change of the draft, and what is left of it is computed.

  The press tests fail at `bfda9ae5` and the computed one at `5fc2fde5`; the
  renderer that never loaded, added after the review below, fails at the
  runtime before its repairs; each at an assertion. All pass here. The
  signal block's renderer test was added when its control survived: the
  press path's own forgetting had hidden that `renderModule()` forgets a
  failed load for every other caller.
- `scripts/press_modules_mutations.py`: ten strict controls, among them a
  failed load asked for again under the same specifier, which the browser
  keeps failed, and a renderer load a press gave up on kept.

**Reviewed 2026-10-21** by an independent read of the code. Repaired: a
renderer that never loaded stayed the one every later press waited on; the
computed values had a deadline that would have failed a slow boot, and with
it a device value's migration; the catch that sends a press unspeculated
also took a failure after the module had loaded; and loads and sends shared
one test knob, so a test's short send deadline bounded cold module loads.

## Not claimed

- **A press's other awaits**: a handler's own code, and its command's,
  bounded by ADR-0381. A navigation's page load is the browser's.
- **Computed values that never load** hold what is computed from them, and
  the parts after them in that change, until they come; a press is not held.
- **Decision 4 has no test of its own**: a batch that arrives while a load
  fails is not staged by any test here.
- **A handler's module is loaded again after any failed press**, a command's
  failure among them (as before): one stall can cost a press two loads.
- **What a failed load is told**: a press's, "This did not work. Try again."
  (ADR-0302's `failed`), whether its module failed or never came; a
  computed value's, nothing the reader sees. The signal's change is held,
  and its value is computed at the next change.
- **The rest of a change that met a failed computed values load**: parts the
  same flush would have rendered after it are rendered at the next change.
