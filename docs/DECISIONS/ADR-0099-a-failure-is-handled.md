# ADR-0099: a failure is handled

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §16.2,
"unhandled ADT variant"; ADR-0011).

## Context

A `Result` carries a failure. ADR-0011 holds a `match` to every case of the
value it takes apart, and ADR-0084 refuses showing one unopened. A value
that nothing uses handles neither of its cases. On 2026-09-26, at 5c2f76d,
a statement's `Result` was dropped without a word:
- **of two `Carts.add(..)` statements, the first could fail**, and the
  command went on to add again;
- **nine corpus fixtures dropped one**, mostly in the transaction family:
  a `Database.rollback(tx)` whose failure was lost before `return Ok(())`,
  and a `Carts.clear(..)` that failed before `tx.commit()`. One of them,
  `rules/affine/transaction-ended-on-every-path.pw`, is `@expect: clean`.

## Decision

**A failure is handled** (PW0618). A `Result` whose value nothing uses is
refused. It is handled by:
- `?`, which returns it;
- `match`, which takes it apart;
- a binding that says it is ignored, `let _ignored = ..`, which discards it
  by name. `let _ = ..` does not parse: `_` is no binding name.

Which values are used:
- a statement's value is not;
- neither is a `for` body's, nor the last value of a body declared `-> ()`;
- a body's last value is, and a `return`'s (written `return` then the
  value, two statements);
- a lambda's body is: it is the lambda's result, its caller's to use. A
  handler's command answer is the runtime's to drop (KNOWN_LIMITATIONS);
- a block after a clause whose value is code (`acquire`, `release(h)`,
  `draw(ctx)`) is that clause's value: the handle `acquire` produces.

The nine fixtures bind each dropped `Result` to a name that says so
(`_cleared`, `_rolled_back`, `_added`, `_current`), which keeps their
control flow and their subject. `?` would have added an early return where
a transaction is still live, which is another defect.

**(ruling needed)** Whether `let _ = ..` should parse as a discard. It needs
the affine rule to count a resource bound to `_` as never consumed.

## Acceptance

- **`compiler/pw-core/tests/results_handled.rs`**, 3 tests, each with
  controls. Two fail at 5c2f76d, the commit before. The third, what is
  returned is used, holds there too, and guards the change.
- **The nine fixtures are corrected, and their diagnostics are as before.**
  So is ADR-0080's test of an inner transaction, whose branch ended in a
  dropped rollback. The store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/results_handled_mutations.py`,
  `just e10-results-handled`, 7 mutants.
