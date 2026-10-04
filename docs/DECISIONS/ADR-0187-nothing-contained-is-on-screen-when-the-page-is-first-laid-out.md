# ADR-0187: nothing the store contains is on screen when its page is first laid out

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. Found by measuring the store's page with Lighthouse after
ADR-0186. E7's gate item 10 (a thousand-item menu at a bounded rendering
cost) is kept.

## Context

- **What Lighthouse found.** Lighthouse 13.4.1 in navigation mode on a phone
  (412 × 823): a cumulative layout shift of 0.136, above the 0.1 that Core
  Web Vitals call good. One shift, as the page loaded: the cart's section
  moved 249 px down, and the recommendations' with it. Research context: no
  recorded command produced it.
- **What moved it.** The host's style for the store:
  `#menu li { content-visibility: auto; contain-intrinsic-size: auto 42px; }`.
  - **Every item was contained**, so the browser first laid each out at its
    42 px placeholder. It then rendered the three on screen, which are
    125 px tall, and what followed them moved down by 3 × 83 px.
  - **The placeholder was stale.** 42 px was an item's height when E7 set
    it. Items have had a description since, and are 125 px at 16 px text,
    on a phone and on a desktop alike.
- **The shift is never painted, and it is counted.** On a minimal page in
  Chromium 154:
  - **With the containment**, a shift of the section after the list is
    reported, wherever the style is.
  - **Without it, none is.**
  - **With 30 items, none is.** The section is then off screen, and a
    contained element's own movement is not reported.
  - **A probe in the first frame** read the section at its final place, at
    the resize-observer step (9 ms), before the first paint (28 ms).
  - **That is what HTML asks.** Its "update the rendering" steps lay a
    document out, determine which contained elements are near the viewport,
    and lay it out again if one was found on screen for the first time, so
    that "the initial viewport proximity determination, which takes effect
    immediately", is in the layout that is painted.
  - **Chrome says so** (its metrics changelog, Chrome 88): a contained
    element's own resizing no longer counts, "however, there still may be a
    layout shift for onscreen elements adjacent to (but not descendants of)
    the `content-visibility: auto` element." The Layout Instability API that
    reports it is the one Chrome's field data (CrUX) reads, so the page
    would be charged for a shift no one saw.
- **The style was in the body.** A `<style>`'s parent is an element that
  accepts metadata content, which a body does not. And "only `style`
  elements in the document's `<head>` can possibly block rendering" (MDN).
  So one in the body lets a first layout run without the containment,
  paying for every item, whenever the body arrives before it.
- **Containment pays only off screen.** The browser renders a contained
  element that is on screen in the first frame anyway, so containing it
  saves nothing and costs the report above.

## Decision

1. **Nothing the store contains is on screen when its page is first laid
   out.** An item is contained when at least 28 items precede it in its
   list, or at least 16 lists precede its list:

   ```css
   #menu > ul > li:nth-child(n+29),
   #menu > ul:nth-of-type(n+17) > li {
     content-visibility: auto;
     contain-intrinsic-size: auto 7.75em;
   }
   ```

   - **Why this holds.** The smallest item, with no description and "Sold
     out" for its button, is 86 px tall at 16 px text in Chromium and
     WebKit, and 92 px in Firefox. A category with one such item is about
     150 px. So a contained item is at least 2,400 CSS pixels down the page,
     in a single column. That is below the first screen of a phone, a laptop,
     and a desktop up to a 4K display's height at 100%. What follows a
     contained item is below it, so nothing on screen moves when the browser
     renders it.
   - **Measured**, in Chromium at 412 × 823, 1350 × 940, 1920 × 2160 and
     1920 × 2400: no shift, and no contained item on screen, for three
     items, a thousand, forty categories of three, a thousand of the
     smallest, and sixteen one-item categories of the smallest followed by
     fifty. The nearest contained item was 2,627 px down, in the last.
   - **The bound holds whatever the menu.** At most 16 × 28 = 448 items are
     laid out uncontained, however many items or categories the menu has.
     The thousand-item case contains 972.
   - **Items alone are contained, each with its own placeholder.** A list
     or a heading is not, because its height depends on how many items it
     has, which a placeholder cannot know. So the scrollbar is as long as
     the items' placeholders say.
