# ADR-0193: an order is placed, and its page follows it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. The last page of a delivery's flow: after the stores
(ADR-0192), a store and its cart (ADR-0190), the order.

## Context

- **A delivery is placed and then followed.** A person places the cart as an
  order and watches it move: placed, prepared, on its way, delivered. The
  store's program could fill a cart and clear it, and nothing after that.
- **The data layer was the cart's.** A command's writes were staged cart
  lines, and committed with the cart's materializer entry (ADR-0172). There
  was no order to write.
- **Only a command on the cart reached an open page.** After a commit,
  `drain_held` read each of the session's documents again and sent what
  changed, against the cart's entry. A change the store makes is no command:
  E14's T04 had the kitchen set an order's status, and a page showed it only
  when it was read again.
- **The browser's rule for a patch** is that at least one entry of its basis
  advances and none moves back (`isNewer`, ADR-0145). A change sent against
  the cart's entry at a version the cart never had would break that rule.

## Decision

1. **The program.**
   - `OrderStatus`: `Placed`, `Preparing`, `OnTheWay` and `Delivered`.
     `OrderError`: `NothingToOrder`.
   - An event `OrderChanged(session)`.
   - A library `Orders`: `current(session)` reads `store:data/orders#current`,
     and `place(session)` writes `store:data/orders#place`, under
     `database.write<Orders>` and `database.write<Carts>`.
   - A session query `Order(session)`, `invalidates_on OrderChanged`.
   - A command `place_order()`, idempotent by interaction. It invalidates
     `Cart` and `Order`, and emits `CartChanged` and `OrderChanged`.
2. **An order is the cart's lines, placed in one commit with the emptied
   cart.** `orders#place` stages the cart empty and the order `placed`, and
   the command's commit writes both. An empty cart places nothing, answered
   with its declared error.
3. **A change the store makes reaches the session's open pages.**
   `session_changed` delivers the program's declared event, which drops a
   read of the order in flight from before the change. It then reads each of
   the session's documents again and sends what changed, against the
   order's own entry at a version that advances. The cart's value goes only
   with the cart's own change.
4. **The pages.**
   - **The cart's page** places its cart, says when it is empty, and links to
     the order once there is one.
   - **The order's page**, at `/order`, shows its status in a live region a
     screen reader says, kept current as the store moves it along.
5. **The development origin grants `database.write<Orders>`**, named in its
   written-out topology. Admission refused the command until it did, as it
   should have.

## Alternatives

- **Poll for the order's status.** A page asking every few seconds is late
  by up to the interval, and asks when nothing changed. The server knows
  when the store said something.
- **Send the store's change against the cart's entry.** The cart did not
  change, and a version spent on it is one the cart's next change could not
  use.
- **Place the order from the store's page.** A delivery site places from
  its cart, where the lines and the subtotal are read, and the cart's page
  is that page.

## Acceptance

Recorded by `just e14-orders` in `docs/evidence/E14/orders.txt`:

- **The development server's tests**:
  - an order placed from the cart, the cart empty after, and a page open on
    the order sent the change;
  - an empty cart placing nothing;
  - the store moving an order along through its address reaching the open
    page each time, against the order's entry at advancing versions, with no
    speculated value sent with it.
- **`e2e/pages.spec.mjs`**, in three engines: the order placed from the cart's
  page, followed on its page through every status without a reload; an empty
  cart placing nothing.
- **`e2e/accessibility.spec.mjs`**: the order's page keeps every rule, with
  no order, placed and being prepared.
- **`scripts/orders_mutations.py`**: 7 mutants.

## Not claimed

- **A read in flight from before the store's change.** The event drops it,
  and no test makes the order's source slow enough to show it.
- **Payment, an address, or a driver.** The order is the cart's lines and a
  status.
- **Several orders a session keeps.** A session has one order, its latest.
- **A change to public data reaching open pages** (ADR-0190).
