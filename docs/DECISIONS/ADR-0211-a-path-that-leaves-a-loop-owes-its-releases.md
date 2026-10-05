# ADR-0211: a path that leaves a loop's body leaves the function, and owes its releases

Status: accepted under the owner's delegation of 2026-10-02. It builds
ruling 0045-a, parts 1 and 2, of ADR-0210, urgent defect 1. Date:
2026-10-05. Milestone: E14.

## Context

- **PW2005 says an affine value is released exactly once on every path**
  (2026-09-25). A `for` loop's flow had no exits, so a path that left the
  function from inside its body was not one:
  - `let tx = Database.begin(); for s in sessions { if s == "" { return Ok(()) } }; tx.commit()`
    checked, and the transaction stayed open on that `return`;
  - so did a failing `?` in the body, `Carts.clear(current_session())?`.
- **The correct program was refused.** Rolling back and then returning,
  inside the loop, is one release on a path that leaves. The loop counted
  it as a release inside a loop, which may run any number of times.
- **Rust's borrow checker is flow-sensitive** in the same way: a value moved
  on a path that returns is not moved on the paths that continue.

## Decision

- **A `return` or a failing `?` inside a `for` body is an exit**, owing one
  release of each live affine value, as one outside a loop is (part 1).
- **A release inside a `for` body is accepted when every path from it leaves
  before the pass ends** (part 2). One on a path that goes round again runs
  again, on the next pass or after the loop, and PW2005 stays.
- **The iterable is evaluated once**, before the passes.
- **A function value is unchanged** (part 3): its `return` leaves the
  function value, so its body has no exits here, and a release in it may run
  any number of times.

## Acceptance

Recorded by `just e14-affine-loops` in `docs/evidence/E14/affine-loops.txt`:

- **`compiler/pw-core/tests/affine_loops.rs`**, 6 tests:
  - a `return` and a failing `?` inside a loop owe the release;
  - a release then a return inside a loop runs once, in a nested loop too;
  - a release on a path that goes round again is refused;
  - an exit inside a nested loop owes it too;
  - a loop that leaves nothing open is unchanged.
  Four fail at the commit before. Two are controls.
- **`scripts/affine_loops_mutations.py`**: 3 mutants.
  `scripts/affine_mutations.py`'s "a release in a loop counts once" is
  re-anchored where the release is found, for loops and function values.
- **The corpus checks as before.**

## Not claimed

- **`while`** (ruling 0051-a) is not built. When it is, its body is a pass
  as a `for`'s is.
