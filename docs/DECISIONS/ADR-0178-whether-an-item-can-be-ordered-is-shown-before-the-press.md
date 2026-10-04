# ADR-0178: whether an item can be ordered is shown before the press, and a change to it reaches every page open

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the first part of the audit's eighth gap:
- charter §15.1's `MenuItem.available: Bool`;
- §15.2's item availability, "Public but short-lived", placed "edge/shared,
  event invalidated".

## Context

- **A sold-out item looked orderable until it was pressed.** Since ADR-0157
  the command reads the item's availability before it commits, and refuses
  a sold-out item by name. The page did not show availability. Every row
  had an Add, so a reader learned an item was sold out only by pressing it.
- **`MenuItem` had no `available`**, though §15.1 declares one.
- **`InventoryChanged` was declared, and nothing emitted it.** The store's
  menu fragment listened for it; its `Menu` query did not.
- **A row was rendered again for any change inside a block** (ADR-0168).
  With the Add inside an availability block, a rename would have lost the
  button's node, and focus with it (test 15).
- **A new version of the shared menu fragment did not always reach the
  pages that showed it.** Found while writing this ruling, by reading the
  code.
  - ADR-0150 rendered the fragment again for a new document whose `Menu`
    read differed from it, and told no open page, since no event had
    announced the change.
  - But the fragment is one version for every reader, and every later
    change was derived from the new version, which the open pages did not
    have.
  - Nothing changed the menu's value without an event until availability
    did. §15.5's forced stale item (ADR-0157) is exactly such a change.
  - Two consequences follow, and this ruling's tests fail the code as it
    was (its mutation controls restore it):
    - after an untold sale and a fresh read, a page already open kept the
      item's Add. A later told change of the same item found nothing to
      send.
    - a rename sent its own row's parts alone, so a sold-out item drawn
      into the fragment with it kept its Add on the pages open.

## Decision

1. **`MenuItem` declares `available: Bool`** (charter §15.1). The menus' data
   layer fills it with what `menus#is-available` answers, so the page and the
   command read one fact.
2. **A menu row shows whether its item can be ordered, before the press**:
   `{#if item.available}` its Add, `{:else}` "Sold out". A sold-out item has
   no Add to press.
3. **A stock change is `InventoryChanged(store, item)`.** The store's `Menu`
   query listens for it, as its fragment did:
   `invalidates_on MenuChanged(id), InventoryChanged(id, _)`. It drops that
   store's kept menu. It does not drop the store's recommendations, which
   listen for the menu's own changes (ADR-0165).
4. **Every new version of the shared menu fragment reaches every page that
   shows it, as the whole difference from what the page showed.**
   - **A told change sends its own structural patch, then the rest.** Its
     own patch is sent where it moves the list: E7-P's insert at its anchor,
     a removal, a move. Then every row that differs from what the pages show
     is set where it is, or rendered again where it is, as a session's list
     is (ADR-0145, ADR-0168). A rename and a stock change are a row's change.
   - **Before a store's document is read, its fragment is brought to the
     `Menu` query's value**, and the pages open are told. A document then
     shows what they do. The fragment is never rendered again where they
     would not hear of it.
   - This amends ADR-0150's second decision: a page already open is now
     patched for a change at the source, once a document reads the change.
5. **The renderer looks into a block that decides as it did**, and sets its
   branch's parts where they are. A block that decides otherwise renders the
   row again where it is, as does any other block that changed, such as a
   list inside the row.
6. **The value analysis types a listener's `_` as a value of any type.** By
   ADR-0091 every value matches it. `invalidates_on InventoryChanged(id, _)`
   was undecided, which the store's program test refuses. Where a key is
   evaluated, `_` still names nothing (PW0021), and decides nothing.
7. **`POST /bench/stock?item=..&available=..&tell=true` tells**:
   `InventoryChanged(47, item)`, and every page of the store sees the item
   as it is now. Untold, the control is still §15.5's forced stale item.

The command's check stays the authority (ADR-0157). What a page shows of
availability can be from before: untold, kept for the menu's freshness, or
the last menu kept while its origin fails (ADR-0177). A press of a stale Add
is refused by name.

## Alternatives

- **A disabled Add.** The HTML Standard makes an actually disabled element
  no focusable area, so a keyboard reader moving through the menu does not
  meet it, and nothing says why it is there. GOV.UK's design system advises
  against them: "Disabled buttons have poor contrast and can confuse some
  users, so avoid them if possible."
- **Hide a sold-out item.** A reader who came for it would not learn it is
  sold out, and the menu would move under a pointer as items sell out.
- **Availability as a query of its own, short-lived.** §15.2 calls
  availability short-lived. But §15.1 declares it on the item, and the page
  would join two lists in every row. What a page shows of the menu is its
  fragment, which the event regenerates whatever a query's freshness is.
- **A member computed for each row, `item.sold_out`.** What a host computes
  for a row is set by a dotted path, and a handler's captures leave those
  out (ADR-0170, ADR-0172). One named by a single segment would be carried
  in what the row's Add captures, which the command receives as the item.
  The field the type declares is the fact itself.
- **Tell no page of a change at the source** (ADR-0150 as it was). The pages
  would keep a version the server no longer derives from (Context).
- **Render the menu again on a stock change.** Every row's nodes would go,
  and focus with them (test 15).

## Acceptance

Recorded by `just e14-availability` in `docs/evidence/E14/availability.txt`:

- `pw-render` (`tests/instance_changes.rs`):
  - a block that decides as it did has its branch's parts set where they
    are, in either branch;
  - one that decides otherwise, and a list inside the row that changed,
    render the row again.
- `pw-core`:
  - a listener's `_` agrees, and its parameters are checked against the
    event's values (PW0605);
  - an evaluated key's `_` decides nothing (PW0021);
  - every relation in the store's program is decided;
  - the compiled Add handler captures the item with `available`.
- The development server:
  - a sold-out item's row says so and has no Add;
  - a told stock change renders that row again where it is, and no other;
    another store's page is not told; back in stock, the Add comes back;
  - a told stock change drops the menu and not the recommendations;
  - untold, the change reaches a new document once the menu is read again,
    and the open pages are told first;
  - a document read behind a change shows what the open pages show;
  - a rename and an insert send what changed unannounced with them, the
    insert at its anchor.
- `e2e/availability.spec.mjs`, in Chromium, Firefox and WebKit:
  - a sold-out item is shown so, and has no Add;
  - a page open when an item sells out is told, and only that row is
    rendered again; focus stays, the others add, and back in stock its Add
    returns and adds;
  - test 10 still passes, untold.
- The browser suite, in three engines.
- `scripts/availability_mutations.py`: 14 mutants.

## Not claimed

- **No count of what is left.** Availability is a `Bool`, as §15.1 declares.
- **Recommendations may name an item sold out since.** They are not asked
  again for a stock change, and they show no Add.
- **A line already in the cart is not marked when its item sells out.** A
  further increase is refused by the command (ADR-0172).
- **An untold change reaches the pages when a document reads it**, once the
  menu kept is read again: within the menu's five minutes, or not at all
  for a store no one opens. The data layer is meant to announce a stock
  change, which is told at once.
