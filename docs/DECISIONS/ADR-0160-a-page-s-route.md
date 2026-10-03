# ADR-0160: a page's route

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The audit's next gap (`docs/research/charter-15-store-audit.md`):
charter §15.3's route `/stores/:store_id`, and more than one store. This is
the compiler's half. Serving a page at its route needs each document to be
its own subscriber first (the next ruling), then the server (the one after).

## Context

- **A route was read only to check links.** A `route "/stores/{id}"` clause
  went into the route table that internal links are checked against (PW5009).
- **Nothing tied a route to its page.**
  - No rule said that the route's `{id}` is the page's parameter. A route
    could name a parameter the page lacks, or leave one out.
  - The page plan did not carry the route. A host could not serve a page
    where the page said.
- **The canonical store declared no route.** The development server served
  its page at `/StorePage.html` with the id fixed at 47.

## Decision

1. **The page plan carries the page's route.** One reader,
   `routes::declared_route`, serves both the link check and the plan, so the
   two cannot read a route differently.
2. **A route and its page's parameters agree** (PW0340).
   - A route is `/` and segments. Each segment is a word or a `{parameter}`.
   - It names each of the page's parameters once, and names nothing else.

   So the address gives every parameter, one segment each, the same
   one-segment rule the link check matches by.
3. **A route's parameter is text** (PW0621): a `String`, or an opaque type
   over one, which an id is. A segment is text, and the plan carries no
   decoding for a number or a flag.
4. **One route is one page's** (PW0341). Two pages at one route, whatever
   their parameters are called, would leave the page an address names to
   whichever is found first.
5. **The canonical store declares `route "/stores/{id}"`.**

## Alternatives

- **Routes inferred from files or modules.** Next.js and SvelteKit route by
  file path. `routes.rs` already rules this out: a link is dead relative to
  what the program declares, and a convention would check the app against
  itself.
- **Typed segments now, `Int` among them.** These would need the plan to
  carry each parameter's type, and a host to refuse a segment that does not
  decode. A page whose parameter is an id needs neither, so typed segments
  wait for a page that needs them.

## Acceptance

- `compiler/pw-core/tests/routes.rs`:
  - the plan carries the route, and a page without one carries none;
  - a malformed route, a name that is no parameter, a name given twice and
    a parameter left out are each refused (PW0340);
  - an `Int` parameter is refused (PW0621), and a `String` or an opaque type
    over one is the control;
  - two pages at one route are refused (PW0341), and a second route beside
    the first is the control.
- Every route already in the corpus checks as before.
- The store checks with its route.
- `scripts/routes_mutations.py`: 9 mutants, recorded by `just e14-routes`.

## Not claimed

- **Serving a page at its route.** The server still serves the store at
  `/StorePage.html`, as the next two rulings set out.
- **An unknown store answered 404.** Which error means "absent" is the page's
  to declare: a ruling of its own.
