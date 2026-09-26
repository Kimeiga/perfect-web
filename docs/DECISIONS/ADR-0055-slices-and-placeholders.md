# ADR-0055: slicing, and the standard library's placeholders computed

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 2, the standard library).

## Context

KNOWN_LIMITATIONS: "There is no slicing ... `sum`, `maximum` and `enumerate`
still have placeholder bodies." A program could take a list's first elements
(`List.take`) but not its rest or a middle, and could not slice a string.
The three placeholders type-checked and computed wrong answers:
- `List.sum` answered 0.0;
- `List.maximum` answered 0.0;
- `List.enumerate` answered `[]` with no declared result type.

Three corpus programs called them:
- A-016 used `sum` and `maximum`;
- A-021 used `enumerate`, with a tuple pattern the language has no type for;
- R-035 used `maximum`.

## Decision

### 1. Slices

| operation | what it gives | how |
|---|---|---|
| `List.drop(items, count)` | all but the first `count` | a view |
| `List.slice(items, start, end)` | `start` up to, not including, `end` | a view |
| `String.slice(text, start, end)` | code points `start` up to `end` | a view |
| `List.reverse(items)` | the last element first | a copy |

- Each bound is clamped to `0..=length`. An `end` before the `start` gives
  the empty list or `""`, as `take`'s count is clamped (ADR-0040).
- A view is the same bytes, since values do not change.
- `String.slice` counts code points, as `String.length` does.
- In the component, `take`, `drop` and `slice` share one clamp. In the
  module, one `slice` function serves both slices and `drop`.

### 2. The placeholders are Pleris

`sum` and `maximum` are written in Pleris, in `list.pw`, and compile as any
declaration does:
- `sum` folds left to right from 0.0.
- `maximum` returns an `Option<Float>`: the largest element, or `None` for
  an empty list. A NaN is the maximum of any list that has one, as IEEE
  754's `maximum` and JavaScript's `Math.max` give.
  **(ruling needed)**: `Option` rather than negative infinity, which is
  `Math.max()`'s answer for no arguments.
- `enumerate` is removed. **(ruling needed)**: the language has no tuple
  type, so a position and an element cannot be one value. A `let mut`
  counter in a `for` loop (ADR-0051) counts positions.

The three corpus programs change with the library:
- A-016 matches `maximum`'s `None`. It passed an `Option<Float>` as a
  `Float`, which the checker did not see, since `derived` is not checked
  (ADR-0047).
- A-021 counts its positions.
- R-035 sums rather than takes the maximum. Its violation, a write inside
  the measure phase, is unchanged; with an `Option` it gained a second
  error beside its own.

## Acceptance

- `compiler/pw-conformance/tests/stdlib.rs`, through the E8 host against
  `Vec` and `str`:
  - `drop`, `slice`, `reverse` over `Int`s and over records;
  - `String.slice` over mixed scripts;
  - bounds at `i64::MIN`, `i64::MAX`, negative and past the end;
  - `sum` and `maximum` against Rust's folds, with the NaN cases and the
    empty list.
- `compiler/pw-conformance/tests/javascript.rs`: six queries more. The
  component and the module agree on 59 queries and 11,800 calls.
- The accepted corpus, the store and kiokun check clean; R-035 reports its
  rule alone.
- Mutation controls: `scripts/slice_mutations.py`, `just e10-slices`,
  10 mutants, in the component, the module and `list.pw`.

## Not done

- **`String.split`, `List.range` and an `Int` sum** are not in the library.
- **Unicode case mapping**, and **maps and sets**, are the next parts of the
  same instruction.
