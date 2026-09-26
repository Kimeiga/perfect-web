# ADR-0068: what each construct takes, checked

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, the value checker; §14 M10).

## Context

A probe for wrong programs that pass `pw check`, run after ADR-0067, found
constructs whose operands were related to nothing. On 2026-09-26 each of
these checked:
- **A call through a function value.** In `g("a")`, where `g` is a
  `fn(Int) -> Int` bound by a `let` or a parameter, nothing was checked:
  not the arguments, not their number, not the result. ADR-0052 made a
  function a value that compiles, and the value relations never typed a call
  through one.
- **A value called that is not a function:** `k(n)`, where `k` is an `Int`.
- **A local function value named like a declaration.** It was checked
  against the declaration, because a call's path was resolved as the
  program's own first. A correct call to the local was refused (PW0605).
- **`for` over a value that is not a list:** `for x in n`, where `n` is an
  `Int`.
- **`?` on a value with no failure:** `b?`, where `b` is a `Bool`.
- **An `if` or a `match` whose branches produce two types, where its value
  is used:** `let x = if c { 1 } else { "a" }`.

**The last found a silent miscompile.** `if a { x } elif b { y } else { z }`,
and `else if` the same, is one syntax node holding `a`, `{ x }`, `b`, `{ y }`
and `{ z }`. The HIR lowering took its third child for the `else`, so every
chain lowered to `if a { x } else b`: when `a` was false the value was the
next condition, and `{ y }` and `{ z }` were never run. A chain of `Bool`s
checked, compiled and answered wrong. `Pick(0)`, which should be `true`, was
`false`. No compiled program in the repository wrote a chain, and A-018's,
in a derived value, is where the new relation found it.

**It also found two ill-typed programs, and four ill-typed witnesses:**
- A-005 applied `?` to `Menu.is_available`, which answers a `Bool`.
- Four generality witnesses joined a `String` branch with a `Secret`, a
  `Result` or a record: `if dry_run { "none" } else { key }`.

Each witness now interpolates the other branch's value (`"{key}"`), which
keeps its label, the witness's subject. A-005 drops the `?`.

## Decision

### 1. A call through a binding is a call through its value

A call whose callee names a binding in scope is a call through the value it
holds, whatever declaration shares its name. It is checked against the
value's function type as a declared callee's call is:
- its arity (PW0604);
- each argument (PW0605), a lambda typed against its parameter's function
  type;
- its result, which is the function's.

A value of another type called is PW0614, "a value called must be a
function". A value whose type is unknown relates nothing, and one of any type
is a function of anything.

### 2. `for` runs over a list, and `?` takes an `Option` or a `Result`

Each is an operand relation (PW0609): `for`'s iterable against `List<_>`, as
ADR-0051 compiles it, and `?`'s operand against `Option<_>` or
`Result<_, _>`.

### 3. The branches of a used `if` or `match` produce one type (PW0613)

An `if` with `else`, or a `match`, whose value is used relates its branches
to each other. It is used when it is:
- bound;
- passed;
- operated on;
- read as a condition, a scrutinee or a part.

A statement's branches are not related, since its value is discarded, and
neither are a body's result's, which the result relation reads branch by
branch. A lambda's body is not read here either: it is its function's
result, which its use relates. **(ruling needed)**: the alternative is to
relate every `if` and `match`'s branches, as a language without statements
would.

### 4. A chain is nested ifs

An `elif` or `else if` chain lowers to an `if` whose `else` is the next `if`,
to the last `else`.

## Acceptance

- **`compiler/pw-core/tests/calls_and_branches.rs`**, 7 tests:
  - a call through a function value: its arguments, arity and result;
  - a value that is not a function;
  - a local function value named like a declaration;
  - `for` over a non-list;
  - `?` on a `Bool`;
  - branches bound by a `let`, and a statement's, not related;
  - an `elif` chain.

  All 7 fail at 36af280, the commit before.
- **`compiler/pw-conformance/tests/if_chains.rs`**, 3 tests through the E8
  host:
  - `elif` and `else if` chains of `Bool`s;
  - a chain of four branches;
  - a chain as a statement.

  Each fails at 36af280. The `Bool` chains check and compile there, and
  answer wrong.
- **No other rejected, rule or generality fixture's diagnostics change,**
  apart from the four witnesses' correction. The store, kiokun and the
  accepted corpus check clean. Their audits gain only agreeing relations for
  `for`, `?` and branches, and one undecided branch relation in the corpus.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/call_branch_mutations.py`,
  `just e10-calls`, 10 mutants. ADR-0043's control for an `if`'s condition is
  re-anchored where this moved its code.

## Not done

- **A lambda with no annotation, called directly:** `let f = (x) => x + 1`
  then `f("a")`. Its parameter's type comes from no use (ADR-0052's
  limitation), so the call relates nothing.
