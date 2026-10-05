# ADR-0196: only a word that begins a statement or an expression is reserved

Status: accepted under the owner's delegation of 2026-10-02. Builds ADR-0195's
ruling 3. Date: 2026-10-05. Milestone: E14.

## Context

- **PW0013 (ADR-0041) refused a statement keyword as a name**: `let query =
  1`, a parameter named `use`. Its next use would have read as a `query ..`
  statement.
- **The words that begin an expression were not refused:**
  - `let return = n` checked, and the binding's next use read as a
    `return`;
  - `let match = n` was a parse error, PW0006 and PW0009, which name the
    wrong invariant;
  - a loop's or a pattern's binding was not checked at all.
- **The owner ruled (ADR-0195, ruling 3):**
  - reserve only words that can begin a statement or an expression inside
    a body;
  - leave every other keyword contextual, as `session` is as a parameter's
    name;
  - have the diagnostic suggest a name.
- **The parser already reads most keywords by position.** `page`, `view`,
  `type`, `cache`, `route` and `session` name values today.
- **The platform declares `fn query`, `fn measure` and `fn mutate`**, used as
  `document.query(..)` and `style.measure(el)`.

## Decision

1. **The reserved words are those that begin a statement or an expression
   in a body**, read from the parser's own two tables (`reserved`):
   - `if`, `elif`, `else`, `match`, `let`, `for`, `fn`, `return`, `true`,
     `false`, `signal`, `provide` and `derived`;
   - the body's statements: `query`, `command`, `subscription`, `use`,
     `observe`, `animate`, `frame`, `measure`, `mutate`, `post_paint`,
     `subtree`, `resource`, `subscribe` and `unsafe`.
2. **No reserved word names a binding.** That covers a `let`, a parameter, a
   loop's binding and a pattern's, and a signal. In a pattern, `true` and
   `false` are literals.
3. **A declaration may be named by a statement word, not by an expression
   word.**
   - A declaration is used by a call, and the parser reads a call by a
     statement word as a call (`query(..)`). So the platform's `query`,
     `measure` and `mutate` stand.
   - A call by an expression word reads as what the word begins:
     `if (..)`, `return (..)`, `fn (..)`, `derived (..)`.
4. **The repair suggests a name.** It reads "name it `match_`, or after what
   it holds": the trailing underscore PEP 8 gives "to avoid conflicts with
   Python keyword".
5. **PW0013 is revision 2.** Its invariant: "a word that begins a statement or
   an expression in a body names no binding, parameter or declaration".

## Alternatives

- **Reserve every keyword.** That would take `session`, `page`, `cache` and
  the platform's `query` from programs that use them for no ambiguity, and
  the owner ruled against it.
- **A raw identifier, Rust's `r#match`.** One more syntax, for a name a word
  of the program's own choosing serves as well.
- **Reserve only statement words, as before.** `let return = n` stays a
  binding whose use is a statement.

## Acceptance

- **`pw-syntax`'s `a_statement_keyword_cannot_name_a_value`.** Eight
  bindings, each refused with its repair: a `let`, two parameters, a loop's,
  a pattern's, a signal word. Also:
  - a function named `query` accepted, and one named `return` refused;
  - `session`, `page`, `cache` and `view` as names, and `true` and `false` as
    patterns, accepted.
- **No program changes.** The store's and kiokun's programs, the corpus and
  the standard packages check as before.
- **`scripts/reserved_words_mutations.py`**, recorded by
  `just e14-reserved-words`.
