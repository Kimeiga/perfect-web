# ADR-0080: the affine rule follows bindings, not names

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (task 7, affine
annotations; ADR-0045).

## Context

PW2005 requires an affine value, a database transaction or a map handle, to
be ended exactly once on every path (ADR-0045). The checker found each use
of the value by its name: a release was a call given a name spelled like it,
and so were the body's value, a `return` and an escape into module state.
ADR-0063 moved the value relations, the declared types and the labels onto
the binding a name means (`crate::lexical`). The affine checker was not
moved.

A probe of function values after ADR-0079, on 2026-09-26 at 7eb7e21, found:
- **A transaction never ended passed.** Each branch of an `if` or a `match`
  began and ended an inner `tx`, and the outer `tx` was never ended.
- **Correct programs were refused.**
  - An outer and an inner `tx`, each ended once, were refused as the outer
    ended twice.
  - A lambda whose own parameter is named `tx` counted as ending the outer
    one inside a function value.
- **An end through a local counted nothing.** With `let end =
  Database.rollback`:
  - `end(tx)` alone was refused as never ending `tx`;
  - `end(tx)` then `Database.rollback(tx)` passed, ending it twice.

## Decision

- **A use of the value is a name whose binding is the acquisition's.** The
  binding comes from `crate::lexical`: a `let`'s pattern, a `use`
  statement, or a parameter the declaration's row promises to release.
  - Releases, the body's value, `return` and escapes all read it.
  - An assignment escapes only into module state, not into a local that
    shares its name.
- **A local bound to a declaration is that declaration where it is
  called.** With `let end = Database.rollback`, `end(tx)` ends `tx`. A
  binding that may be reassigned (`let mut`) is not read this way.
- **A value given to any other function value is refused** (PW2005, "is
  given to `f`, a function value, which may release it"). That covers a
  lambda, a parameter, a record field, or a declaration chosen at run time.
  The function may end it where nothing counts. A function type states no
  row (ADR-0078's ruling).

## Acceptance

- **`compiler/pw-core/tests/affine_bindings.rs`**, 4 tests, each with
  controls. All four fail at 7eb7e21, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** That
  includes R-011, R-012 and `examples/rules/affine`. The store, kiokun and
  the accepted corpus check clean, and their artifacts are byte-identical,
  apart from ADR-0058's two handlers.
- **Mutation controls:**
  - `scripts/affine_binding_mutations.py`, `just e10-affine-bindings`, 5
    mutants;
  - ADR-0045's control for the body's value is re-anchored to the new match.
