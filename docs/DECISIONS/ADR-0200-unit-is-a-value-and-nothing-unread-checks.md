# ADR-0200: `()` is the unit value, and nothing the compiler cannot read checks

Status: accepted under the owner's delegation of 2026-10-02. Found building
ADR-0199. Date: 2026-10-05. Milestone: E14.

## Context

- **`()` written as a value was an expression that did not parse.**
  - The parser accepted it, an empty parenthesis.
  - The lowering, which is total, made it an error node.
  - Nothing reported it.
- **The checker types an error node as anything**, since the parser's error
  says what is wrong. So `fn f() -> Int !{} { () }` checked, and so did
  `let x = ()` returned as a `String`. The build refused them later: "the
  backend does not lower an expression that did not parse yet".
- **Programs write `()`.** R-022's handler, the generality fixtures'
  `{ () }` bodies, and tests each checked through this.
- **`check_sources`, which every test calls, reported no syntax error.**
  `pw check` reports a file's syntax errors, and checks only the files that
  parse. `check_sources` lowered every file whatever the parser said, and
  returned the checker's findings alone. Holding it to `pw check` found four
  tests passing on programs that do not parse:
  - **`named_arguments.rs`:** its control put a string inside a string's
    hole, `"{pick(x = "public", n = 0)}"`, which does not parse. It asserted
    no diagnostics, and none came.
  - **`each_typing.rs`:** its fixture module wrote `type ItemId = { id:
    String }`, a record syntax the language no longer has.
  - **`corpus_history.rs`:** R-019's C8 text writes a comment as `//` in
    markup. Since ADR-0167 that is text, and the `<ul>` it mentions opens an
    element that never closes. Its invariant was checked on what recovery
    made of it.
  - **`template_blocks.rs`:** an unclosed `{#if a}`. Its test expected
    PW5019, "`{#if}` is never closed".
- **That last one is a parser defect.** The template region counted elements
  and blocks in one depth. A block left open took its element's close tag,
  and every `}` after it, as markup. `pw check` reported "unclosed block,
  expected `}`" twice, at the end of the file. The precise PW5019 was
  reachable only through recovery, which `pw check` never shows.
- **Sources:**
  - Haskell 2010 §3.9: "The unit expression () has type ()."
  - The Rust Reference, tuple expressions: "Tuple expressions without any
    tuple initializer operands produce the unit tuple."

## Decision

1. **`()` is the unit value**, of the type `()`, which the language already
   has and also writes `Unit`.
   - It lowers to a literal and is typed `Unit`. `fn f() -> Int !{} { () }`
     is PW0606: "declares its result `Int` and this produces `Unit`".
   - Each backend builds it as the unit constant it already had.
2. **`check_sources` is `pw check`.** A file's syntax errors are its
   diagnostics, and a file that does not parse is kept out of the program.
   The backend's `Checked::of` shares the conversion.
3. **An expression or a pattern the compiler cannot read is refused**
   (PW0015). Every file the checker is given parses, so an error node there is
   text the parser accepted and the lowering has no meaning for.
4. **A gate over every `.pw` file in the repository**: each file that parses
   lowers with no error node.
5. **A block left open ends with its element.** The template region tracks
   what is open, element or block, innermost last. A close tag ends the
   blocks opened inside its element and left open. Each keeps no closing
   marker, and the checker reports it (PW5019), at the block.
6. **The four tests are corrected.** Their programs are written in the
   language as it is. R-019's C8 text is listed as one that no longer
   parses. The suite checks that it does not parse, and that it is refused
   for that.

## Alternatives

- **Make `()` a syntax error.** The language has the type `()`, and programs
  write a value of it. Haskell, Rust, OCaml, Elm and Swift all write it
  `()`.
- **Type an error node as no type**, so that it disagrees with every
  expected type. The diagnostic would name a type mismatch, not the
  expression.
- **Leave `check_sources` lenient, and add a separate strict entry.** Tests
  would keep checking what `pw check` never shows.
- **Report an unclosed block from the parser.** The checker already names the
  block and its span. The parser's part is to end the block where its
  element ends.

## Not built

- **An element left open is still reported at the end of the file.** A close
  tag closes the innermost element whatever its name. So in
  `<main><div>A</main>`, `</main>` closes `<div>`, and `<main>` takes every
  `}` after it. Matching a close tag to its element by name, and naming the
  element left open, is its own ADR, beside HTML's optional end tags.
- **A string's hole cannot hold a string.** `"{f("a")}"` does not parse,
  where Swift's and Kotlin's interpolations read one.

## Acceptance

- **`compiler/pw-core/tests/unit_value.rs`**, 6 tests:
  - `()` is a value of `()`, and of no other type;
  - a command that answers `()` is built;
  - `check_sources` reports a file's syntax errors, and keeps the file out
    of the program;
  - an expression, and a pattern, the compiler cannot read are refused
    (PW0015).
- **`compiler/pw-core/tests/every_expression_is_read.rs`:** every `.pw` file
  in the repository that parses lowers with no error node.
- **pw-syntax's `a_block_left_open_ends_with_its_element`**, and
  `template_blocks.rs`'s `an_unclosed_block_is_refused`: PW5019, once.
- **No program changes.** The store's build is byte-identical, and the
  corpus checks as before.
- **`scripts/unit_value_mutations.py`**: 8 mutants (`just e14-unit-value`).
