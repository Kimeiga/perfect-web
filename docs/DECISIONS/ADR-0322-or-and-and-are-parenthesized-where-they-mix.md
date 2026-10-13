# ADR-0322: `|` and `&` are parenthesized where they mix

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14 (the language). Found by W9
(track `store-hours`, its first question to the integrator).

## Context

- **`|` and `&` were one power, left to right.** The parser gave both
  binding power 3 (`grammar.rs`, `infix_power`), so `a | b & c` was
  `(a | b) & c`.
- **Most languages read it the other way.** C, Rust, Java, Python and SQL
  bind `&&`, `and` and `AND` tighter than `||`, `or` and `OR`, and Boolean
  algebra writes conjunction first. A reader who knows any of them reads
  `a | (b & c)`.
- **W9's "open until" read one way and ran the other** (track
  `store-hours`, 2026-10-12). It compared `to_day == today | to_day == today
  + 1 & to == 0` and said "until today at 7:00 pm". Its test caught it, and
  the condition was parenthesized.
- **A swept corpus**: every shipped program that mixes the two parenthesizes
  each `&` group already: `pw-std`'s `list.pw:102`, kiokun-site's character
  ranges, kiokun's `app.pw:121` and `:156`.

## Decision

1. **`|` and `&` have no precedence between them.** A chain that mixes them
   without parentheses is refused at parse, once, at the operator where the
   mix begins: PW0031, "`|` and `&` are mixed without parentheses, and
   neither binds tighter", with "write `(a | b) & c` or `a | (b & c)`,
   whichever is meant".
2. **Nothing else changes.** A chain of one of them is left to right, as
   before, and a comparison inside one binds tighter, as before:
   `a == b | c > d` is `(a == b) | (c > d)`.

## Alternatives

- **Bind `&` tighter, as most languages do.** Rejected: a program written
  for the old parse would change meaning silently, and the reader who
  expects either reading still has to know which the language took. A
  refusal changes no program's meaning and names the fix.
- **Keep one power and warn.** Rejected: a warning is read past, and the
  program still runs the reading its writer did not mean.
- **Precedent.** Carbon's design leaves `and` and `or` unranked: "the
  precedence of `and` and `or` is not directly comparable with each other,
  so they cannot both be used directly in an expression without
  parentheses" (its logical operators' design, and proposal p0680). GCC's
  `-Wparentheses`, in `-Wall`, warns "when operators are nested whose
  precedence people often get confused about", `&&` within `||` among them.
  Both read 2026-10-12.

## Acceptance

- `compiler/pw-syntax/src/grammar.rs`'s test
  `or_and_and_mixed_without_parentheses_is_refused`: five mixed chains
  refused once each, at the operator where the mix begins; each grouping
  written out, chains of one, a comparison inside one, and a lambda's body of
  its own parse.
- Every shipped program and the corpus parse as before (`just ci`'s
  `test-compile`, `corpus`). One test's program mixed them: the JavaScript
  conformance program's `Inside`, `a & b | c`, now `(a & b) | c`, the
  reading it had (`pw-conformance`'s `javascript.rs`; found by the track's
  `ci` run 38298832576, since its local run had not run that crate).
- `scripts/logical_mixed_mutations.py`, each mutant killed.

## Not claimed

- **Bitwise operators.** The language has none: `|` and `&` are Boolean.
- **`|>`**, the pipeline, which is its own lowest power, and `|` in a type
  or a match's patterns, which no expression parses.
