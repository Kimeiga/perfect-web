# ADR-0060: nested and literal patterns

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, "exhaustive pattern matching"; §14 M10, compiled pure
computation).

## Context

KNOWN_LIMITATIONS said:
- **Some matches are not analysed.** A match was counted Blocked, neither
  proven nor refused, when it had:
  - a constructor pattern nested under `Some`, `Ok` or `Err`
    (`Some(Some(x))`);
  - a literal pattern (`Some("a")`);
  - a scrutinee the value relations could not type.
- **The backend refuses nested patterns** by name.

A `Bool`, an `Int` or a `String` could not be matched at all. `match b {
true => 1 }` was Blocked by the checker and refused by the backend.

Writing this found three more gaps:
- **A case named alone under another was a binding.** Take `match o {
  Some(Empty) => 1, None => 0 }` over an `Option<Shape>`:
  - the analysis was Blocked, since the payload of `Some` was opaque to it;
  - the backend read `Empty` as a binding of that name, which took every
    payload;
  - so the program checked, compiled, and answered 1 for `Some(Circle(3))`.

  This was a silent miscompile. It was present since ADR-0036 compiled
  matches, for any case name written under `Some`, `Ok` or `Err`, and
  ADR-0059 extended it to declared cases.
- **`-1` was a parse error in a pattern**, so a match could not name a
  negative number.
- **A declared case's field was read by its spelling.** For
  `Full(Option<Int>)`, `Option<Int>` named no declaration, so any pattern
  under `Full` was unread.

## Decision

### 1. The analysis types each match's subject

- **Per match, an ADT per instance.** Each type a pattern can take apart
  is declared as an ADT of its own instance, with every constructor's
  fields typed: `Option<Option<Int>>`, `Result<Option<Shape>, String>`,
  `Maybe<Int>`. A declared case's fields come from `Signatures`, resolved
  and under the arguments the type is applied to, not from their spelling.
- **`Bool` has two constructors,** `false` and `true`, and a pattern's
  `true` and `false` are them.
- **An `Int` or `String` literal is a constructor of its type's own.** No
  list of literals completes the type, so a match over one needs an arm
  taking the rest. Its missing value is shown as `_`.
- **A literal of another type is PW0608.** Its repair says a pattern
  against an `Int` is one of its literals, or `_`.
- **A type no pattern takes apart is matched by a name only.** A record,
  a list or a `Float` is known and has no constructors, so `_` or a name
  covers it, and a constructor or a literal against it is PW0608: `Some(x)`
  over a `List<String>` passed `pw check` Blocked, and was refused by the
  backend.
- **Still unread,** each Blocked with its reason:
  - a `Float` literal, since equality on floats decides no case;
  - a scrutinee of a type the program does not state.
- **A missing value names a field by its type:** `Err(String)`,
  `Some(Circle(Int))`, as a declared case's witness always did.
- **The shared environment keeps only the constructor names.** A bare name
  in a pattern that some type has as a constructor is that constructor
  (ADR-0038).

### 2. A negative literal is a pattern

`-1` and `-0.5` parse as literal patterns. A `Float` one is refused by the
backend (below).

### 3. What a pattern binds is typed

The value relations bind every name a pattern binds, at any depth. Each is
typed from the part of the scrutinee's type it is read against. A name
alone binds the whole value, unless it names a case: `other =>` has the
scrutinee's type, where it was unknown.

### 4. The backend compiles a decision tree

- **One level deep stays as it was.** A match over a variant whose arms are
  one level deep keeps ADR-0059's compilation: one `Instr::Match`, each
  arm's body lowered once. The store's and kiokun's artifacts are
  unchanged.
- **Every other match compiles to a decision tree.** That is a match with:
  - a pattern nested in another;
  - a case named alone under another (`Some(Empty)`);
  - a literal;
  - a `Bool`, `Int` or `String` scrutinee.
- **Each node tests one value**, the first the first remaining arm tests,
  left to right:
  - a variant, as an `Instr::Match`, with an arm per case the arms name
    (its fields fresh values, tested next) and one arm for the rest;
  - a `Bool`, as an `Instr::If`;
  - an `Int` or a `String`, as `if v == literal` for each literal the arms
    name, in their order, then the rest.
- **A leaf is the first arm every test on its path agrees with.** Its body
  is lowered there, with the names its path bound.
- **An arm reached on several paths is lowered on each.** The IR's regions
  are structured and have no jumps, so there is no join point to share.
  **(ruling needed)**: code size for a match that duplicates an arm, against
  a jump the IR would need. A tree of more than 1,024 leaves is refused by
  name.
- **A path no arm takes is refused,** never compiled to a trap.
- No new instruction: a tree is nested `Match` and `If`, which both
  encoders already compile.
- **Refused by name:**
  - a `Float` literal pattern;
  - an or-pattern that binds a name, which each alternative would bind
    from a different place.

## Acceptance

- **`compiler/pw-core/tests/patterns.rs`**, 7 tests, each with a control:
  - a case nested in an option;
  - an option nested in a declared case;
  - a `Bool`;
  - an `Int`'s literals;
  - a literal of another type;
  - a nested name typed in its arm;
  - a name alone typed as the whole value.
- **`compiler/pw-conformance/tests/patterns.rs`**, 10 tests through the E8
  host against a model in Rust:
  - nested options;
  - a case and a literal inside an option;
  - `Some(Empty)` alone, the miscompile's shape;
  - a `Bool`;
  - an `Int` with negative literals and an or-pattern;
  - a `String` with a binding for the rest;
  - a literal arm falling through to the next;
  - a result of an option of a case, three deep;
  - the two refusals.
- **`compiler/pw-conformance/tests/javascript.rs`:** five new queries. All
  84 queries' component and module agree on 16,800 generated calls.
- **Tests whose premise changed**, updated:
  - `match_exhaustiveness.rs`: a nested pattern and a literal are
    analysed, a record is matched by a name only, and `Err`'s witness shows
    `Err(String)`;
  - `evidence_reachability.rs`: the Blocked example is a `Float` literal,
    where it was an `Int` one;
  - `variants.rs`: a nested pattern that misses a case is the checker's
    PW0305, and `Some(x)` over a list the checker's PW0608, where both were
    the backend's refusals;
  - `checking_source.rs`: the shared environment is its constructor
    names.
- **The store's and kiokun's artifacts are byte-identical** to the build
  before this change, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/pattern_mutations.py`, `just
  e10-patterns`, 11 mutants.
  - ADR-0038's four controls in `scripts/match_mutations.py`, and two of
    ADR-0059's, are re-anchored where this moved their code.

## Not done

- A `Float` literal pattern.
- An or-pattern that binds a name.
- Reporting an arm no value reaches, in the checker (ADR-0059's ruling).
- A nested or literal pattern in a template's `{#match}`, which takes
  `Option` and `Result` apart only (ADR-0042).
