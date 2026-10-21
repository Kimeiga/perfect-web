# ADR-0382: the modules a press needs are loaded within the deadline, and a failed load is not kept

Status: accepted by the integrator under the owner's delegation, 2026-10-21.
Milestone: E14. Extends ADR-0381 from a press's command to the modules it
loads first; keeps E7-L's lazy loading and ADR-0122's speculation.

## Context

- **A press loads what it needs when it is first made** (E7-L): its
  handler's module (`/handler/<identity>.mjs`), and, where its command
  speculates, the page's speculation module and, for a speculation that
  renders a region, the renderer (`/pw-render.wasm`, ADR-0172).
- **One failed load of the speculation module was kept** (found 2026-10-21,
  reading `speculations()` beside `loadHandler`'s comment, then reproduced in
  Chromium). `speculations()` kept its promise with `??=`, a rejected one
  too, so every later press that speculates failed with the same error until
  the page was reloaded, and the network was not asked again: one dropped
  request on the store's first Add, and no Add worked again. `renderModule()`
  kept a failed renderer the same way. `loadHandler` had met this for
  handlers: the browser's module map keeps a failed import, so it asks under
  a new specifier (`?attempt=N`). A probe confirmed it here: in Chromium,
  importing the speculation module again under its first specifier, after a
  failed load, fails again.
- **No module load had a deadline.** A press waits for its turn
  (ADR-0152) only after its handler's module has loaded; one that never
  loaded held its press, and every press made after it. ADR-0381 bounded a
  command's sends, and named this as not yet bounded.
- **`reapplySpeculations()` awaited a load under way** after each batch of
  the server's frames: a load that failed rejected the batch's follow-up,
  and the focus it restores.

## Decision

1. **A module a press needs is loaded within the deadline**: ADR-0381's ten
   seconds, lowered by `?deadline=`. A load given up fails as a failed load
   does, and is no longer waited for; the import itself cannot be stopped,
   and what it brings late is not used. So a press made after it is not held
   by a module that never comes.
2. **A failed load is not kept.** The speculation module and the renderer
   are forgotten when they fail or are given up, and the next press asks the
   network again; the speculation module under another specifier, as a
   handler's is, so that no failure the browser keeps answers it.
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

- `e2e/press-modules.spec.mjs`, 5 tests, in Chromium, Firefox and WebKit:
  - a press whose speculation module failed to load is sent unspeculated,
    and the next press loads it again and is speculated; the same where the
    renderer failed;
  - a press whose speculation module never loads is sent unspeculated at
    the deadline;
  - a press whose handler's module never loads fails at the deadline, and
    the next press loads it again;
  - a clear pressed after an order whose module never loads runs once that
    load is given up, and no order is placed.

  All five fail at `bfda9ae5`, each at an assertion, and pass here.
- `scripts/press_modules_mutations.py`: six strict controls, among them a
  failed load asked for again under the same specifier, which the browser
  keeps failed.

## Not claimed

- **The computed values' module** (`computedValues()`, which a feed page
  loads when a signal's change first needs it) still keeps a failed load,
  and has no deadline; no test here reaches it. Not repaired here.
- **A press's other awaits**: a handler's own code, and its command's,
  bounded by ADR-0381. A navigation's page load is the browser's.
- **Which words a failed load is told in**: "This did not work. Try again."
  (ADR-0302's `failed`), whether the module failed or never came.
