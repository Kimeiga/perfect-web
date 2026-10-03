# ADR-0145: a page keeps every part and list its queries decide current

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-02.
Milestone: E14 (E14-Q, third slice), for T03.

## Context

E14-Q's first slices made the page's values its queries': the compiler plans
each binding and part (ADR-0125), and the server runs each query by its
declared policies (ADR-0127). After a command, though, the server patched
the store's document by name:
- the cart's count, `cart.line_count`, by a text patch;
- the menu, a public fragment, by E7-P's structural patches, broadcast to
  every reader.

Nothing else a page showed was rendered or kept current generally. T03 adds
the cart's lines below the count, from a private query of their own. That
store did not render at all: the server never gave the renderer the list.
Had it rendered, adding an item would have left the list as it was until a
reload.

The protocol had a second, quieter gap. A patch carries the basis it was
derived from, and the browser applies only a patch whose basis advances
what it holds. One change reaching two places would be two patches with one
basis, and the second would be refused as advancing nothing.

## Decision

1. **The server renders every list a session's queries fill.** Each
   collection the page plan names, from its binding's value, as each part
   is. The menu's shared fragment stays as it is. `pw-render --plan`, which
   renders a page for no session, renders such a list empty when the values
   give none, as the cart's count is 0 there. A list from a shared binding
   must still be given.
2. **What a session's document shows is recorded, and a change is the
   difference from it.** The server records each part's text and each of
   the session's lists when it serves the document, and again as it sends
   each change. After a command it derives:
   - a text patch for each part whose text changed, and none for one that
     did not;
   - for each list, keyed operations: an item whose key left is removed; a
     new one is inserted after the one before it, or at the head; one out of
     place is moved; one whose value changed is set in place, text part by
     text part, inside its instance. When the item's markup holds more than
     text, a button with its captures for instance, it is rendered again
     where it is.

   An item whose key stays keeps its nodes, as E7-P keeps the menu's.
3. **One change, one frame** (`pw-protocol`, `PatchSet`). Every patch one
   change derives travels in one `patch_set` frame, with the change's basis.
   The browser applies them in order, rebuilding its index after each
   structural one, and holds the new version once all of them applied. A set
   with no patches is still sent: the document reflects the new version when
   nothing it shows changed. The single `patch` frame stays, for the menu's
   broadcast.
4. **A block a query decides is refused by the plan, for now** (planned
   and kept current since ADR-0146). Found while
   building this: such a block was left out of the plan, so a host had no
   value for its subject. The store with `{#if cart.lines}` checked and
   built, and the server failed at its first render. `pw build` refuses the
   page now, naming the block, until a host renders and patches one.
5. **A set the document cannot apply whole reloads it.** A patch addressing
   a part or instance the document does not have means the document is not
   what the change was derived from, and every later change would be derived
   from it too. The version is not held, and the page is read again, as a
   `reload` recovery reads it. Never "try anyway".

## Alternatives

- **Render each changed list again whole** (`ReplaceRange`). Rejected: every
  item's nodes would be replaced on every change, losing focus and anything
  a person had open, which is what a keyed list exists to keep.
- **One patch per place, each with a version of its own.** Rejected: it
  would make every place its own entry, when they are one change to one
  entry. A page that applied some would claim a version for each, and none
  for the change.
- **Apply what applies and carry on.** Rejected (5).

## Acceptance

- Server tests on the store with T03's list, built by the compiler as `pw
  build` builds it (`Build::write`, moved from the CLI into `pw-core` for
  this):
  - the list renders, and is the session's alone;
  - one frame per change, holding the count and the inserted line;
  - a line that stays is set in place;
  - a new line goes after the one before it;
  - a line that leaves is removed;
  - each change is derived from the last one sent;
  - the menu is no session's list;
  - a change reaches its own session's document alone;
  - a moved list is moved, and an item holding more than text is rendered
    again.
- `pw-protocol`: a patch set round-trips, and its version is checked.
- `pw-render`'s `plan_lists.rs`: a private list the values do not give is
  empty, and a shared one must be given.
- `page_blocks.rs`: a block a query decides is refused by the plan, with the
  store as it is as the control.
- Browser tests in `resource-path.spec.mjs`, in Chromium, Firefox and WebKit:
  - a patch set is held once, after all of it applied;
  - a set the document cannot apply whole reloads it.
- T03's four controls on all three stacks, and the own-renderer suite.
- `scripts/patch_set_mutations.py`: mutants across the server, the protocol,
  `pw-render` and the browser's runtime.

## Not claimed

- **A shared list other than the menu** is rendered, and is not patched.
- **A block a query decides** (4), and so a list inside one. A list is
  patched at the top of the page: one in a block not shown has no range.
- **A list of lists**: an instance's address carries one frame here.
- **Any page but the store's**: the server serves queries for the store's
  route (ADR-0140).
