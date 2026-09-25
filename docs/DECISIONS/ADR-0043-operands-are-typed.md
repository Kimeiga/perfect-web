# ADR-0043: an operator's operands are typed, and `Float.from_int`

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10.

## Context

KNOWN_LIMITATIONS recorded a gap in the checker: "`1 == "a"` types as a
`Bool` and passes `pw check`; the backend refuses it." The E9 value relations
typed a comparison as `Bool` whatever it compared. An arithmetic operator's
result had no type unless both operands were one numeric type, and a mixture
was silently unknown. Nothing related an operand to the type its operator
takes, or an `if`'s condition to `Bool`.

ADR-0039 §2 already decided the language's rule: "Operands of two different
types are refused". Only the component backend enforced it, and only for
code it compiled.

## Decision

### 1. A relation for operands (PW0609)

The value relations gain `Operand`, reported as PW0609, a new code: "an
operator's operands, and a condition, must have the types they take".

| where | what it takes |
|---|---|
| `==` `!=` `<` `<=` `>` `>=` | a right side of the left side's type |
| `+` `-` `*` `/` `%` | an `Int` or a `Float` on the left, and a right side of its type |
| `&` `\|` | `Bool` on both sides |
| `!` | a `Bool` |
| unary `-` | an `Int` or a `Float` |
| `if c` | a `Bool` condition |

As every value relation does, it reports only what the program's types
decide. An operand of unknown type is undecided, counted by
`pw audit-values` and never refused. Which operators a type supports beyond
this table is the backend's to refuse. `==` on two records, for example, is
not a type disagreement.

### 2. `Float.from_int`

Arithmetic on a count and a measurement needs a conversion, and the language
has no implicit one. `packages/pw-std/float.pw` declares the `Float` module
with `from_int(n: Int) -> Float`, an intrinsic. The component backend compiles
it to `f64.convert_i64_s`: exact up to 2^53 in magnitude, then the nearest
`Float`, ties to even, as IEEE 754 and Rust's `as f64` convert. The name is
**(ruling needed)**; `to_float` and a method on `Int` were the alternatives.

## Found while building it

**Two accepted fixtures divided a `Float` by an `Int`.**
- A-017 wrote `self.content_width() / badges.length()`.
- A-021 wrote `width / values.length()`, and also `i * step` with an `Int`
  index. PW0609 could not see the second, because `enumerate` has a
  placeholder body and `i` has no type.

Both programs are refused under ADR-0039 §2, and both passed `pw check` as
accepted corpus. Their subjects are layout batching and custom paint, not
arithmetic, so the arithmetic is repaired with `Float.from_int`, with a
comment in each. The fixtures' claims are unchanged.

## Acceptance

- `compiler/pw-core/tests/value_relations.rs`:
  - each operator refuses the types it does not take, and takes the ones it
    does;
  - an untyped operand is undecided, not refused.
- `compiler/pw-conformance/tests/stdlib.rs`: `Float.from_int` agrees with
  Rust's `as f64` at 0, ±1, the `Int` bounds, around 2^53, and on generated
  values.
- `compiler/pw-conformance/tests/computation.rs`: `a == b` on an `Int` and a
  `String` is refused by `pw check` (PW0609), before the backend sees it.
- The accepted corpus checks clean, and disagrees nowhere in the value
  relations.
- Mutation controls: `scripts/operand_mutations.py`, `just e10-operands`.
