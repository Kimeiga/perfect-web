# ADR-0049: a string's escapes are the language's

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (settles
assumption A-023).

## Context

A-023 was open: "only an escape-free string literal has a value". The lexer
knew only that a backslash keeps the next character inside a string. What an
escape meant was stated nowhere, so the backends disagreed:
- the handler backend and the Wasm lowering refused any string with a
  backslash, and any `"""` string;
- the Koka and Marko backends passed the token through, so `\n` meant
  whatever each target's rules said.

The Marko adapter did the same with an interpolated string. So
`"hi {name}"` in a view expression became a JavaScript string with the braces
in it.

## Decision

### 1. One reading of a string token

`pw_syntax::strings` is the only code that reads a string token. It splits
the token into text, with escapes decoded, and holes:
- the grammar asks it whether the escapes exist;
- the lowering asks it where the holes are;
- every backend asks it, through `Literal::string_value`, for the
  characters.

### 2. The escapes

In a `"..."` string:

| written | is |
|---|---|
| `\n` `\t` `\r` | line feed, tab, carriage return |
| `\\` `\"` | a backslash, a quote |
| `\{` `\}` | a brace, which opens no hole |
| `\u{1F600}` | one Unicode scalar value, in one to six hex digits |

Anything else after a backslash is PW0014, reported at the backslash. A
surrogate, or a value past U+10FFFF, is PW0014 too. A hole is `{expr}`,
ending at the first `}`; an empty one is PW0014. A `{` with no `}` after it,
and a `}` alone, are text, as they were.

**(ruling needed)**: the set. It is the common core of Rust, Swift and
JavaScript. It has no `\0`, no `\xHH`, and no `\u` with four hex digits, so
that `\u` has exactly one form.

### 3. A `"""` string is raw

Its value is exactly the characters between the delimiters: no escapes, no
holes, and no indentation removed. The corpus writes one, A-024's `because`
justification, which reads as written. **(ruling needed)**: raw, rather
than Swift's rule of removing the closing delimiter's indentation.

### 4. Each backend encodes the value in its own syntax

| backend | the value, as |
|---|---|
| Wasm component | bytes in the data segment |
| JavaScript module (ADR-0044), handler (ADR-0033) | a JSON string |
| Koka | `\\`, `\"`, `\u` with four hex digits and `\U` with six, printable ASCII as is. Run against Koka 3.2.3 itself |
| Marko | a JSON string; an interpolation as its pieces joined with `+`, each hole `String(..)` |

### 5. Markup attributes are HTML

`<input pattern="\d+">` is an attribute's text, not a Pleris string. Its
backslashes are kept and never refused. Holes in it are found as before
(ADR-0042).

## Acceptance

- `compiler/pw-syntax/src/strings.rs`: 7 unit tests of the rules.
- `compiler/pw-core/tests/string_escapes.rs`: 7 tests, covering:
  - PW0014 at the backslash;
  - the value;
  - no hole at `\{`;
  - a hole beside an escape;
  - a raw `"""` string;
  - an attribute kept as written;
  - Koka's encoding;
  - Marko's join.
- `compiler/pw-conformance/tests/strings.rs`: each escape's value from a
  compiled component through the host, against the characters in Rust.
- `compiler/pw-conformance/tests/javascript.rs`: two queries with escapes and
  a hole. The component and the module agree, now on 34 queries and 6,800
  calls.
- `compiler/pw-core/tests/handlers.rs`: a handler sends the decoded value,
  where it was refused.
- Mutation controls: `scripts/string_mutations.py`, `just e10-strings`, 16
  mutants.

## Not done

- **A string inside a hole.** `"{f("a")}"` ends the outer token at the inner
  quote, as it always has. A hole holds an expression without quotes.
- **Policy strings are not decoded.** `because "..."`, `route "..."` and
  `host "..."` are read as written. None of them has a backslash.
