# ADR-0237: what lowering parses, it reports

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found while writing ADR-0238: a command
speculating on two entries writes its clause's arms with a comma between
them. Without the comma, the second arm vanished and `pw check` was clean.

## Context

- **The declaration grammar keeps some text as text, and lowering parses
  it.** Each is parsed on its own, with the expression grammar:
  - a clause's value: an `optimistic` clause (ADR-0122), and a clause naming
    declarations by their keys, such as `invalidates`, `depends_on` or
    `invalidates_on` (ADR-0088);
  - a string's hole, `{a}` in `"sum {a}"` or in an attribute's text
    (ADR-0042, ADR-0049);
  - the expression a block marker carries: `{#if c}`, `{#match e}`,
    `{:else if c}`.
- **Lowering dropped every error those parses made.** What one could not
  read was simply not there. Each of these checked clean before this ADR:
  - `invalidates Cart(current_session()) Order(current_session())`, on the
    store's `place_order`. The order was invalidated by nothing, a stale
    cache from a missing comma;
  - `invalidates_on Liked(id) Posted(_)`, on the feed's thread. The thread
    listened for no post. It was refused only by PW5107, for a clause that
    names `Posted`;
  - `"sum {a b}"`, which rendered `a`;
  - `{#if flag other}` and `{#match a b}`, decided by `flag` and `a`.
- **An interface's clauses were never parsed at all.** A declaration with no
  body is an interface (`effects.rs`), and its clauses' terms are lowered
  into its body's arena, so they were not lowered. A `query` with no body
  and `invalidates_on Changed(id) Other(id)` checked.
- **`{#if flag ==}` was refused as PW0015**, "an expression this compiler
  does not read", rather than by the parse's own error.
- **The grammar's codes for these were PW0103 to PW0105**, in the semantic
  range, which the registry keeps apart from syntax. None was registered:
  no one had seen them.
- **Text left after a standalone parse was an error for each token**, had
  anyone seen them. A missing `=>` was one for the arrow and one more for
  each token of the transition after it.

## Research

- **Svelte refuses what follows a tag's expression.** It reads the
  expression with acorn, then requires the `}`: `read_expression`, then
  `parser.eat('}', true)`, for `{#if ..}` and for a plain `{..}`
  ([tag.js](https://github.com/sveltejs/svelte/blob/main/packages/svelte/src/compiler/phases/1-parse/state/tag.js)).
  So `{#if a b}` is an error.
- **rustc keeps reading after an omitted separator**: "Attempt to keep
  parsing if it was an omitted separator", in `parse_seq_to_before_tokens`
  ([parser/mod.rs](https://github.com/rust-lang/rust/blob/master/compiler/rustc_parse/src/parser/mod.rs)).
  - When the next element parses, it is kept, and the error is the
    separator's, with a "missing `,`" suggestion.
  - When it does not, its error is cancelled, the separator's is reported,
    and the sequence ends.

  So one mistake is one error, and the elements are what was meant.

## Decision

1. **What lowering parses, it reports.** Each standalone parse's errors are
   kept in the HIR (`Hir::syntax`), each at its place in the file. The checker
   reports them first, as parse errors. `pw check` and the build refuse them.
   An interface's clauses are parsed for this alone: no arena holds their
   terms.
2. **What follows is one error, PW0016 `read_whole`**, over all of it:
   - "a hole is one expression";
   - "a block's subject is one expression";
   - "a branch's condition is one expression";
   - "an optimistic clause is one transition";
   - "a clause's values are separated by commas".

   A hole and a block marker's expression are parsed as one expression. They
   were statements in a synthetic function's body, where `a b` is two
   statements and parses.
3. **A missing comma is read as rustc reads it.** A clause's value with no
   comma before it is read, and the comma is the error, with its repair: the
   clause's keys are what was meant. One that does not parse there is no
   value: its own errors are dropped, and the comma is the error, over all
   that is left. A value that does not parse after a comma is its own error.
   - **An optimistic clause's transition with no `=>` before it is read**, so
     the arrow is the one error. After an arrow, a missing transition is
     PW0009, as it was.
4. **PW0017 `optimistic_clause`**: an optimistic clause with no `as <name>`,
   or no `=>`.
5. **A standalone parse names its end.** "expected an expression, found the
   end of a block's subject", not "end of file" in the middle of a file.
6. **An error node a standalone parse leaves is that parse's error**, not
   also PW0015.
7. **The parser's help renders as help**, a repair, as `pw check` renders a
   file's parse errors. It was a note.

## Acceptance

- **`compiler/pw-core/tests/read_whole.rs`, 6 tests**, each with controls:
  - a key clause's missing comma is PW0016 over `Posted(_)`, its help a
    repair; the clause's keys are `Liked` and `Posted`, and the build is
    refused. A stray `;` where a comma is missing is PW0016 alone;
  - an interface's clause with a comma missing;
  - an optimistic clause: text after its transition, no `as`, an arrow with
    no transition after it, no `=>`;
  - a hole in a string and in an attribute's text;
  - a block's subject, a `{#match}`'s and a branch's condition;
  - `{#if .. ==}`: one error, at the marker's `}`, naming the subject's end.

  Every diagnostic's code is registered.
- **`pw-syntax`'s unit tests, 3**: the rest one error over it; a missing
  comma read past, and what is not a value there the comma's error over the
  rest; a missing arrow read past, and an arrow with nothing after it an
  error. Each parse is lossless.
- **`scripts/read_whole_mutations.py`: 18 mutants**, recorded by `just
  e14-read-whole`.
- **Run whole, near what this changed**: `unit_value_mutations.py`, its
  "an expression the compiler cannot read checks" re-anchored, 8 of 8 killed;
  `clause_key_mutations.py` 10 of 10, `string_mutations.py` 16 of 16,
  `template_mutations.py` 16 of 16.
- **The workspace, 2,126 tests; the browser suite, 779 in three engines.**

## Not claimed

- **An `{#each}`'s head.** It is read by splitting its text, in five places:
  `resolve.rs`, `template_ir.rs`, `infer.rs`, `names.rs` and `marko.rs`.
  `{#each xs ys as x (x)}` checks. Lowering it once, by the grammar, is its
  own ADR.
- **Two expressions on one line of a block are two statements.**
  `fn f(a: Int, b: Int) -> Int !{} { a b }` checks, and is `b`. A ruling of
  its own.
- **The resource graph reads a clause's text, not its keys.** `graph.rs`
  splits the value at its commas, so it misses a value read past a missing
  comma, and a comma inside a key's string splits the key. The thread with
  `Liked(id) Posted(_)` is still refused by PW5107 beside PW0016. Reading the
  lowered keys is its own ADR.
- **An interface's keys are not resolved.** `invalidates_on Changed(nosuch)`
  on a `query` with no body checks; on one with a body it is PW5104.
- **The registry's other gaps.** The declaration rules emit PW0101 and PW0102,
  which are not registered. The parser's `for` without `in` is PW0102 too:
  one number, two meanings. Its own ADR, next.
