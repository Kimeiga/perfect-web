# ADR-0181: a menu is grouped by its category, and a list inside a row is changed where it is

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the rest of the audit's eighth gap: charter §15.1's
`MenuItem.store_id` and `MenuItem.category: MenuCategory`, `Store.menu_version`,
and `Cart`'s `id`, `consumer_id` and `version`.

## Context

- **The store's menu was one list.** A delivery app's menu is read by
  category, and §15.1 gives each item one.
- Three things stood in the way of grouping it:
  - **The plan could not read a row of a loop inside a loop.** In
    `{#each section.items as item}`, `section.items` was refused as "a list
    that is not a query's", and a test pinned the refusal. No host would
    compute the inner rows' `item.price.display`.
  - **A change in a list inside a row rendered the row again** (ADR-0168,
    ADR-0178). In a grouped menu, any change to one item would have taken the
    nodes of every item in its category: their focus, and E7-P's guarantee
    that an item whose key stayed keeps its node.
  - **E7-P's insert, removal and move each sent a patch made for its own
    operation**, at the top-level list. A grouped menu has nowhere to address
    one.
- `Store.menu_version` and `Cart`'s fields: see decision 7.

## Decision

1. **`MenuItem` declares `store_id` and `category`.** The new types:
   - `MenuCategory { id: CategoryId, name: String }`;
   - `MenuSection { category, items: List<MenuItem> }`, a category's items in
     the order the store lists them.
2. **The store's `Menu` answers its sections**: `Menus.sections(id)`, the data
   layer's `menus#sections`. Categories come in the order of their first
   items.
   - Store 47's items are all coffee, so E7-P's moves stay within one list.
   - Store 48 has two categories, Drinks and Bakery.
   - The benchmark's own store reads `menus#for-store`, which still answers
     a flat list (ADR-0156).
3. **The page gives each category a heading and its items' list.**
4. **The plan reads a loop inside a loop through it.**
   - The inner loop's `section.items` is `menu.*.items`: the list in each
     item of the query's `menu`.
   - Its rows read members as a query's list's do, in text and attributes.
   - The host computes each inner row's reads within each outer item.
5. **The renderer derives a keyed list's change**: `pw_render::list_changes`,
   for a session's list and the shared menu alike. The server derived it
   until now, for a list at the top of a page alone.
   - A keyed list inside a row is diffed where it is
     (`InstanceChange::List`). Its own removals, inserts, moves and changed
     rows are each addressed inside the row's instance.
   - An item new at the head goes before the first instance, by its address.
   - An unkeyed list inside a row still renders the row again: its instances
     have no address.
6. **Every menu change is derived from the menu's values.** E7-P's
   operations change the data. The patches are the difference between what
   the open pages show and the menu now (ADR-0178), each in its category's
   list. This replaces E7-P's patches made per operation, and ADR-0178's
   step between them.
7. **A menu's version and a cart's identity stay the runtime's**, not fields.
   - The menu's version is its materialized entry's version. Every patch to
     the menu carries that version as its causal basis.
   - A cart is its session's entry: the `Cart` query is keyed by the
     session, so its id and its consumer are the session, and its version is
     the entry's.
   - A field holding any of these would be a second source of the same
     fact, free to disagree with the first.

## Alternatives

- **Categories as a filter**, as T07's tabs are: a keyed read per category
  over a flat list. A reader would see one category at a time; a store's
  menu shows them all.
- **One list, with a heading in the row where the category changes.** A row
  would need its neighbour, which no row reads.
- **Render a category again when anything in it changes.** E7-P's guarantee
  would be lost for every item beside the one that changed.
- **E7-P's operation patches beside the derivation.** Two derivations of one
  change, which ADR-0178 found could disagree.
- **`menu_version`, `Cart.id`, `consumer_id` and `version` as fields.** See
  decision 7.

## Acceptance

Recorded by `just e14-menu-categories` in
`docs/evidence/E14/menu-categories.txt`:

- `pw-core` (`tests/row_reads.rs`): a row of a loop inside a loop reads
  members of its item, planned as `sections.*.items`.
- `pw-render` (`tests/nested_lists.rs`):
  - an item renamed inside a section is set where it is, inside the
    section's instance;
  - one inserted, moved or removed is its list's own operation, and one new
    at the head goes before the first;
  - an item moved across sections is a removal from one list and an insert
    into the other;
  - a new section is the outer list's insert, rendered whole.
- The development server:
  - a menu is grouped by its category, store 48's in two;
  - a rename, a stock change and an insert are each derived from the
    menu's values, addressed inside the coffee category.
- The browser, in Chromium, Firefox and WebKit:
  - `e2e/stores.spec.mjs`: a store's categories, each with its items;
  - `e2e/keyed-list.spec.mjs`: E7-P's operations keep their nodes inside the
    category, and an insert at the head goes before the first item;
  - the whole browser suite.
- `scripts/menu_categories_mutations.py`: 7 mutants.

Also:
- **The store's artifacts change** with its types: its WIT, its contracts,
  its components and their handlers (`just e8-wit`, `just e8-contracts`,
  `just e10-component`, `just e10-handlers`). The compiled Add captures the
  item with its store and its category.
- **Tests that read the store's `Menu` as one list** give their pages a
  list query of their own.
- **The trusted platform contract's hash changes** with `examples/domain.pw`.
- **Twelve mutants of older scripts are re-anchored** where this moved their
  code. One, an inserted row's place in ADR-0178's step, is retired with the
  step.

## Not claimed

- **A list inside a speculated row** is still refused at build (ADR-0170).
- **An item moved to another category is a new node**: it leaves one list
  and enters another.
- **No category navigation**: a reader scrolls to a category.
