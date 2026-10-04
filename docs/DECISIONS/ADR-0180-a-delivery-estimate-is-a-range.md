# ADR-0180: a delivery estimate is a range, and says when it was made

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the audit's eighth gap: charter §15.1's `DeliveryEstimate {
min_minutes: PositiveInt, max_minutes: PositiveInt, generated_at: Instant }`.

## Context

- The store's `DeliveryEstimate` was `{ minutes: Int }`: one number, of any
  sign, and no time it was made. The audit's §15.1 line: "missing:
  `{ minutes: Int }`, and no `Instant`".
- A delivery is not a point: a courier, a kitchen and traffic each vary. A
  delivery app quotes a range.
- The language had no `Instant`: the platform's clock answers an `Int`.

## Decision

1. **`DeliveryEstimate { min_minutes: PositiveInt, max_minutes: PositiveInt,
   generated_at: Instant }`** (charter §15.1). Each bound is a `PositiveInt`,
   so the host checks an estimator's answer: an estimate of 0 minutes is a
   failed read (ADR-0179).
2. **`Instant`**: a point in time, the milliseconds since 1970-01-01T00:00Z
   (UTC), declared by the platform's `clock` module beside the clock it is
   read from. An opaque type over `Int`, which states no invariant: an instant
   before 1970 is still an instant.
3. **The estimate's slot says the range, in words**: "Delivery in 25 to 35
   min". Not "25–35": screen readers read an en dash in a range unreliably,
   and CSUN's accessibility guidance is to write "to" (test 14).
4. **The development server's estimator answers a range**, `minutes` to
   `minutes + 10` by default, and `/bench/estimate?minutes=..&max=..` sets
   both. It answers `minutes` too, which the benchmark's own store still
   reads (ADR-0156): a program is passed the fields its type declares
   (ADR-0166).

## Alternatives

- **One number.** What the store had: it says more precision than the
  estimator has.
- **An en dash.** Shorter, and what delivery apps show; some screen readers
  say nothing for it.
- **`Instant` as a record of seconds and nanoseconds**, as WASI's wall clock
  answers. A delivery's estimate needs milliseconds at most, and one `Int`
  orders and subtracts.
- **A record's invariant, `min_minutes <= max_minutes`.** ADR-0179's
  invariants are bounds on one value; one that relates two fields would need
  the build to reason about both, and the host to check a record. Not yet
  needed by more than this.

## Acceptance

Recorded by `just e14-estimate-range` in
`docs/evidence/E14/estimate-range.txt`:

- The development server:
  - an estimate is said as a range, "Delivery in 20 to 30 min", ten apart
    unless the estimator says, and as it says, "15 to 45";
  - an estimator's answer of 0 minutes is refused by the host, and the slot
    says the estimate is unavailable, the page served;
  - the store's slots come after its own content, the estimate's "30 to
    40";
  - T10's tests, on the benchmark's own store, still read `minutes`.
- `e2e/slots.spec.mjs`, in Chromium, Firefox and WebKit: the range in words,
  as `/bench/estimate?minutes=..&max=..` sets it, and an estimate of 0
  unavailable.
- `scripts/estimate_range_mutations.py`: 6 mutants.

Also:
- **The store's artifacts change** with its type: its WIT
  (`docs/evidence/E8/store.wit`, `just e8-wit`), its contracts (`just
  e8-contracts`), whose estimate's answer is now checked at `ok.min-minutes`
  and `ok.max-minutes`, and its page's plan (`just e10-component`).
- **The trusted platform contract's hash changes** with `clock` and
  `examples/domain.pw`.
- One mutant of ADR-0166's is re-anchored where the slot changed.

## Not claimed

- **`generated_at` is shown nowhere.** The estimate is the session's, kept
  for no time (ADR-0165), so it is always just made.
- **No time zone or locale formatting** of an `Instant`.
- **That the least is at most the most.** The estimator answers it; no
  invariant relates two fields.
