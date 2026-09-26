# ADR-0090: `pw build` checks what `pw check` checks

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §14, E10 gate
item 1: the store "builds from source").

## Context

The declaration rules, `rules::check`, cover these:
- a retry without an idempotency key (PW0312);
- an unbounded retry (PW0313);
- a stale session read (PW0102);
- a keyed query with no stale-work policy (PW0325);
- read-your-writes on a public read (PW0101);
- a written `rollback` (PW0327);
- a placement that cannot grant an effect;
- an effect naming an operation (PW0332);
- a shared cache with no invalidation (PW0200).

They ran in one place: the `pw check` command called them beside
`check_sources`. `pw build` checks through `lower::Checked::of`, which runs
`check_units`, and they were not in it. On 2026-09-26, at 5c3238a:
- **`pw build` compiled R-015's `retry forever` query into a component.**
  `pw check` refuses it (PW0313).
- **`pw build` built R-014**, a command that retries and is not idempotent.
  `pw check` refuses it (PW0312).

Every test that asked `check_sources` for a program's diagnostics missed the
same rules. The corpus harnesses called `rules::check` again beside it, with
a comment saying so.

The rules' diagnostics had also never been held to the checker's standard.
The corpus tests that require it read only what `check_sources` reported,
and skipped a file it had nothing for. Once the rules ran there, those tests
found four things:
- **PW0312 and PW0325 restated their invariants** in words the registry does
  not use.
- **PW0313, PW0327, PW0200 and PW0332 carried no boundary span**, and
  **PW0101, PW0102 and PW0325 no explanation**, which charter §16.3
  requires.
- **R-014 expects PW0312 to name the policy as written**, "declares `retry
  bounded_exponential`", and it said "declares a retry policy".

## Decision

- **One checker.** `check_units` runs the declaration rules, so `pw check`,
  `pw build` and every `check_sources` report the same diagnostics. The
  command no longer calls them beside it, and neither do the corpus
  harnesses.
- **The rules meet the checker's standard.**
  - A rule's invariant is the registry's sentence for its code; an alias the
    registry does not hold keeps the rule's.
  - Every rule carries a boundary span, an explanation and a repair.
  - PW0312 names the policy as written.

`pw check` still prints PW0100 beside PW5001 for a session's data in a
shared cache, two detectors for one invariant, as it did before. PW0100 is
the registry's alias for PW5001, and R-004 expects both messages.

## Acceptance

- **`compiler/pw-core/tests/one_checker.rs`**, 3 tests:
  - the build refuses R-014 and R-015, and `check_sources` reports them. Both
    tests fail at 5c3238a, the commit before;
  - the store builds, which holds there too and guards the change.
- **`tests/checking_source.rs`** holds the rules to charter §16.3, to the
  registry's sentences and to the corpus's `@expect-error` lines. It failed
  on four rules until they were repaired.
- **Only PW0312's message changes** among the rejected, rule and generality
  fixtures' diagnostics: R-014 and a witness now name `retry
  bounded_exponential` and `retry fixed`. The store, kiokun and the accepted
  corpus check clean. Their artifacts are byte-identical, apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/one_checker_mutations.py`,
  `just e10-one-checker`, 5 mutants.
