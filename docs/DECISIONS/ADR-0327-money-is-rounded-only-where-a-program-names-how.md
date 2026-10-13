# ADR-0327: money is rounded only where a program names how

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-13. Milestone: E14 (NEXT, the DoorDash customer
app's item 6, step 6a: checkout's money). Builds on ADR-0048 (an opaque
type's representation is its module's), ADR-0179 (an opaque type's
invariant), ADR-0039 (Euclidean `/` and `%`), ADR-0172 (a cart's lines and
subtotal) and the ruling of 2026-08-20 (opacity is not ABI transparency).

## Context

- **Any module could round money, silently.** `Money<C>` was `Money {
  minor_units: Int }`: every module read an amount's cents and could divide
  them with `/`, which floors (ADR-0039). Checkout (item 6) takes parts of
  amounts at rates: a service fee, a sales tax, a tip, a percent off. Each
  is a rounding, and the law names it: California's Regulation 1700(a)(3),
  New York's 20 NYCRR 530.1(a) and Texas's Tax Code §151.053(a) each make a
  half cent or more a cent (read 2026-10-12, NEXT's research).
- **The charter's example already wrote money as nominal.** A-001
  constructs `Money(n)` from a count of cents and adds amounts with
  `Money.add`, a function nothing declared; the check passed it, as it
  passes a call it cannot decide.
- **Practice.** Java's `RoundingMode.HALF_EVEN` "statistically minimizes
  cumulative error when applied repeatedly over a sequence of
  calculations"; Stripe rounds its fees half up, and Stripe Tax rounds half
  up; Square's Orders API rounds half to even (each read 2026-10-12). Half
  even under-collects a tax the three states round half up: Texas's
  $20.625 is $20.63, and half even makes it $20.62.

## Decision

1. **`opaque type Money<C> = Int`**, an amount's minor units, read in
   `domain` alone (ADR-0048). No function gives another module an amount's
   units: a page shows one with `display`, and a host passes one as an
   `s64`.
2. **Exact operations**: `cents(n)`, `plus`, `minus`, `times(amount,
   count)`, `sum(amounts)` and `is_less`. `total` and `subtotal` are
   written with them.
3. **A part of an amount only at a named rate and rounding**:
   - `opaque type Rate = Int where value >= 0`, in millionths (150,000 is
     15%, 88,750 is New York City's 8.875%), built by `percent(p)` or
     `millionths(n)`, each `None` for a negative rate;
   - `type Rounding = HalfUp | HalfEven | TowardZero | AwayFromZero`,
     `HalfUp` a half away from zero, as the three states' taxes are;
   - `at_rate(amount, rate, rounding)`, the one place an amount is divided.
4. **A generic opaque type whose representation never mentions its
   parameters crosses as its representation**, as every opaque type that is
   not generic does and as a phantom generic record does (`wit.rs`'s
   `phantom`): `Money<USD>` is `type domain-money = s64` on the wire, and a
   browser writes an amount as a whole number. This amends the ruling of
   2026-08-20 for that case alone. A generic opaque type is never its
   *argument's* representation, a privacy qualifier's transparency, which is
   what the ruling refused; and one whose representation mentions its
   parameters (`Wrapped<T> = List<T>`) has a layout per argument and still
   no WIT form. The ruling's two controls say so now
   (`canonical_abi.rs`'s `a_generic_opaque_type_is_never_its_arguments_representation`,
   `signature_behavior.rs`'s `same_spelled_user_type_does_not_impersonate_the_privacy_constructor`).
5. **A-001 says it with the money API**: `times(line.unit_price,
   line.quantity.count)` and `sum`.
6. **The benchmark's store takes the same money** (`benchmarks/baselines/
   pleris/domain.pw`): the development server serves both stores, and passes
   an amount as an `s64` to each. No task's patch touches money.
7. **USD alone.** No other currency, no conversion, no locale's format:
   `Money<C>`'s parameter is a marker the program has one of.

## Alternatives

- **Keep the record and add the functions.** Any module could still divide
  `.minor_units`; "rounded only where named" would be a convention.
- **`opaque type Money<C> = Cents` over a record**, so the wire keeps `{
  "minor_units": n }`. The same opacity, with a wrapper the boundary does
  not need.
- **A float rate.** A rate of 0.08875 is not 8.875% in binary floating
  point; millionths are exact for every US rate written to a thousandth of
  a percent.
- **One rounding for everything.** A tax and a fee each name the rounding
  their rule asks; a default would be the silent rounding this removes.

## Acceptance

- `compiler/pw-conformance/tests/javascript.rs`'s
  `money_is_rounded_as_the_program_names_it`: each rounding at halves of a
  cent either side of zero, a third, New York City's 8.875% and Texas's half
  cent, as the component computes it, and the JavaScript module agreeing;
  a negative rate is none; a sum past an `Int` traps in both.
- Another module reading an amount's units is refused (PW0610, "has no
  member").
- The store, its contracts, components and IR built again; the host and the
  development server pass amounts as `s64`; every program checks as before
  (`just ci`).
- `scripts/money_mutations.py`, 11 mutants, each killed (`just e14-money`).
- The trusted platform contract's hash moves with `domain`'s money
  (`platform_contracts.rs`), said there.

## Not claimed

- **Allocating an amount across lines** (a tax over items, a discount over
  stores): nothing asks for it yet.
- **Another currency**, a conversion, a locale's format.
- **A rate above the whole** is allowed: a 150% surcharge is a rate.
