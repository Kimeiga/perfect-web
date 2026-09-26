# ADR-0083: a call's arguments open on the callee's line

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (NEXT's "the unit
value `()` on the next line parsed as a call").

## Context

Statements end at a newline. The grammar already ended an expression before
a `<`, a `-` or a `!` at the start of a line, since each also begins one:
`<main>` opens markup, and does not compare the line before it to `main`. A
`(` is the same kind of token, and was not treated so. On 2026-09-26, at
df45603:
- **`g(n)` then `()` parsed as one call, `g(n)()`.** A function that ends
  with a call and then the unit value was refused, with the message
  "`` does not resolve": the callee of that call is a call, which names
  nothing.
- **`let h = add(1)` then `(2 + 3) * 4` parsed as
  `let h = add(1)(2 + 3) * 4`**, and was refused the same way.

## Decision

**A call's arguments open on the callee's line.** A `(` at the start of a
line begins a new statement, as `<`, `-` and `!` do. A call may still break
inside its arguments, and a chain may still continue on a line that starts
with `.`, which cannot begin a statement.

## Acceptance

- **`compiler/pw-core/tests/call_lines.rs`**, 2 tests. The first fails at
  df45603, the commit before. The second, a call broken inside its
  arguments and a chain continued with `.`, holds there too, and guards the
  change.
- **The grammar's tests pass.** Every line in the repository's sources that
  begins with `(` is `()` opening a block's statements, so nothing parses
  differently. The store, kiokun and the accepted corpus check clean, and
  their artifacts are byte-identical, apart from ADR-0058's two handlers.
- **Mutation control:** `scripts/call_line_mutations.py`,
  `just e10-call-lines`, 1 mutant.
