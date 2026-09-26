# ADR-0047: every name resolves, in lexical scope

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (a correction
to E9's resolution; charter §7.1, §16.3).

## Context

`PW0021` ("a name resolves") was reported in three places:
- a bare call whose callee is nobody's (`check::bare_call_is_unowned`);
- a qualified call whose head is a module (`check::unresolved_uses`);
- a name inside a policy term.

A name used as a value was examined nowhere. `let x = nothing` and
`{nothing.here}` in a template passed `pw check`, and every analysis read the
name as unknown, which each of them treats as "nothing to report".
KNOWN_LIMITATIONS listed it: "A name that resolves to nothing is refused only
in a call."

## Decision

### 1. A scoped walk over every body

`names::check` walks each body in lexical scope, in order:
- a `let` binds its names for the statements after it in its block, not for
  its own initialiser or anything before it;
- a lambda's parameters, a `for` pattern and a match arm's pattern bind for
  their bodies only;
- a template's `{#each xs as x}` binds `x` for the block's children, a
  `{:Some(x)}` arm for its own nodes, and a `<stream>`'s `<ready as={items}>`
  and `<failed as={e}>` for their children;
- a declaration nested in another sees the enclosing declarations'
  parameters and bindings.

A name no scope binds must be one of:
- a declaration this module can see, whether local, imported or from a
  prelude, or any declaration of its own file;
- a constructor of a sum type it can see;
- one of the language's values (`true`, `false`, `None`, `Some`, `Ok`, `Err`,
  `self`, `todo`, and the statement word `return`).

A field path's head is held to the same rule. A path through a module must
name a member of that module: `List.nothing` is refused. A record shorthand
(`Point { a }`) reads `a` from scope, and a method's receiver
(`unbound.method()`) must resolve. Callees keep their existing rule, which
knows the call-shaped owners: policy operators, CSS value functions, privacy
labels.

### 2. Clauses the grammar keeps as names

Inside a body, a clause is written as statements:
- `scope component` is two names;
- `acquire { .. }` is a name and a block;
- `release(h) { .. }` is a call and a block;
- a page's `view { .. }` is a name and a block.

The analyses that give these meaning read them in that shape
(`check::name_pair`, `routes::table`), and the policy table says what each
clause's value is (`policy::domain_of`, `policy::carries_terms`). So:
- A statement naming a policy head, with a value on its line or a block after
  it, is syntax. So is a UI noun followed by a block.
- Its value is a word of the clause's domain, unless the table says the
  clause carries terms. The analysis that reads the clause judges that word:
  `scope application` is the scope graph's.
- A block after the head is code. `release(h)` binds `h` in its block.

Two in-block heads the table lacked were added:
- `respects`, whose only word is `prefers_reduced_motion`;
- `on_mount` and `on_unmount`, the hooks of `unsafe.lifecycle`, as blocks.

**(ruling needed)**: clauses are recognised by position rather than
re-parsed as policies. The policy module already names the real fix, "the
`PolicyExpr` split"; this rule reads the shapes the analyses read and adds
no second vocabulary.

The structured-concurrency forms `task.spawn` and `durable.spawn`, which the
scope graph reads by path, own their bare-word arguments: `detached`, and the
scope in `scope = component`.

### 3. Two parse defects the walk found

- **`derived e` was a bare name and a discarded statement.** `derived`, the
  charter's "pure value computed from other values" (§7.5), was never
  syntax. So `let total = derived widths |> List.sum()` parsed as
  `let total = derived` followed by a statement whose value was thrown away.
  A-016 and A-018 computed nothing into `total`, `max` and `columns`. It now
  parses as one expression, `Expr::Keyword { keyword: "derived", args: [e] }`.
  **(ruling needed)**: `derived` is a reserved word, refused as a binding
  name (PW0013) as the statement keywords are (ADR-0041).
- **A statement's named argument was an assignment.**
  `observe intersection(self, threshold = 0.1)` read `threshold = 0.1` as an
  assignment to an undeclared name. The statement's argument list now
  consumes a named argument as a call's does.

### 4. Fixtures and tests that used undeclared names

These are corrected, and each is still caught, or still clean, as before:
- R-004 and five `private_in_shared_cache` witnesses read the session as a
  bare `session` that nothing declared. They now call `current_session()`,
  as the store does.
- R-021's form submitted to an undeclared `save_address`, which it now
  declares.
- The resume-capture witness bound a value named `derived`; it is
  `summary` now.
- Two tests' premises used an undeclared name to get an unknown type:
  `checking_source.rs`'s scrutinee and `cross_file.rs`'s pages.

## Acceptance

- `compiler/pw-core/tests/every_name_resolves.rs`: 20 tests, one scoping rule
  each, with its control.
- Every accepted file, the store and kiokun check clean.
- Every rejected fixture, rule fixture and generality witness reports exactly
  the diagnostics it reported before this change. That was compared against
  a build of `2141a98`.
- Mutation controls: `scripts/names_mutations.py`, `just e10-names`. There
  are twenty mutants. Each undoes one piece of the walk, or one of the two
  parse repairs.

## Not done

- **`derived`'s purity is not checked.** It parses as one expression. The
  charter calls it pure, and no rule says so yet.
- **A clause's words are not checked here.** A word outside its clause's
  closed set is left to the analysis that reads the clause, and most nested
  clauses have none.