2. **The placeholder is an item's height: 7.75em**, 124 px at 16 px text.
   Items measure 123 px in Chromium, 130 px in Firefox and 121 px in WebKit.
   It scales with the text, as an item does.
   - **Measured on a thousand-item page**, against its height with every
     item rendered: 0.8% over in Chromium, 4.2% under in Firefox and 2.4%
     over in WebKit.
   - **With 42 px it was 64% under**: the page grew from 44,140 px to
     123,196 px in Chromium as it was scrolled through, and the scrollbar
     with it.
3. **The style is in the head**, where HTML puts it, so the first layout
   has it.

## Found on the way

**E7's gate item 7b had failed since 2026-10-03 or 2026-10-04, unseen.** Its
test bounds the bytes the store's page downloads to run at 128 KiB. It runs
only in `just e7-performance`, which had last run on 2026-08-07, and
running it for gate item 10 found 141,339 bytes. The runtime's script grew
past the bound between ADR-0152 and ADR-0172. That is ADR-0188's.

## Alternatives

- **An exact placeholder.** An item's height is its text's: a description
  of two lines, or "Sold out" for a button. No one length fits every item,
  and a short miss on screen is still a reported shift.
- **Contain what follows the menu too**, the cart and the
  recommendations, so that their movement is not reported. That hides the
  report, not its cause. It also contains a live region and the cart's
  controls, for nothing.
- **Fixed-height items, the description clamped to two lines**, as many
  delivery sites do. Then a placeholder is exact. But a person who raises
  the text spacing loses the clamped text (WCAG 1.4.12), and this store has
  no item page to read it on.
- **Contain whole categories.** A category's height is its items' count
  times an item's height, which a placeholder cannot know, so the scrollbar
  would jump as each was rendered: the defect E7 named.
- **Count items across categories**, so that exactly the first 28 are left
  alone. CSS cannot count across parents, and a template's loop has no
  index. The two thresholds bound both shapes, a long category and many
  short ones.
- **Contain only after the page has loaded.** The first layout would pay
  for every item, which is the cost E7's gate bounds.
- **Virtualization.** It removes items from the document, which E7 refused
  for a menu (charter §14 M7).
- **No containment.** A thousand items cost their full first layout, which
  fails E7's gate item 10.

## Acceptance

Recorded by `just e14-stable-layout` in `docs/evidence/E14/stable-layout.txt`:

- **`e2e/stable-layout.spec.mjs`**:
  - **Chromium**: no layout shift is reported as the store's page loads,
    at a phone's size and a desktop's;
  - **three engines**: no item contained on screen when the page is first
    laid out, for the store, a menu of one category of a thousand items,
    and one of forty categories of three, each built from the store's own
    item markup and style;
  - **three engines**: the page as first laid out is within 10% of its
    height with every item rendered;
  - **the style is in the head**.
- **`e2e/performance.spec.mjs`, gate 10**, alone in Chromium: the first item
  is not contained and the last is; 972 of a thousand are. The first layout
  of a thousand items took 15.2 ms uncontained, 1.8 ms all contained, and
  2.3 ms as the page is served.
- **`scripts/stable_layout_mutations.py`**, 6 mutants: the first items of a
  list contained, the first lists contained, the lists' threshold dropped, no
  item contained, the 42 px placeholder, and the style back in the body.

## Not claimed

- **A layout other than a single column.** A grid of cards puts the 25th
  item higher, and the thresholds would have to count rows.
- **A first screen taller than 2,400 CSS pixels**, such as a 4K display
  turned upright at 100%.
- **Categories with no items.** Sixteen empty categories are about 1,300 px,
  not 2,400.
- **Text smaller than 16 px.** The thresholds count items, whose height is
  their text's.
- **Field data.** The shift was measured in a lab, by Lighthouse and the
  Layout Instability API.
