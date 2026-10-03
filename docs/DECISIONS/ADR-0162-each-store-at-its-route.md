# ADR-0162: each store at its route

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The third step of the audit's route gap: charter §15.3's
`/stores/:store_id`, and §15.6 test 11, "`MenuChanged(store_47)` invalidates
store 47 only", which needs a second store to mean anything.

## Context

- **The compiler planned the route** (ADR-0160), and each document became its
  own subscriber (ADR-0161).
- **The development server still held one store.**
  - It served the store page at `/StorePage.html` and `/`.
  - It gave the page's `id` a constant, 47.
  - It kept one materialized menu fragment, and sent a menu change to every
    page it held.
- **Test 11 was met only inside `pw-materialize`'s own tests**, since no page
  showed a second store.

## Decision

1. **A path is routed by the plans' routes.**
   - One segment per `{parameter}`, as the compiler's link check matches a
     link, and literal segments equal.
   - A segment is decoded as a path is: `%XX`, and a `+` as itself. The
     server's query decoding makes `+` a space, as a form does.
   - The store page is served at `/stores/{id}`. A page that binds no query
     is served at its route as at `/page/<path>`.
   - `/StorePage.html` and `/` stay store 47's, for the benchmark's store,
     which declares no route, and for the suite that addresses it so.
2. **A document's parameters are its own**, registered with its subscriber
   before it is read, and forgotten with it.
   - A binding's argument that names a page parameter is the document's
     value.
   - Every read for the document uses it: when it is served, when a change
     reaches it, and when a keyed read is made for it.
   - A keyed read for a document the server does not hold is dropped before
     its arguments are computed, since they are that document's.
3. **The development server holds two stores**:
   - 47, Blue Bottle, whose menu is the keyed list E7-P changes;
   - 48, Harbor Coffee, with a menu of its own that nothing changes.
4. **Each store has its own menu fragment.** Its entry identity is
   `Menu(store)`, its identity domain is the store's, and the items it was
   rendered from are kept per store.
5. **A change to a store's menu reaches the documents that show that
   store**, and no other's (test 11). A change to the session's cart reaches
   every document of the session, whatever store it shows.

## Found on the way

- **A mutant of the server run against the browser ran the old server.**
  `run.sh` builds the page, not the server the suite runs, so "no page is
  named by its route" survived its first run. The browser runner of this
  script, and of ADR-0161's, now builds the server too. Every other
  script's browser mutants change the runtime, which `run.sh` copies.
- **"one menu fragment for every store" survived its first run.** No test
  served store 48 again after 47. With one fragment, that page would have
  shown 47's menu, kept since the last render. The test does so now.

## Acceptance

- Server tests:
  - a path gives a route its parameters: one segment each, `%XX` decoded,
    `+` kept, and a malformed escape refused;
  - `/stores/48` is the store page, with Harbor Coffee's name and menu, and
    none of store 47's;
  - a change to store 47's menu patches the 47 page and not the 48 page,
    and an add patches both.
- `e2e/stores.spec.mjs`, three engines:
  - each store is served at its route;
  - the second store's Add adds to the cart;
  - test 11 end to end: store 47's renamed item reaches its page, and store
    48's page is as it was and does not reload.
- The browser suite.
- `scripts/stores_mutations.py`: 7 mutants, recorded by `just e14-stores`.

## Not claimed

- **An unknown store answered 404.** `/stores/999` reads a store the data
  layer does not have, and is answered 503, as a page whose queries fail
  is (ADR-0147). Which declared error means "absent" is the page's to say.
  That is the next ruling.
- **A home page listing the stores.** Its links would be checked against the
  route table (PW5009).
- **A cart per store.** The cart is the session's, as the domain declares it.
