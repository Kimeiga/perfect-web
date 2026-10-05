# ADR-0192: the stores, as the home page

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. The next of the store's pages, after its cart's (ADR-0190).

## Context

- **A delivery site starts with its stores.** A person chooses one from a
  list before reading its menu. The store's program had no list: its pages
  were a store's, at `/stores/{id}`, and its cart's, at `/cart` (ADR-0190).
- **The development server answered `/` with store 47's page**, an alias
  from E7. No program could declare the root.
- **The store's data layer read one store at a time**, `stores#get`
  (ADR-0125). Nothing listed them.

## Decision

1. **`Stores.list()`, a host operation of the store's data layer**,
   `store:data/stores#list`, under `database.read<Stores>`, as
   `Stores.get` is. The development server answers it with every store it
   holds, 47 and 48, each the record `stores#get` answers.
2. **`StoreList()`, a public query** over it, cached `shared` as the
   `Store` query is.
3. **`HomePage`, at `/`.** Each store by its name and description, its name
   a link to its page, and the session's cart beside them, a link to the
   cart's page and its count. The page is the session's (`cache private`),
   because it shows the session's cart, and the count is kept current as
   the cart changes (ADR-0190).
4. **`/` is the page that declares it.** Store 47's page stays at
   `/StorePage.html`, for E7's measurements, and at `/stores/47`.

## Alternatives

- **List the stores in the store's page.** A store's page is one store's,
  at its own address. A list belongs to a page of its own.
- **Make the home page public, cached `shared`**, as its list is. Its cart
  is the session's. A page that shows a session's value is the session's,
  and ADR-0184 sends it `private, no-store`.

## Acceptance

Recorded by `just e14-home` in `docs/evidence/E14/home.txt`:

- **The development server's test**: the home page at `/`, titled, listing
  each store by name and description with a link to its page, and its cart's
  count patched as the cart changes.
- **`e2e/pages.spec.mjs`**, in three engines: from the home page to a
  store's; its count moved by an add on a store's page, open beside it.
- **`e2e/accessibility.spec.mjs`**: the home page keeps every rule, as
  served and with its cart counted.
- **`scripts/home_mutations.py`**: 2 mutants.

## Not claimed

- **A change to a store's name or description reaching an open home page.**
  No change to public data other than the menu reaches open pages
  (ADR-0190).
- **Searching or filtering the stores**, by cuisine or distance.
