# ADR-0065: a value that holds at every type

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, the value checker; §14 M10).

## Context

After ADR-0063, each of kiokun's 20 undecided value relations was one kind: a
value with a part no declaration states, typed as a hole the relations could
not decide against:
- `None`, which was an `Option` of something unstated;
- `[]`;
- `Err(e)`, whose success is unstated, and `Ok(x)`, whose failure is;
- `todo`;
- the early return of `e?`;
- a generic value whose parameter nothing fixes: `Maybe.Nothing`, `Either.Left(1)`'s
  `B`, or `Secret("")`'s phantom `C`.

So `fn f() -> Option<Int> { None }` was undecided, as was every relation
through such a value: 41 of the store's relations, and 70 of the accepted
corpus's. Each of these values is well-typed at every type its hole could be.

The hole was `Ty::Unknown`, which means "the program does not say".
`None`'s hole is not that. The program says `None` is an `Option` of
anything.

## Decision

### 1. `Ty::Any`: a part that holds at every type

`Ty::Any` agrees with every type, where `Ty::Unknown` decides nothing. It is
given to:
- `None`'s payload, `[]`'s element, and `Ok`'s failure and `Err`'s success;
- `todo`, which never returns;
- the success an early return (`e?`) returns with;
- a construction's type parameter that none of its fields mentions. This
  covers a case without a payload, `Maybe.Nothing`, a case whose fields leave
  one out, `Either.Left(1)`'s `B`, and an opaque type's phantom parameter,
  `Secret("")`'s `C`.

It is displayed as `_`.

### 2. It fixes no variable

A variable a value of any type meets is left free, and fixed by what else it
meets: `List.concat([], ["a"])` is a `List<String>` whichever list comes
first. A variable only such values meet closes to `Any`: `List.get([], 0)`
is an `Option` of anything.

### 3. Never a function's link

A generic function named as a value keeps its parameters unknown, as before:
`Maybe.Just` is a function from a `T` to a `Maybe<T>`. Made `Any`, its two
`T`s would come apart, and `List.map([1], Maybe.Just)` would agree with a
`List<Maybe<String>>`. For the same reason a declared function's result
that its arguments do not fix stays unknown: a host function
`fn decode<T>(raw: Unknown) -> T` answers at no type its arguments fix.
**(ruling needed)**: the alternative, from parametricity, is to make it `Any`
too, which is sound for a function written in Pleris and not for one the host
supplies.

### 4. A mutable binding holds one type

A `let mut` initialised with such a value is incomplete, not polymorphic:
`let mut xs = []` holds what is assigned to it. An assignment completes a
mutable binding's incomplete type, so `let mut xs = []` then `xs = [n]` is a
`List<Int>`, and returning it as a `List<String>` is refused (PW0606). A
`let` without `mut` keeps `Any`, since its value cannot change.

### 5. Where it meets the rest

- **An operator's operand of any type** is the number the other side is.
- **The exhaustiveness analysis** reads a scrutinee or a payload of any type
  as unknown, as before.

## Acceptance

- **`compiler/pw-core/tests/any_type.rs`**, 10 tests:
  - `None`;
  - `[]`, through a generic call and as an operand;
  - `Err`;
  - `todo`;
  - `Maybe.Nothing`;
  - a `let mut`, refused and accepted;
  - a variable a value of any type meets, whichever side it is on;
  - a case passed as a function, which keeps its link;
  - a parameter no field mentions: a case, a record built by field names,
    and an opaque type's phantom;
  - an early return.

  Each wrong program is refused. Nine fail at 81a3164, the commit before.
  The tenth is the guard that a function's link is kept, and holds before
  and after.
- **Every relation decided before is decided the same after,** relation by
  relation. Undecided relations fall from 41 to 1 in the store, from 20 to 1
  in kiokun, and from 70 to 18 in the accepted corpus. What remains is other
  constructs:
  - a `measure { .. }` block;
  - a dimensioned literal (`8.px`);
  - a `derived` value;
  - a painter's context;
  - `Money.add` named as a function;
  - a handle from a `use` of a host call;
  - a type named as a value.
- **No rejected, rule or generality fixture's diagnostics change.**
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/any_type_mutations.py`, `just e10-any`,
  14 mutants.

## Not done

- The constructs above, each undecided for its own reason.
- A generic function's result its arguments do not fix (§3).
