# ADR-0182: keyboard and screen-reader semantics remain valid

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's ninth gap: charter §15.6 test 14, "Keyboard and
screen-reader semantics remain valid", with §17.4's minimum: semantic
controls, labels, keyboard activation, focus order, live-region behavior,
contrast, reduced motion, and screen-reader smoke test documentation.

## Context

- **No audit had run.** ADR-0168 named each Add by its item and made the
  cart's count a live region, and left the automated audit (its "Not
  claimed").
- **Measured first, by hand.** Lighthouse 13.4.1's accessibility audit, which
  runs axe-core, in Chrome 154 through Chrome DevTools, in snapshot mode,
  scored 100 in each state it read:
  - the store as served;
  - a cart line, and an item sold out;
  - a refusal said, at a phone's width;
  - store 48;
  - the page that is not found.

  Every axe rule that applied passed. The one failure on every page was SEO's
  meta description. It is not a recorded command, so it is context here, not
  evidence.
- **What that audit cannot see.**
  - **A change.** axe reads one state. This page patches itself, and each
    patch can break what the last state had right: a live region replaced is
    one a screen reader no longer listens to; an insert can duplicate an id
    or a name.
  - **What a person does**: the keyboard's order, pressing, focus shown,
    what a live region says, reduced motion, reflow.
  - **Two engines**: Lighthouse runs in Chromium alone.
- **What this ADR's audit found:**
  1. **One Add said "Items in cart: 1" up to four times.** The count is a
     polite, atomic live region (ADR-0168). The runtime wrote a part's text
     whenever it was asked to, whether or not it was the same. The press's
     speculation, the command's answer and the cart's new value each wrote
     "1" again, and a screen reader says each write. Measured in all three
     engines: three or four writes for one press.
  2. **No page said how wide it is.** None had a viewport meta, so a phone
     lays the page out 980 CSS pixels wide and shows it shrunk. Its text is
     too small to read until zoomed, and zoomed, it scrolls sideways.
     - Next.js's App Router adds `width=device-width, initial-scale=1` by
       default, and SvelteKit's template writes it.
     - The repository's own kiokun slice has it.
     - axe's `meta-viewport` rule only checks that a viewport does not stop
       zoom, so a page with none passes it.
  3. **The store's title is "Store", for every store.** WCAG 2.4.2 failure
     F25: a title that does not identify the page. axe checks only that a
     title is there. See decision 6.

## Decision

1. **Test 14 is a browser spec, run in Chromium, Firefox and WebKit**
   (`e2e/accessibility.spec.mjs`). It reads the page by rules
   (`e2e/accessibility-rules.mjs`): axe-core's, or WCAG 2.2's at level AA,
   under the same names, wherever a test can decide them from the document:
   - the document: `html-lang-valid`, `document-title`, `meta-viewport`;
   - landmarks and headings: `landmark-one-main`, `landmark-unique`,
     `page-has-heading-one`, `heading-order`, `empty-heading`;
   - lists: `list`, `listitem`;
   - ids and ARIA: `duplicate-id`, the IDREFs, `aria-valid-attr`,
     `aria-valid-attr-value`, `aria-roles`, `aria-prohibited-attr`;
   - controls: `button-name`, `label-content-name-mismatch` (WCAG 2.5.3),
     `tabindex`, `aria-hidden-focus`, `nested-interactive`;
   - layout: `target-size` (WCAG 2.5.8) and `color-contrast` (WCAG 1.4.3),
     from what the engine laid out and computed;
   - two of this page's own: every section is a region named for a reader to
     go to, and no two buttons share a name (ADR-0168).

   The rules are read **as served, and after each kind of change**:
   - a line added; its quantity changed; an add refused, and said;
   - an item renamed, one new at the head, one moved, one sold out and every
     page told, one removed;
   - the last line removed, and the cart cleared;
   - also store 48, and the page that is not found.
2. **What a person does and hears is tested as they do it:**
   - **Keyboard order.** Every control is reached by Tab in the order it is
     read, and back again with Shift-Tab, and focus is shown where it is.
     Safari uses Option-Tab, as it does for a person.
   - **Pressing.** Each control is pressed with Enter and with Space. When
     the last line is removed, focus goes to the cart's heading.
   - **The tree a screen reader reads**, every node of it, as Playwright
     reads WAI-ARIA and accname in each engine. It is checked for the store
     as served, and for the cart with a line and a refusal.
   - **Live regions.** Each is the node it was when the page was served,
     through every change, and each change is said once.
   - **Reflow and motion.**
     - At 320 CSS pixels the page reflows (WCAG 1.4.10).
     - On a phone it is laid out at the phone's width.
     - With motion reduced, nothing moves: no animation, no transition.
