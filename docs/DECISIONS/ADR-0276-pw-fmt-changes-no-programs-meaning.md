# ADR-0276: `pw fmt` changes no program's meaning

Status: accepted under the owner's delegation of 2026-10-02. Found while
formatting the feed for ADR-0275. Date: 2026-10-08. Milestone: E14. Amends
ADR-0013.

## Context

- **`pw fmt` broke the feed.** Run over `examples/feed/app.pw`, it wrote
  `max_bytes 5_000_000` as `max_bytes 5 _000_000`, which `pw check` refuses.
  It also wrote `counted_follower(person, -1)`, in an `optimistic` clause, as
  `- 1`.
- **A policy's value is tokens, read as text.** The grammar keeps a
  clause's value as the tokens after its name, with no expression's nodes,
  so the formatter's rules for an expression's spaces split them: a prefix
  `-` hugs its operand only inside a `UnaryExpr`. What reads a value reads
  its text: the upload's byte count is `policy::count` of the clause as
  written.
- **The lexer had no digit separator.** `5_000_000` was `5` and the name
  `_000_000`. A clause accepted it, read as one word of text, and an
  expression refused it as two statements on one line (PW0030). Yet the
  HIR's readers of a literal stripped a `_` (`values.rs`, `check.rs`, an
  integer read in `lower.rs`), as if the lexer had kept one.
- **ADR-0013's gate could not see it.** It compared the significant tokens
  before and after, and the lexer read both texts as `5` and `_000_000`.
  "Semantics-preserving" was one of ADR-0013's non-negotiable properties;
  the check of it was the tokens alone.
- **Nothing ran `pw fmt` over the reference apps.** `just ci`'s `pw fmt
  --check` read the corpus, its library and rules, kiokun and the
  packages, but not the feed, the store, the demo or the generality cases.
  The feed had never been formatted.
- **Two layout defects, which change no meaning, showed in the feed.**
  - An `if` in a call's argument put its `else` branch a level deeper than
    its `if` branch, and the closing `})` a level deeper than `} else {`.
    The two blocks opened on different lines, so each counted a level
    apart from the argument list both sit in.
  - A clause's value continued on a second line came back flush with the
    clauses, as a clause of its own would. A declaration's level was
    dropped beneath any scope inside it, a rule meant for a type's field
    list.
- **Prior art.**
  - Python (PEP 515), JavaScript (ES2021's numeric separators) and Go 1.13
    group digits by one `_` between two digits, and nowhere else. Rust also
    allows a trailing `_` and several together.
  - gofmt and rustfmt hold a formatter to the parse: the output must mean
    what the input meant.

## Decision

1. **A `_` between two digits groups them** (Python, JavaScript, Go): one
   `_`, between two digits, in a whole number or either part of a decimal;
   `5_000_000` is one token. A literal's value is its digits, with the `_`
   dropped where the literal enters the HIR. A `_` anywhere else groups
   nothing, and the number ends before it, as before.
2. **A policy's value keeps its gaps as written.** Two of its tokens written
   together stay together, and two written apart stay apart, with one space
   between. A clause's name is aligned as ADR-0013 rules. A policy that
   owns a block is formatted as any body is.
3. **`pw fmt` refuses to write a program that says something else.** Before
   it writes, or where `--check` would, it compares the parse of the input
   with the parse of its output: the same significant tokens in the same
   order, and in each policy's value the same gaps. A difference is refused
   by name, and the file is left as written: "formatting it would change
   what it says, at line L: `optimistic`'s value `-1` would be `- 1`".
4. **Layout, as the tree says.**
   - A line that begins by closing a construct an earlier line opened
     indents from that earlier line: `} else {` sits where its `if`'s block
     opened, and what the `else` opens sits where the `if` branch does.
   - A clause's value continued on a second line sits one level in from
     its clause.
5. **The reference apps, the demo and the generality cases are held to
   `pw fmt --check`**, formatted once. `examples/history` keeps its sources
   as they were.

## Acceptance

- **`compiler/pw-syntax/src/lexer.rs`**: `a_digit_separator_stands_between_
  two_digits`: `5_000_000`, `1_000.000_5` and a duration's `30_000.millis`
  are each one number. The controls: `5_`, `5__000` and `1_.5` end the
  number before the `_`.
- **`compiler/pw-syntax/src/fmt.rs`, 4 tests.**
  - A policy's value keeps its gaps: `max_bytes 5_000_000`, `f(q, -1)`, and
    `g(r,1)` as written. The controls: a body's expression is spaced as
    ever, and so is a block policy's body.
  - A change of what a program says is named: a gap in a value, and a
    token. The controls: a body's spaces, and a run of spaces in a value,
    are no change.
  - An `else` branch lines up with its `if` branch inside a call's
    argument.
  - A clause's continued value sits one level in.
- **`compiler/pw-syntax/tests/corpus_fmt.rs`**: every program `just ci`
  holds to `pw fmt --check`, 342, not only the corpus's 91: formatting is
  idempotent, keeps every token and comment, still parses, keeps what each
  program says (new), and finds each already formatted.
- **`compiler/pw-core/tests/backend_lowering.rs`**: `5_000_000 + 1_0` lowers
  to 5000000 and 10. The control: the same digits, written whole, lower to
  the same numbers.
- **`scripts/fmt_meaning_mutations.py`, 9 mutants**:
  - a `_` grouping nothing, and grouping wherever it stands;
  - the HIR keeping a number's `_`;
  - a policy's value spaced as an expression, and a block policy's body
    kept as written;
  - the comparison of a value's gaps, and of a token's text, dropped;
  - a line that begins by closing indenting from itself;
  - a clause's value continuing flush with the clauses.

  Recorded by `just e14-fmt-meaning`.
- **The scripts whose anchors this moved, each run whole**:
  `computed_rows` and `speculated_arms` (the feed's `optimistic` clause, one
  level in now), and `pattern` (a negative literal's arm, which drops its
  `_` now).
  - The other 18 scripts with a mutant in `lower.rs` are unaffected. No
    program a test reads writes a `_` in a number but a policy's value,
    which `lower.rs` does not lower, and this ADR's own test.
  - The scripts with a mutant in the formatted sources are unaffected too:
    their anchors match as before, and `meaning_kept` holds each formatted
    source to the program it was.
- **The whole workspace's tests**, and `pw fmt --check` over the 342.

## Not claimed

- **The CLI's refusal is not under mutation.** It calls `meaning_kept`,
  whose comparison is, but nothing the formatter writes now reaches it. It
  is a guard for the formatter's next bug.
- **A layout**: as ADR-0013 says, `pw fmt` is a canonical spacing. It
  breaks no line the author did not, and joins none.
- **A message of its own for a misplaced `_`**: `5_` and `5__000` are
  refused as they were, by what follows the number.
- **A policy's value parsed as what it is** (an expression, a list of
  events, a predicate): with nodes of its own it could be spaced as an
  expression is. Its gaps are kept as written until then.

## Alternatives

- **Rust's separator rule** (`5_`, `5__000`): more ways to write one
  number, where the language holds to one.
- **Comparing the HIR before and after**: a policy's value is its text
  there, so a run of spaces collapsed to one would read as a change of
  what it says.
- **Leaving the reference apps out of `pw fmt --check`**: the formatter's
  defects lived exactly where nothing ran it.
