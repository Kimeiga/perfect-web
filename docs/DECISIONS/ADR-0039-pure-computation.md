# ADR-0039: pure computation in the component backend

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"), which follows the report naming the integer rulings this needs.
Decisions marked **(ruling needed)** were made without a ruling and are offered
for reversal. Date: 2026-09-25. Milestone: E10 (charter §14 M10 task 2).

## Context

After ADR-0036 the component backend compiled calls to host operations,
matches over `Option` and `Result`, field reads and the language's own cases.
Everything a program *computes* was refused: arithmetic, comparisons, `if`,
string constants, a call to another Pleris declaration, a record built.
`docs/NEXT.md` puts this first, because it is what keeps kiokun.com's shard
rule and ranking in the host rather than in Pleris.

## Decision

### 1. `Int` is a 64-bit integer that never wraps (ruling needed)

- `+`, `-`, `*` and unary `-` stop the invocation with a trap when the exact
  result does not fit in 64 bits.
- `/` and `%` are **Euclidean**, as Koka's `int` is: the remainder is in
  `[0, |b|)` and `a == b * (a / b) + a % b`. `-7 / 2` is `-4` and `-7 % 2`
  is `1`, measured against Koka 3.2.3.
- Dividing by zero traps. Koka answers `0` for `x / 0` and `x` for `x % 0`;
  Pleris refuses to produce a value there, because a wrong answer that looks
  like a right one is the failure this project exists to remove.
- `MIN / -1` traps: its exact result does not fit.

So a Pleris `Int` agrees with Koka's unbounded `int` wherever it produces a
value, and the Koka oracle can check every case whose divisor is not zero.
A trap is reported by the host as a failed call, never as a value.

`Float` is IEEE 754 binary64. `%` on `Float` is refused until its semantics
are decided; Wasm has no instruction for it.

### 2. Comparisons, logic and `if`

- `==` and `!=` compare two values of one primitive type. `<`, `<=`, `>`
  and `>=` order `Int`, `Float` and `String`; strings are ordered by their
  UTF-8 bytes, which is code point order. Operands of two different types are
  refused.
- `&` and `|`, the language's logical operators, evaluate their right side
  only when it decides the result.
- `if c { a } else { b }` is a structured instruction with two regions, as a
  `match` is (ADR-0036). An `if` without `else` has no value and is refused
  where a value is needed.

### 3. Strings

A string is UTF-8, held as a pointer and a byte length, the way the Canonical
ABI flattens it. A literal is placed in a data segment below the invocation
region. An interpolated string, `"han-{n}"`, is its pieces concatenated into
the region: a `String` part as it is, an `Int` part in decimal, a `Bool` part
as `true` or `false`. A `Float` part is refused until its formatting is
decided.

### 4. A call to another declaration is inlined

The component exports one function (ADR-0032). A call to a Pleris declaration
with a body and no `host` binding is lowered by lowering the callee's body with
its parameters bound to the arguments. So:

- recursion, direct or through other declarations, is refused by name: an
  inlined recursion has no end the compiler can see;
- a generic declaration is refused, until specialization exists;
- the code-size cost is measured, not assumed away (`just e10-bench`).

Internal functions with their own calling convention are the alternative. They
need an ABI for values that never cross the boundary, and they are deferred
until code size or recursion needs them.

### 5. Types that do not cross the boundary

The encoder types every value by a WIT type, and until now every one came from
the world. A value made inside a body may have a type the world never
mentions: `List<Int>`, or a record only a helper uses. The encoder maps an IR
type onto the world's own type when the declaration crosses the boundary, and
otherwise onto a definition it adds to a private copy of the world's
`Resolve`. Every layout still comes from `wit-parser`'s `SizeAlign`, and a
value that crosses the boundary still has the world's type, compared by
`same_type`.

### 6. Records are built

`Store { id: 1 }` allocates the record's canonical layout in the region and
stores each field. A declared *variant* is still refused, as is a match over
one.

## Found while building it

1. **A query's body could lose its statements without an error reaching the
   backend.** A query's body is where its policies live, so the parser read a
   bare name at the start of a line as a policy's head unless `.`, `(`, `<`,
   and a few other tokens followed. `{ a + b }` was an unknown policy `a` with
   the value `+ b`: the parser reported PW0005, the body kept no statement,
   and nothing else noticed. Now an operator after the name makes it an
   operand.
2. **The backend compiled programs that did not parse.** `Checked::of`, the
   proof a program checked, ran the checker on the units' trees, and a `Unit`
   carries its source and not its parse errors. So the tree that survived a
   syntax error was compiled. `pw build` refuses unparsed files before this,
   so the committed builds were never affected; the library path was. It now
   reads the parse errors again and refuses them, under a new `Parser`
   detector.
3. **The Koka backend negated with `-`.** Koka reads `(-a` as the start of an
   operator section and refused the generated module. `~` is Koka's negation.
4. **An import called only inside a `match` arm had no core import.** The
   encoder read the top of a body for the imports it needed. No committed
   program hit it: kiokun's `Lookup` calls `Entries.get` at the top first.
5. **The IR's record table typed every non-primitive field as a `String`.**
   `redirect: Option<String>` was a `String`. Nothing read the table. The
   encoder reads it now, for records the world never names, so it is built
   from the resolved signatures.

## Consequences

- The Koka backend already covers the pure subset's arithmetic, comparisons,
  `if` and `match` (ADR-0015). It is the oracle for these semantics, run by
  `just e10-pure`, because CI has no Koka.
- Traps are `unreachable` or Wasm's own integer traps. The host reports
  either as a failed invocation. They are not yet distinguished by cause.

## Acceptance

- Each construct has a test that runs it through the E8 host, and a refusal
  test for what stays outside.
- Mutation controls: undoing each piece fails a test.
- The differential oracle compares every compiled pure declaration with an
  independent Rust reference over generated inputs, and the Koka oracle
  compares them with Koka where Koka and Pleris both produce a value.
