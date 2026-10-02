# ADR-0141: a dialog a signal shows is the browser's modal dialog

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14, toward T11's accessible dialog.

## Context

A modal dialog has to do five things to be accessible (WAI-ARIA's dialog
pattern):
- be announced as a dialog, with a name;
- move focus into itself when it opens;
- keep focus and pointer input from the page behind it;
- close on Escape;
- give focus back to what opened it when it closes.

A `<div role="dialog">` does none of them by itself. The HTML `<dialog>`
element, shown with `showModal()`, does all five; the standard specifies
each. But `showModal` is a call, and Pleris has none in a template.

Escape is the hard part. It closes a modal dialog by itself. The standard
lets a page stop that only sometimes: a `cancel` event is cancelable only
while the page has the user's activation, so that a page cannot trap the
user. So a dialog can close without the program having said so. If the
signal that shows it still says it is shown, the page holds a closed dialog,
and the control that opens it sets the signal to the value it already has:
nothing renders, and the dialog cannot be opened again.

## Decision

1. **A `<dialog>` written without `open`, in a block a signal decides, is
   shown with `showModal()` when the browser renders the block**, and when
   the runtime boots on a document the server rendered with it shown. The
   browser then:
   - focuses its `autofocus` element, or its first focusable;
   - makes the page behind it inert;
   - closes it on Escape.
2. **The runtime closes it with `close()` before it removes the block**, so
   focus goes back to what had it.
3. **A modal dialog handles `close`** (PW5303, new): `on:close={() => open =
   false}`. `close` fires however the dialog closed, Escape included, so the
   signal that shows it always hears of it. Its record is the dialog's
   return value, `CloseEvent { value }`, declared in the platform's `events`.
4. **A `<dialog>` that nothing shows is refused** (PW5303): one without
   `open`, outside any block a signal decides. That includes a block a
   parameter decides, which the server decides once. **`<dialog open>`** is
   HTML's dialog, shown in place, and is left alone.
5. **Focus goes back to what invoked the dialog, in every engine.** The
   standard says `close()` restores focus to what was focused before
   `showModal()`. WebKit's build here does not. And WebKit does not focus a
   button a pointer presses, so what was focused there is the body. The
   runtime keeps what had focus when it showed the dialog, or else the
   element whose handler ran last. When the dialog closes and focus has been
   lost, it focuses that. Where an engine restored focus, or a handler moved
   it on purpose, focus is left where it is.
6. **A signal set to the value it holds changes nothing**, and renders
   nothing. A dialog the runtime closes fires `close`, whose handler sets the
   signal its block already shows.

## Alternatives

- **`<div role="dialog" aria-modal="true">` with the focus work done by the
  runtime.** Rejected. Trapping focus, making the page behind it inert and
  restoring focus would each be written by hand. The browser already does
  them for `<dialog>`, as the standard specifies.
- **The runtime prevents the browser's own close, so only the signal closes
  the dialog.** Rejected. The standard makes `cancel` non-cancelable without
  the user's activation, so the runtime could not always prevent it. The
  rule would hold until it did not.
- **A new attribute, `<dialog modal>`.** Rejected. HTML's own vocabulary
  already says it: a `<dialog>` without `open` is not shown in place, and a
  block a signal decides is what shows it.

## Acceptance

- **`compiler/pw-core/tests/dialogs.rs`, 4 tests**, each with controls:
  - a dialog a signal shows says what closing it does, at top level and
    inside an arm;
  - a dialog nothing shows is refused, a block a parameter decides
    included, and `<dialog open>` is left alone;
  - `close` gives its handler the return value, and its record's fields are
    checked;
  - a view with a dialog alone is read.
- **`spikes/own-renderer/e2e/dialog.spec.mjs`, 5 tests**, in Chromium,
  Firefox and WebKit, over `examples/demo/dialog.pw`:
  - the dialog is modal, named, and focused where `autofocus` says;
  - the page behind it is inert to the pointer and to Tab;
  - Escape closes it, the signal hears, focus goes back, and it opens again;
  - a button in it closes it the same way;
  - its action runs, and it closes.
- The platform contract's hash moves, for `close`.
- **Mutation controls:** `scripts/dialogs_mutations.py`, `just e14-dialogs`,
  5 mutants.

## Not claimed

- **A dialog's return value from a form**, `<form method="dialog">`: forms
  wait for ADR-0131's rulings.
- **A dialog in a view**, other than one the page shows: a view's signals
  wait for ADR-0130's step 3.
