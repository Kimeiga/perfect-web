# ADR-0108: a query names a resource that exists

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.5,
ADR-0047).

## Context

A page reads a resource by querying it: `let menu = query Menu(id)`. The
statement lowers to a keyword expression whose first modifier is the
resource's name. ADR-0047 has every name used as a value resolve. The name
check handled the keyword family (`use key = ..`, `observe resize`,
`frame { .. }`) with one rule: bind the first word, as `use key` does. That
rule is recorded in KNOWN_LIMITATIONS as "binds any keyword statement's first
word (`query Store(..)`'s `Store` too), to keep a modifier quiet".

So a query's resource was never resolved. On 2026-09-26, at bc6eb7b, `let menu = query Nonexistent(id)` in a page checked with no
diagnostic. The graph kept the read as a dangling edge, and PW5100 reports a
dangling edge only for a `depends_on`, `invalidates_on`, `emits` or
`invalidates` clause. A page has none of those.

## Decision

**A query names a resource that exists.** In `query R(..)` and
`subscription R(..)`, `R` is a use, resolved as any name is (PW0021). The
statement binds nothing; its `let` binds the value. A qualified
`query Resources.Menu(..)` does not parse (PW0009), so the resource is always
a bare name, imported or declared.

The rest of the keyword family is unchanged: `use key = ..` still binds
`key`.

**(ruling needed)** What a query may name: a query, a subscription or a
resource. A name that resolves to something else, such as a function, is
not refused here.

## Acceptance

- **`compiler/pw-core/tests/queries_name_resources.rs`**, 2 tests. Each fails
  at bc6eb7b, the code at 7106daf:
  - a query naming nothing, against the library's menu;
  - a subscription naming nothing.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change in
  the harnesses, which give a fixture the accepted modules it imports. The
  store, kiokun and the accepted corpus check clean: every query in them
  names a declared resource.
- **Mutation controls:** `scripts/query_name_mutations.py`,
  `just e10-queries-name-resources`, 3 mutants.
