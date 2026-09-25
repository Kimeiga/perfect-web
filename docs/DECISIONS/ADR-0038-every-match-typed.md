# ADR-0038: every match is analysed, and each pattern against its own type

Status: accepted under the owner's instruction of 2026-09-24 ("do all of this
until its finished and the project is perfect"). Decisions marked **(ruling
needed)** were made without a ruling and are offered for reversal.
Date: 2026-09-25. Milestone: E10.

## Context

ADR-0011 requires `pw` to reject an incomplete match "regardless of the
function's effect row", because Koka accepts one in a function that may raise
`exn`, and it fails at runtime. The kiokun slice's backend refused a match that
`pw check` had passed: `match get(w) { Some(x) => .. }`, with no `None` arm.

The analysis read only one kind of scrutinee: a parameter annotated with a sum
type the program declares. A match over a call, a field, a local, or any
`Option` or `Result` was not analysed. That is the gap the slice found.
Closing it found four more, and each made the analysis report a proof:

| found | what the analysis concluded, at `1e2d0de` |
|---|---|
| every pattern, at any depth, was read against the scrutinee's constructors | `match s { Circle(Draft) => .., Square => .. }` Proven: `Draft` is not `Shape`'s, so it read as a binding, and `Circle(Sent)` was covered |
| a constructor its type does not have read as a wildcard | `Circle(None) => ..` Proven, for the same reason |
| a parameter's declared type was read for a name an arm binds again | `Some(x) => match x { Red => 1 }`, with a parameter `x: Status`: Proven against `Status`, where `Red` read as a binding |
| `=> return e` parsed as an arm ending at `return`, then an arm whose pattern was `e`, with no `=>` and no error | any match with such an arm Proven: the phantom arm read as a wildcard |

The table was reproduced by running the same probe at `1e2d0de` and after the
change. Each row is now a test in
`compiler/pw-core/tests/match_exhaustiveness.rs`.

## Decision

### 1. A scrutinee is typed by the value relations

A bare name is read as the parameter only when no pattern in the body binds the
name again. Anything else is typed by the E9 value relations
(`values::type_of`), which already type calls. A name bound at two sites is
unknown to them, so a shadowed parameter blocks the analysis, with that
reason, instead of being read as the parameter. The typer now binds a match
arm's names: `Some(x)` binds the option's `T`, and `Ok(x)` and `Err(e)` bind
the result's two sides.

### 2. `Option` and `Result` are sum types to the analysis

Each is declared in the analysis's program with its two cases. Their payloads
are one opaque type, so a constructor pattern nested under `Some`, `Ok` or
`Err` blocks the analysis, with that reason, rather than being checked
against a type the analysis does not have. The backend refuses nested
patterns too (ADR-0036).

### 3. Each pattern is read against its own type

A nested pattern is read against its field's type, in the checker's lowering
and in `exhaust.rs`'s arity validator, which made the same assumption. A
literal pattern blocks the analysis: it covers one value of a type the
analysis does not enumerate.

### 4. A constructor its type lacks is an error: PW0608

`PATTERN_CONSTRUCTOR`, *a constructor pattern must name a constructor of the
type it matches*, owned by Types. It is reported where it occurs, and names
the constructors the type has. The analysis is blocked, never proven.

PW0603, a wrong field count, is now found at any depth too; it was checked
only at the top of an arm, and a nested one blocked the analysis without a
word. Each arm's faults are reported on their own, so an arm the analysis
cannot read no longer hides another arm's error.

**A bare name is a constructor if any type in the program has a constructor of
that name (ruling needed).** A nullary constructor has no parentheses, so
`Red` and a binding named `Red` are one token. The analysis already read a
bare name as a constructor of the scrutinee's type (`to_exhaust_pattern`'s
load-bearing case). It now reads the name as a constructor of whichever type
has one, and refuses it against any other type. The cost is that a binding
cannot share a constructor's name. The alternative is to accept the binding
and warn.

### 5. `return` is a statement, in an arm too

The grammar has no `return` expression. A block reads `return e` as two
statements, `return` and then the value (`values::result_sites`). This keeps
that reading and makes it hold everywhere:

- the expression parser stops after `return`, so `return (x)` is not a call
  of `return`, which was refused as unresolved, and `return -1` is not a
  subtraction, which was accepted silently;
- an arm whose body begins with `return` holds the value on the same line
  too, and the HIR reads the two as a block;
- an arm with no `=>` is an error (PW0001), where it had been accepted
  without a word.

**A `return` expression proper is not built (ruling needed on whether it
should be).** It would replace the two-statement reading in every consumer.

## Consequences

- The value relations now see a value returned from an arm: a `return "x"` in
  a function declaring `Int` is PW0606.
- The generality witness `affine_not_consumed_once/caught-match.pw` has the
  only `=> return` arm in the repository. Its match is now read as written.
- The E9 counts are unchanged for the store program and the accepted corpus.
  `pw check` is clean on the store, the kiokun slice and the accepted corpus.
- The backend's own refusal of a match that misses a case stays, as a second
  layer. No test reaches it now.

## Acceptance

- `compiler/pw-core/tests/match_exhaustiveness.rs`, 13 tests. They cover:
  - `Option`, `Result` and call scrutinees;
  - a foreign constructor, at the top level and nested;
  - a nested pattern read against its field's type;
  - what blocks: a literal, and a constructor under `Some`;
  - a shadowed parameter;
  - `=> return` refused for the case it misses, with its value checked;
  - a field count checked at any depth, and in every arm.
- `exhaust.rs`: a nested pattern is validated against its field's type.
- `pw-syntax`: `return` holds the value on its line, in an arm and in a block;
  an arm without `=>` is reported.
- The kiokun slice's two matches in `Lookup` are Proven.
