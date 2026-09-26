# ADR-0082: a `derived` value performs no effect

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.5, "no
universal lifecycle effect").

## Context

The charter lists `derived` among its primitives as "a pure value computed
from other values" (§7.5), and `docs/SEMANTICS.md` repeats it. A derived
value is recomputed whenever what it reads changes, so an effect inside one
would run an unknown number of times. ADR-0047 made `derived e` parse as a
value and recorded, under "Not done": "`derived`'s purity is not checked".

On 2026-09-26, at a5d4785, `let t = derived clock.now()` checked, in a
declaration whose row allows the clock. The row check asks only whether the
declaration may perform the effect, not whether the `derived` value may.

## Decision

**A `derived` value performs no effect** (PW0334, "a `derived` value is
computed from other values, and performs no effect"). Everything the value
performs counts: its calls, the members it reads, and the functions it
names (ADR-0078). An effect beside it in the same body is the body's. The
diagnostic names the effect and the route it took.

## Acceptance

- **`compiler/pw-core/tests/derived_purity.rs`**, 2 tests, each with
  controls:
  - a clock read;
  - a function named as a value, and one written in place;
  - a geometry read through a member.
  Both fail at a5d4785, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  accepted corpus's `derived` values (A-016, A-018) are pure and check
  clean, as do the store and kiokun.
- **Mutation controls:** `scripts/derived_mutations.py`, `just e10-derived`,
  3 mutants.
