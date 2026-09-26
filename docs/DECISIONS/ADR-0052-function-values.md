# ADR-0052: a function is a value

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 1, own backends).

## Context

ADR-0040 §3 compiled a function argument where a standard-library list
operation runs it: a lambda, or a declaration's name, inlined into the loop.
Anywhere else a function was refused by name:
- stored in a binding;
- returned;
- passed to a declaration of the program's own.

KNOWN_LIMITATIONS listed it ("A function is not a value"). ADR-0031 §4 had
given the checker function types, `fn(A) -> R`, and the corpus's standard
library declares them.

## Decision

### 1. The IR

- `Type::Function(params, result)`: a function value. It never crosses the
  component boundary, since WIT has no function type.
- `Instr::Closure { index, captures }`: a function value. It names its code,
  one of the export's `closures`, and carries the values it captured.
- `Instr::Apply { function, function_ty, args }`: a call through a function
  value.

### 2. What a closure is

A lambda used as a value compiles to a function of its captures, then its
parameters:
- **Captures** are the names its body reads that a binding holds where it
  is made, as it holds them then. A `let mut` binding is captured as a copy,
  and a closure cannot assign one.
- **Parameter types** come from where the lambda is used: a `let`
  annotation, or the parameter it is passed as. A lambda whose use fixes
  none is refused by name.
- A **declaration's name**, where a function is wanted, is its body compiled
  as such a function, with nothing captured, at the instance the wanted type
  gives it. A standard-library or host operation is refused as a value.

A binding's written type is now what its initialiser is lowered against. It
was read by the checker and ignored by the backend.

**(ruling needed)**: a closure captures by value. The language's values are
immutable, so the one difference from capture by reference is a `let mut`
binding assigned after the closure is made, which the closure does not
see.

### 3. In the component

- A function value is the address of an environment in the invocation
  region.
- The environment's first word is the code's slot in a `funcref` table.
- The captures follow, in their canonical layouts.
- A closure's code is a function taking the environment first, then its
  parameters and result passed as a callee's are (ADR-0050).
- A call through a value is `call_indirect` through the slot the
  environment names, at the core type of the value's function type.

The table is the module's own, and wit-component carries it inside the
component.

### 4. In the JavaScript module

A closure's code is a module-level function of its captures and its
parameters. The value is an arrow function that passes what it captured and
what it is given. A call through a value is a JavaScript call.

## Acceptance

- `compiler/pw-conformance/tests/function_values.rs`, through the E8 host
  against Rust:
  - a lambda passed to a declaration, and applied twice;
  - a closure capturing an `Int`, and one capturing a `String`;
  - closures returned by a declaration and composed;
  - a declaration's name as a value;
  - a closure chosen by an `if`;
  - a recursion carrying a function.
- `compiler/pw-conformance/tests/javascript.rs`: four closure queries more.
  The component and the module agree on 49 queries and 9,800 calls.
- `compiler/pw-conformance/tests/stdlib.rs`: the function passed to a
  declaration that it refused now runs.
- The store's and kiokun's 19 artifacts stay byte-identical to `fd95b59`'s.
- Mutation controls: `scripts/function_value_mutations.py`,
  `just e10-function-values`, 7 mutants.

## Not done

- **A function value read from a record field is not called.** `r.check(5)`
  reads as a method call, and no declaration named `check` takes the record.
- **A function value crossing the boundary.** A command or query cannot
  take or return one, and a host cannot give one.
