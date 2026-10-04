# Screen-reader smoke test: the store's page

Charter §17.4 asks for "screen-reader smoke test documentation" beside the
automated checks, because automated checks cannot prove a page accessible.
This is the procedure, and what each step should say. ADR-0182's spec,
`spikes/own-renderer/e2e/accessibility.spec.mjs`, checks the tree each step
reads from, in three engines; this is what a person hears from it.

**Status: written, not yet performed by a person.** No screen reader has run
it. A run is recorded at the end, one row per screen reader and browser.

## Setup

1. Build and serve the store, as the browser suite does:
   `BUILD_ONLY=1 bash spikes/own-renderer/run.sh` and
   `cargo build -p pw-dev-server`, then, in `spikes/own-renderer`,
   `PORT=3141 ../../target/debug/pw-dev-server dist`.
2. Open `http://127.0.0.1:3141/stores/47` in a fresh private window, so the
   cart is empty.
3. Screen readers, one run each:
   - VoiceOver on macOS, with Safari: Command-F5. Safari moves focus to a
     button with Option-Tab, or with Tab once "Press Tab to highlight each
     item on a webpage" is set in Safari's Advanced settings.
   - NVDA on Windows, with Firefox and with Chrome.
   - TalkBack on Android, with Chrome, for the phone's layout.

## Steps, and what each should say

| # | Do | Should hear |
|---|---|---|
| 1 | Load the page. | The page's title, "Store". (ADR-0183 is to make it the store's name: WCAG 2.4.2.) |
| 2 | List the landmarks (VoiceOver rotor; NVDA D). | Main; regions Delivery, Menu, Cart, Recommendations. |
| 3 | List the headings (VoiceOver rotor; NVDA H). | "Blue Bottle", level 1; "Coffee", level 2; "Cart", level 2. |
| 4 | Wait for the estimate. | "Delivery in 25 to 35 min", once, without moving. "To", not a dash. |
| 5 | List the buttons. | Add Espresso, Add Cortado, Add Cold Brew, Clear. No two the same. |
| 6 | Move through the Menu's list. | For each item: its name, its description, its price, "Add *name*, button". |
| 7 | Press Add Espresso. | "Items in cart: 1", once. Focus stays on Add Espresso. |
| 8 | Tab to "Increase quantity of Espresso" and press it. | "Items in cart: 2", once. Focus stays on the button. |
| 9 | Sell Cold Brew out without telling the page: `curl -X POST 'http://127.0.0.1:3141/bench/stock?item=cold-brew&available=false'`. Press Add Cold Brew. | "Items in cart: 3", then "Items in cart: 2" and "That item just sold out." |
| 10 | From another window, rename an item: `curl -X POST 'http://127.0.0.1:3141/command/menu?op=rename&id=cortado&name=Cortado%20Doppio'`. | Nothing. Focus stays where it is. The button reads "Add Cortado Doppio" when next reached. |
| 11 | Press "Remove Espresso" on the last line. | Focus moves to "Cart, heading level 2". "Items in cart: 0". |
| 12 | Zoom to 400% (or open on a phone). | Nothing to scroll sideways; the text wraps. |

Afterwards, put the server back:
`curl -X POST 'http://127.0.0.1:3141/bench/stock?item=cold-brew&available=true'`
and rename Cortado back (`name=Cortado`).

## What a failure looks like

- A step's words said twice. That was found once: an Add said "Items in
  cart: 1" up to four times, until ADR-0182.
- A change said nowhere: the region was replaced, not changed.
- Focus lost to the top of the page after a press or a change.
- A button heard as "button" with no name, or a name that is not what it shows.

## Runs

| Date | Screen reader | Browser | OS | Result | Notes |
|---|---|---|---|---|---|
| — | — | — | — | not yet run | — |