3. **The runtime writes a part only when what it shows changes.**
   - A range that already holds its text is left as it is.
   - An attribute that already holds its value is not set again.

   After the fix, one press is one write, in each engine. A selection in the
   text stays too.
4. **Every page this host serves is laid out at the device's width:**
   `<meta name="viewport" content="width=device-width, initial-scale=1">`.
   - It is in the store's page, the page that is not found, the demos' pages
     and `pw-render`'s shell.
   - Zoom is never limited.
   - Like the doctype, it is part of the shell, not something a page
     decides.
5. **axe-core itself is not installed.** It needs a network install, which
   is the owner's to allow. The rules here are axe's that apply to this
   page, and the spec tests changes and keyboard use, which axe does not.
   Running axe-core as well (`@axe-core/playwright`, MPL-2.0), in three
   engines and in each state, is recommended to the owner.
6. **The title is the page's to declare, not the host's to choose.** The
   host has nothing but "Store" to give, and a title is the page's
   statement of what it is, from its own values. A page cannot declare one
   yet. That is the language's next decision (ADR-0183). Until it lands,
   test 14 does not claim WCAG 2.4.2 for the store.

## Alternatives

- **axe-core alone, through `@axe-core/playwright`.** It reads one state at a
  time, and does not press, tab, or listen to a live region. It covers more
  rules than this spec does, and is recommended in addition (decision 5).
- **Lighthouse as the recorded command.** It runs in Chromium alone, reads
  one state, and needs a network install as well.
- **Every rule as a compiler check.** Charter §8.2 gives the compiler
  duplicate ids and ARIA relationships. Where the source decides a rule,
  that is where it belongs, and those two are recorded as next. This spec
  reads what only the running page has: layout, contrast, focus, and what
  each patch leaves.
- **Live-region writes coalesced in time.** A debounce delays what is said,
  and still says it twice when two writes land in different frames.
  Writing only a change says each change once, when it happens.
- **A live region silenced while a press is speculated.** A screen reader
  would hear nothing of a press until its answer came, which is the delay
  speculation exists to remove.
- **A viewport each page declares.** No page needs any other. One that does
  can declare it when it exists.

## Acceptance

Recorded by `just e14-accessibility` in `docs/evidence/E14/accessibility.txt`:

- `e2e/accessibility.spec.mjs`, in Chromium, Firefox and WebKit.
  - The phone's test runs in Chromium and WebKit; Playwright's Firefox
    emulates no phone.
- `scripts/accessibility_mutations.py`: 14 mutants, each killed by the spec
  in Chromium:
  - the shell: no viewport (twice), no language;
  - the runtime: a text, or an attribute, written again;
  - the markup: a heading a level down, every Add named "Add", the cart named
    by a missing id, the count no live region, a control first in the tab
    order;
  - the stylesheet: a faint description, focus not shown, a page wider than a
    phone, a line that moves in.

Also:
- **The whole browser suite passes**, so the runtime's change breaks nothing
  that reads what a patch wrote.
- **One older mutant is re-anchored** where the attribute's write moved
  (`instance_changes_mutations.py`).
- **`docs/research/screen-reader-smoke-test.md`** gives the procedure for a
  person with VoiceOver, NVDA or TalkBack, and what each step should say.

## Not claimed

- **WCAG 2.4.2 for the store's title** (decision 6). Met by ADR-0183: a page
  states its title, and the store's is its name.
- **axe-core's own run** (decision 5).
- **A screen reader run by a person**: the procedure is written, and no one
  has run it.
- **Target size at AAA** (WCAG 2.5.5, 44 by 44). The store's buttons are the
  browsers' own, about 20 pixels tall, and meet AA by their spacing.
- **Forced colors, text spacing (WCAG 1.4.12) and 200% text alone.**
- **The other pages**: the demos' and the kiokun slice's are not read by this
  spec.
- **A phone's layout in Firefox.**
