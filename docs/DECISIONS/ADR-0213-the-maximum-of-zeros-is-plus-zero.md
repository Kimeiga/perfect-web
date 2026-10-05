# ADR-0213: `List.maximum` gives +0 over −0

Status: accepted under the owner's delegation of 2026-10-02. It fixes
ADR-0210's urgent defect 7, under ruling 0055-a (confirmed: `maximum`
returns an `Option`). Date: 2026-10-05. Milestone: E14.

## Context

- **`List.maximum` kept the first of +0 and −0.** `−0 == +0` is true, so
  neither is `>` the other, and `[-0.0, 0.0]` answered −0.
- **IEEE 754-2019's `maximum` orders −0 below +0**, and ECMA-262's
  `Math.max(-0, 0)` is +0. A NaN is already the maximum of any list holding
  one.
- **Its test could not see it.** It compared the answer with `==`, under
  which the two zeros are equal, and its random lists rarely held +0.

## Decision

- **+0 is larger than −0.** Between two equal values, `x` is taken when
  `1.0 / x > 1.0 / b`: that is +∞ for +0 and −∞ for −0. Float division is
  IEEE's in both backends (`f64.div`, and JavaScript's `/`), so it does not
  trap. Two −0s are −0.

## Acceptance

Recorded by `just e14-float-maximum` in
`docs/evidence/E14/float-maximum.txt`:

- **`compiler/pw-conformance/tests/stdlib.rs`**: its model is IEEE's
  `maximum`, compared by bits. Five explicit lists of zeros are added, which
  fail before.
- **The JavaScript backend agrees, by bits** (`tests/javascript.rs`).
- **`scripts/float_maximum_mutations.py`**: 2 mutants.
  `scripts/slice_mutations.py`'s two controls of `maximum` are re-anchored.
