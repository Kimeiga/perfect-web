# ADR-0323: a member read is computed wherever a page reads it

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Takes NEXT 3c, found on
the way to ADR-0320. Builds on ADR-0125 (what a page shows, as reads of
what its queries return), ADR-0048 (a member function read as a field),
ADR-0169 (a row's member reads), ADR-0226 and ADR-0229 (a computed part, at
the top and inside a block), ADR-0235 (what a speculated region computes)
and ADR-0320 (a block told part by part).

## Context

- **A member read was computed only in text at the top of a page.**
  - `{cart.line_count}` reads `domain.line_count(cart)`, a member, since a
    cart has no such field (ADR-0048). At the top of a page a host computes
    it, a part of its own (ADR-0125). In a loop's row, of the row's item, a
    host computes it for each row (ADR-0169).
  - Inside a block it was refused: "reads a member of `cart` inside a
    block, which a host would call per instance". A block at the top of a
    page is rendered once a change, not per instance, and
    `{#if cart.lines}<p>{cart.line_count}</p>{/if}` did not build (found
    writing ADR-0320's tests, 2026-10-12).
  - So were one in an attribute, at the top or inside a block, in what a
    block decides by, `{#if cart.has_lines}`, and in the page's title and
    metadata: "which no host computes there".
- **The same value written as a call was computed.** `{counted(post.likes,
  "a", "b")}` inside `{#if post.pinned}` is a computed part a host computes
  into what the block is rendered with (ADR-0229). The member's written form
  was refused where its call's was not.
- **A program wrote around it.** The store's page shows its subtotal
  outside the block its lines decide, and its fees' note inside it. Its
  link to the cart's page says "Go to your cart" whatever the cart holds,
  where DoorDash's store page says "Checkout" with the subtotal.

- **Found writing this ruling's tests: a member read inside a view that
  contains itself built, and nothing computed it.** Each use of such a view
  is an instance made at run time (ADR-0203), whose template no page's plan
  reads. A computed value there is refused, "a view that contains itself
  computes none yet" (ruling 0073-a). `{post.shout}`, a member of `Post`
  inside the feed's `Replies`, passed every check, and its page could not
  have rendered it.

## Decision

1. **A member read of a query's value, outside a loop's list, is a part a
   host computes once a change**, into what the page is rendered with, by
   its path: `cart.line_count`.
   - Inside a block, in text or an attribute, or in what a block decides by,
     the block is rendered with it. The block is planned as one that renders
     a query's value (ADR-0320), so a host tells it part by part while its
     arm stays.
   - Inside a loop's row, the page's own binding is read neither by field
     nor through a member, as before: a row is rendered again for its own
     item alone (ADR-0146). A member of the row's item stays the row's
     (ADR-0169).
   - An attribute at the top of the page is set again when it changes, as a
     computed attribute is (ADR-0226). The title and the metadata are
     written from it, as from any value.
   - The steps are the top-level part's: each field, and the member by its
     own component. No new component is compiled.
2. **A block the browser renders again from a speculation computes it
   there** (ADR-0172, ADR-0235): the expression that reads it, compiled
   from the value the page speculates on, given to the renderer with the
   region's values by its path. One inside a block the browser does not
   render again, of a value it speculates on, is refused there as any read
   is: "a speculated value read inside a block" (the store's count inside
   each menu item, `pw-conformance`'s test).
3. **A loop's list read through a member is still refused**, by name: a
   host keeps a list it reads by field, and patches its rows where they are
   (ADR-0170).
4. **A member read inside a view that contains itself is refused**, as a
   computed value there is, by name: "`<Replies>` contains itself and reads
   `post.shout` through a member function, and a view that contains itself
   computes none yet (ruling 0073-a)".
5. **The store's page names its link to the cart's page "Checkout", with
   the subtotal, while the cart holds a line**, and "Go to your cart" while
   it holds none: one block, shown with the first line before the server
   answers, as the line is (ADR-0172), and back with the last line gone.

## Alternatives

- **Write the call instead** (`{line_count(cart)}`): it builds today, as a
  computed part. Rejected as the answer: the two forms read the same value,
  and a reader of the page should not need to know which a host computes.
- **Lift each member read into a computed part's function**, as a computed
  hole is (ADR-0226). Rejected: the member is already a component, which a
  top-level part runs. A function per read would compile it again.
- **Compute it once for every part that reads one path.** Each part keeps
  its own, as a top-level part does: two parts that read one member run it
  twice a change. Not taken here; a host's derive is not where a page's
  time goes (ADR-0317's measure).

## Acceptance

- `compiler/pw-core/tests/member_reads_in_blocks.rs`, 5 tests:
  - the finding: a member read inside a block, computed through the
    member's component, its block planned; the control, the page as it is;
  - the page's binding inside a row, by field or through a member, still
    refused (ADR-0146), and the row's own member the row's;
  - an attribute at the top and inside a block, what a block decides by,
    the title and the metadata;
  - a loop's list through a member refused, by name, and read by field
    built;
  - the cart's speculated block computing the count in the browser, in
    text and in an attribute; the control, the page as it is;
  - a member read inside the feed's `Replies`, which contains itself,
    refused by name; the control, a field there.
- The development server: the count inside `{#if cart.lines}` shown with
  the block, set where it is at the next change beside the top's, and gone
  with the block; and in a block a speculation renders, rendered whole with
  it.
- `e2e/cart.spec.mjs` in three engines: "Checkout $3.50" before the server
  answers the first press, "Checkout $7.00" after the next, and no link
  once the last line is removed.
- `scripts/member_reads_mutations.py`, 8 mutants, each killed.

## Not claimed

- **A loop over a list a member computes.** Refused, as before.
- **A member read inside a view that contains itself** is refused, not
  computed: an instance's template is no page's plan's (ruling 0073-a).
- **Two parts that read one member run it twice a change**, as two parts
  at the top of a page do.
