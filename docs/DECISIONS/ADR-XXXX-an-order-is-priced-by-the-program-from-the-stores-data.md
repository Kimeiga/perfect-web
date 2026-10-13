# ADR-XXXX: an order is priced by the program from the store's data

Status: proposed by W9, track `order-total`, under the owner's delegation of
2026-10-02, with the integrator's rulings of 2026-10-13. Date: 2026-10-13.
Milestone: E14 (NEXT, the DoorDash customer app's item 6, step 6b: the
order's total). Builds on ADR-0327 (money is rounded only where a program
names how), ADR-0328 (a store's hours), ADR-0304 (the clock), ADR-0307
(what a page asks of its reader), ADR-0323 (a member read wherever a page
reads it) and ADR-0193 (an order placed from the cart).

## Context

- **The cart said "Delivery fee and taxes are added at checkout", and none
  was.** An order was its lines; nothing priced a fee, a tax, a tip or a
  discount.
- **Each charge is a rule someone wrote down.**
  - DoorDash charges a delivery fee, a service fee that is a percent of the
    subtotal within a least and a most, and a small-order fee under a
    subtotal (read 2026-10-12, NEXT's research).
  - Sales tax rounds a half cent up: California's Regulation 1700(a)(3),
    New York's 20 NYCRR 530.1(a), Texas's Tax Code §151.053(a). Texas taxes
    the sale, not each line (34 TAC §3.286).
  - San Francisco's rate is 8.625%: CDTFA's table, effective 2026-10-01,
    read 2026-10-13.
  - A tip added automatically is a mandatory charge, and taxed: California's
    Regulation 1603(h), New York's TB-ST-320. A tip the customer chooses is
    not taxed.
  - A discount the retailer funds lowers its gross receipts; one a third
    party reimburses does not: California's Regulation 1671.1 (and CDTFA's
    restaurant guide: a third party's reimbursement "is considered part of
    your gross receipts"), New York's TB-ST-806.
- **A total shown is a total promised.** A fee schedule, a rate or the clock
  can move between the page and the press.

## Decision

1. **Each store's fee schedule is its data, on both layers**
   (`checkout.rs`, migration `0013_order_total.sql`): a delivery fee; a
   service rate (a `Rate`, ADR-0327) with a least and a most; a small-order
   threshold and fee; a tax rate; and whether delivery, and service with
   the small-order fee, are taxed.
   - **The seed is San Francisco's**: delivery $2.99; service 15%, $3.00 to
     $12.00; $2.50 under $10.00; 8.625%; neither flag set.
   - A cart's stores' schedules are read in **one statement**, `WHERE store
     = ANY($1)`, through the counted connection, pinned at one statement for
     one store and for two.
2. **The total is the program's, in one place** (`examples/store/
   checkout.pw`), by ADR-0327's money alone: every part of an amount at a
   rate and a named rounding, never `/` on cents.
   - **A cart is priced store by store, then added.** Each store's part has
     its own delivery fee, its service fee clamped to its own bounds, its
     own small-order fee, and its tax at its own rate, **once**, on that
     part's taxable base, never per line.
   - **Every fee, and the tip, is of the subtotal before any promo.** A
     promo discounts its store's food alone, so it never lowers a fee or the
     tip.
   - subtotal = the lines; discount = the promo's, half up, between nothing
     and the store's subtotal; service = at_rate(subtotal, rate, HalfUp),
     clamped; small = the fee under the threshold; taxable = subtotal − a
     store-funded discount + delivery where taxed + service and small where
     taxed; tax = at_rate(taxable, rate, HalfUp); a part's total = subtotal
     − discount + delivery + service + small + tax; the order's = the parts'
     + the tip.
3. **The tip is the reader's choice, never a default, never taxed.** None
   until they choose; 15, 18 or 20 percent of the subtotal, half up; or an
   amount, typed as dollars and read exactly from its digits into cents,
   never through a float: more than two decimals, a sign or nothing typed
   is not an amount. A negative amount is refused (Money carries no
   invariant). No tip button is shown as chosen first.
4. **A promo code is data, and one store's**: percent or amount off, a least
   subtotal, an expiry instant compared through `clock.passed` (so the
   checkout is told when it expires, ADR-0304), funded by the store or the
   platform. Each refusal by name: `UnknownCode`, `Expired`,
   `BelowMinimum(least)`, `AlreadyUsed`, and `NotInCart` for a code whose
   store has no line in the cart.
   - **Once per reader, by the data**: `promo_uses`' key on `(code,
     owner)`. `place_order` records the use before the order; of two orders
     at once, the second waits on the first and finds the conflict, refused
     `Promo(AlreadyUsed)`.
5. **The tip and the code are the cart's, on the server**: `choose_tip`,
   `choose_tip_amount`, `apply_promo` and `remove_promo` set them, and the
   `Checkout` query prices the cart from the data, the choices and the
   clock.
6. **The reader pays the total they were shown.** `place_order(shown)`
   prices the cart again from the data in its own transaction, never from
   the page's numbers, and refuses `Promo(e)` where the code no longer
   applies and `TotalChanged(now)` where the total is not the one shown.
7. **The order records what it charged, per store**: `order_parts` (store,
   subtotal, discount, who funded it, delivery, service, small-order fee,
   the taxable base, the rate applied, the tax); the order keeps the tip,
   the total and the code. A later rate never rewrites what was charged.
8. **The cart page itemizes each charge** under a disclosure, its code
   field and its tips; the order page says what it was charged. The
   disclosure is decided by the checkout (`checkout.any`), not the cart's
   lines: a block the browser renders again from a speculation of the cart
   computes only from the cart (ADR-0323's decision 2), and the total is
   the checkout's.

## The integrator's, 2026-10-13

- **The tax position of the seed**: neither delivery nor service taxed.
  - The platform charges the delivery, service and small-order fees for its
    own services; couriers are its independent contractors.
  - The platform is a delivery network company that has not elected to be a
    marketplace facilitator: Regulation 1684.5(a)(2), a DNC "is not a
    marketplace facilitator, unless it makes an election" (the election is
    under (e)).
  - The store is the retailer of its food, the platform its agent: CDTFA's
    Tax Guide for Restaurant Owners, "online ordering service": the
    restaurant is "considered the retailer of the meals" and owes tax on
    "the meals' full selling price". So the platform's fees are not the
    store's gross receipts, and California does not tax the platform's
    services.
  - A restaurant delivering its own food is another model: the guide's "Hot
    prepared food is taxable including any delivery fees you may charge",
    and Regulation 1628(b) taxes delivery by the retailer's own facilities.
    And "whenever a surcharge is separately added to any taxable sale, the
    surcharge is also subject to tax". So both flags stay per-store data,
    and the tests set each both ways.
- **Pricing per store part**: approved, every fee and the tip on the
  subtotal before any promo, the tip in no tax base (Regulation 1603(h)),
  the tax at the store's rate.
- **Promos**: approved, the tax base following who funds a discount, an
  amount off clamped to the store's subtotal and never below nothing.
- **The tip**: NoTip the default and no button preselected; the dollars
  read exactly; a negative amount refused.
- **The total shown is the total charged** (`TotalChanged`), the charges
  per store part with the rate applied, and single use in the data.
- **Discounts round half up**: toward zero would make every percent off
  worth up to a cent less than it says, a bias against the customer. No law
  governs a discount's rounding; this is the integrator's reasoning.
- **`is_less`**: ADR-0327 lists it among money's exact operations and
  master never declared it. Added here, with its mutant: an ADR-0327
  correction, found by W9 in 6b.
- **A page cannot send a command a variant argument**, so the tip is chosen
  by two commands, `choose_tip(percent)` and `choose_tip_amount(amount)`,
  the data a variant (`Tip`). Accepted for 6b; Pleris's to fix, when the
  API becomes `choose_tip(Tip)`.
- **A query answers data and the page words it**: `OrderTotal` answers the
  latest order's total as `Money<USD>`, nothing where none is placed, shown
  only in the order's arm (a top-level binding's member read inside a block
  is computed, ADR-0323). When a member read of a match arm's binding is
  computed (the integrator's track), it becomes `Option<Money<USD>>` read
  through the arm.

## Alternatives

- **One schedule for an order of several stores.** Which store's? Each
  store's part is its own sale, at its own rate.
- **A promo over the whole cart.** It would have to be allocated across
  stores with different rates (ADR-0327's "Not claimed").
- **The page's total trusted.** A page is the reader's; a total it sends is
  what it says, not what the data says.
- **A tip as a percent by default**, as some apps preselect: an automatic
  tip is a mandatory charge, taxed.

## Acceptance

- `tests/order_total.rs`, in memory and on PostgreSQL (9 tests), each rule
  with its control: Texas's $20.625 charged $20.63, $20.274375 $20.27; New
  York City's 8.875% with delivery taxed and not; the service fee clamped
  at $3.00 and $12.00, $5.10 between, with service taxed and not; the
  small-order fee under its threshold and not at it; each code's refusal by
  name, and 15% of $3.50 taken off as $0.53; the tip untaxed, a percent not
  offered and a negative amount refused; a store's discount lowering the
  tax and the platform's not; the order charged the page's total, per store
  part with the rate applied, and a stale total refused `TotalChanged`.
- `checkout.rs`' tests on PostgreSQL: the fees of one store and of two in
  one statement; two transactions using one reader's code, the second
  waiting and finding the conflict, another reader's use recorded.
- `e2e/order-total.spec.mjs`, three engines: the cart itemizes each charge
  ($16.09 for two espressos), a 20% tip of $1.40 untaxed, and the order
  page says "Total charged: $17.49"; a code that is not one refused by name.
- `scripts/order_total_mutations.py`, 22 mutants, narrowed by name to
  `tests::order_total::` and `checkout::tests::`.

## Not claimed

- **A delivery across a district line**: California's district taxes follow
  the destination (Regulation 1823); the rate here is the store's.
- **A rate that changes on a date**: a store's rate is its schedule's now.
- **Allocating one discount across stores** (ADR-0327).
- **Payment**: 6c, the integrator's (`AwaitingPayment`, an order placed only
  from a verified webhook). `place_order` writes the order with its charges.
- **A guest's choices following them in** when they sign in: the tip and
  the code are their owner's.

## Merge notes

- Migration `0013_order_total.sql`.
- Regenerated: the store's IR, graph, WIT, contracts, handlers, the store
  page's plan and components; `store-values.json` gains the checkout's
  values; the trusted platform contract's hash moves with `domain`.
- Moved: `build.rs`' components; `handlers.rs`' handlers and parts;
  `blocks_that_render_a_query.rs`' cart page anchor and planned blocks;
  every `place_order` call in the server's tests passes the shown total.
- Re-anchored: `orders_mutations.py`' "a placed order is not recorded" and
  `store_postgres_mutations.py`' "an order is placed without its lines", on
  `checkout.rs`, where a user's order is placed with its charges.
- KNOWN_LIMITATIONS:
  - no CDTFA text addresses a platform's own fees directly, so the
    deployment's tax adviser sets the flags;
  - CDTFA proposed in 2026 to drop the DNC opt-out; whether it passed is
    unconfirmed. If it does, the platform becomes the retailer and the
    flags change;
  - a page cannot send a command a variant argument; the tip is chosen by
    two commands until it can;
  - a member read of a match arm's binding is not computed; OrderTotal
    answers cents(0) where no order is placed, shown only in the order's
    arm, until it is (the integrator's track);
  - a cart's lines are mapped to their stores by a read per store
    (`checkout#parts`, as `hours#for-cart`), until the track that bounds
    every read.
- An ADR-0327 correction: `is_less` declared, found by W9 in 6b.
