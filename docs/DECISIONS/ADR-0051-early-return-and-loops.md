# ADR-0051: an early `return`, `?`, and `for` loops compile

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 1, own backends).

## Context

The component backend (ADR-0039) and the JavaScript modules (ADR-0044)
refused three things by name:
- an early `return`, because a body's value was its last expression;
- `?`;
- a `for` loop, and an assignment, because a compiled body bound each name
  once.

The checker accepted `x = e` whatever `x` was, and whatever `e`'s type:
- a parameter could be assigned;
- so could a `let` without `mut`;
- `x = "a"` passed for an `Int` `x`.

## Decision

### 1. The IR

Four instructions and one loop kind:
- **`Return`**: leaves the function with a value. What follows in its region
  never runs. Its own result is a placeholder of the type the region needs,
  never read.
- **`Local`**, **`Set`** and **`Get`**: a `let mut` binding. `Get` is a copy,
  so a value read before an assignment keeps what it held.
- **`EachKind::For`**: the body runs once per element, and its value is
  discarded.

A branch or match arm that returns takes its siblings' type. An `if` with no
`else` compiles as a statement, whose value is the unit value.

### 2. `return` leaves the function it is written in

In the component:
- an export's `return` puts its result where the world's signature takes it,
  as its end does, then returns;
- a callee compiled beside the export (ADR-0050) returns the way it passes
  its result.

In the JavaScript module a `return` is a JavaScript `return`, and a `for`
loop is a `for..of` in the function's own body.

A callee with a `return` or a `?` is compiled beside its export and called,
never inlined, since an inlined `return` would leave the caller.

A lambda that a list operation runs is compiled into its loop. So a
`return`, a `?`, or an assignment inside one is refused by name, since it
would leave or change the function around it. The checker already reads a
`return` in a lambda as the lambda's.

### 3. `e?`

`e?` is a match. The success arm is the payload. The failure arm returns
`None`, or `Err` with the same payload. `Result`'s error type must be the
function's.

### 4. What the checker says about assignment

- **PW0611**: an assignment's target must be a `let mut` binding. The
  innermost binding of the name decides. A parameter, a loop's or a
  pattern's name, and a module-level `let` are refused. A module-level
  `let mut` may be assigned; `Decl::mutable` records it.
- **PW0607**: `x = e` relates `e` to the type `x` holds, as a new
  `Assignment` relation.

**(ruling needed)**: `for` over a list is the only loop. The language has no
`while`, and recursion (ADR-0050) covers what a condition loop would.

## Acceptance

- `compiler/pw-conformance/tests/control_flow.rs`, through the E8 host
  against Rust:
  - guard returns;
  - a `return` from inside a loop;
  - `mut` accumulators in one loop and in two nested loops;
  - a string built in a loop;
  - `?` on a `Result` and on an `Option`;
  - a callee that returns early, called and passed to `List.map`;
  - a bound `if` and a bound `match` with a returning branch;
  - a read kept across a later assignment.
- `compiler/pw-conformance/tests/javascript.rs`: six such queries more. The
  component and the module agree on 45 queries and 9,000 calls.
- `compiler/pw-conformance/tests/computation.rs`: the early `return` it
  refused runs; a `return` and an assignment inside a lambda are refused by
  name.
- `compiler/pw-core/tests/assignments.rs`: 6 tests of PW0611 and PW0607.
- Every fixture reports what it did before this change, against a build of
  `995daaf`.
- Mutation controls: `scripts/control_flow_mutations.py`,
  `just e10-control-flow`, 13 mutants.

## Not done

- **A function value** is still refused. It is the next part of the same
  instruction.
- **An assignment to a field** (`p.x = 1`) is refused by the backend. A
  record is a value; a new one, assigned to a `let mut` binding, is the
  change.
