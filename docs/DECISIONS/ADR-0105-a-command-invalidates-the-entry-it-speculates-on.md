# ADR-0105: a command invalidates the entry it speculates on

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.5,
ADR-0025).

## Context

ADR-0025 makes an optimistic clause target a resource entry,
`optimistic Cart(current_session()) as cart => ..`, and says what happens
next:

```text
optimistic transition(before)  → speculative state displayed
command succeeds               → authoritative result reconciles it
command fails                  → restore before
```

Both backends reconcile through the entry:
- the Marko backend gives a binding its command's result when the command
  `invalidates` the query the binding reads;
- the dev server's page learns the committed value when an event reaches the
  entry.

On 2026-09-26, at 2bfe69c, a command speculating on the library's
`Cart` checked with neither `invalidates` nor an event `Cart` listens for. No
backend replaced the speculation, so it stayed on the page as if it had been
committed, whatever was. ADR-0101 could not see this: the library's `Cart`
reads nothing, so no write reaches it.

## Decision

**A command invalidates the entry it speculates on** (PW5107). Each
optimistic clause's target must be reached by its command, in one of two
ways:
- the command names the resource in `invalidates`;
- the command emits an event the resource listens for (`Graph::invalidates`,
  as for PW5106).

The target is resolved as PW0331 resolves it: a bare name in the term
namespace, a qualified path whole.

**One defect, one diagnostic.**
- When the target also reads what the command writes, PW5106 would report
  the same missing clause. It sets the entry aside for PW5107, whose message
  says why the clause matters here.
- A command whose `invalidates` or `emits` names nothing, or names a
  declaration of another kind, is PW5100's or PW5103's to report, and is set
  aside, as PW5106 does. The two share `clauses_refused` now.

**(ruling needed)** Keys are not compared, as for PW5106. The rule does not
check that `invalidates Cart(..)` names the entry the clause speculates on.

## Acceptance

- **`compiler/pw-core/tests/speculation_reconciled.rs`**, 5 tests. Each fails
  at e922292, the code at 2bfe69c. The cases, each with its control:
  - a speculation with neither clause, against `invalidates` (and a clause
    naming nothing, PW5100's alone);
  - an event the entry does not hear, against one it does;
  - a speculated entry that also reads the write, reported once;
  - a second command's `invalidates`, which is not this command's;
  - the underline, related spans and repairs.
- **ADR-0101's and ADR-0103's tests pass unchanged.** ADR-0101's two
  mutants for a refused clause anchored in the code `clauses_refused` now
  holds. They are re-anchored and still killed.
- **The ADR-0101 probe.** The store's `add_to_cart` with neither clause now
  reports PW5107, and not PW5106 as well.
- **Corpus.** No rejected, rule or generality fixture's diagnostics change.
  The store, kiokun and the accepted corpus check clean: every optimistic
  clause in the corpus has its `invalidates`.
- **Mutation controls:** `scripts/speculation_mutations.py`,
  `just e10-speculation-reconciled`, 5 mutants.
