# ADR-0098: a name is written once where it is declared

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1).

## Context

A module declares each name once in a namespace (PW0024). Inside a
declaration nothing held a name to that. On 2026-09-26, at 9be6b32, each of
these checked:
- **a parameter taken twice**, `fn f(x: Int, x: String)`, and a type
  parameter, `fn f<T, T>`. The body saw one of the two;
- **a record's field declared twice**, `type P = P { x: Int, x: String }`,
  and a sum type's case, `| A | A`. A field access, a pattern and a
  match's exhaustiveness each read one of the two;
- **a policy written twice**, `cache private` then `cache shared`. Every
  reader of a policy takes its first writing, so the order decided whether
  a session's data was refused a shared cache (PW5001), and the second
  writing was dropped;
- **an attribute given twice**, `<a href="/a" href="/b">`. HTML keeps the
  first value, and drops the second;
- **a name bound twice in one pattern**, `P.Pair(a, a)`, binding `a` as an
  `Int` and again as a `String`, and a lambda taking `acc` twice. The
  checker typed the arm's `a` as the second binding.

A record literal naming a field twice was already refused (PW0612).

## Decision

**A name is written once where it is declared** (PW0028). Each of these is
refused where it is written a second time: a parameter, a type parameter, a
field, a case, a policy, a name one pattern binds (a `match` arm's, a
`let`'s, a `for`'s, or a lambda's parameters together), and an element's
attribute. Attributes are compared as HTML reads their names (ADR-0095). An
or-pattern's alternatives bind the same names, and count once.

One policy head repeats: an effect's `impact`. An effect has as many
impacts as it has facets, as the platform's `dom.mutate` and `style.mutate`
write them (`crate::policy::repeats`). Nothing else in the corpus writes a
name twice.

## Acceptance

- **`compiler/pw-core/tests/declared_once.rs`**, 5 tests, each with
  controls, among them an effect with two impacts. All 5 fail at 9be6b32,
  the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/declared_once_mutations.py`,
  `just e10-declared-once`, 10 mutants. A lambda's parameter list lowers as
  one pattern, in either form, so taking its parameters one by one is not a
  mutant the tests can tell apart.
