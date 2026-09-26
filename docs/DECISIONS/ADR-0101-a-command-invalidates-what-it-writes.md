# ADR-0101: a command invalidates what it writes

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.5, §9.4).

## Context

Charter §7.5 makes a `command` the "explicit mutation with authorization,
idempotency, transaction, optimistic behavior, and invalidation".
[ADR-0007](ADR-0007-explicit-invalidation-before-inference.md) makes
invalidation explicit:
- a command `invalidates` the entries it drops and `emits` typed events;
- a cached query listens for events with `invalidates_on`.

It names the cost: "a missing declaration produces stale data". Its only
mitigation is PW0200, for a shared cache with no invalidation source.
Nothing held a command to the data it writes.

On 2026-09-26, at e1e8cb1, the store's `add_to_cart` with neither
`invalidates` nor `emits` checked. It writes `Carts`, which the store's
`Cart` reads with `freshness 0.seconds` and `consistency read_your_writes`.
The program told `Cart` nothing:
- the Marko backend makes a page's binding reactive, and gives it a
  handler's result, only for a query some command `invalidates`;
- the program emitted no event for the materializer to consume.

The dev server hid this. After any cart write it emits `Events.CartChanged`,
whatever the command declares, because it reads no command's `emits`. That
is recorded in KNOWN_LIMITATIONS, and NEXT carries it.

The corpus had the same gap in two places:
- **A-005 and A-004.** The accepted corpus is checked as one program. In it,
  A-005's `add_to_cart` invalidated only the library's `Resources.Cart`,
  whose body is `todo`. A-004's `Cart` reads `Carts` with zero staleness and
  read-your-writes, and nothing told it of the write.
- **Five generality witnesses.** The generality harness puts every accepted
  module in each witness's program. Three affine witnesses and both
  optimistic ones clear the cart and tell no reader.

## Decision

**A command invalidates what it writes** (PW5106). A command may perform
`database.write<T>`, directly or through what it calls. Then each query,
subscription or resource in the program that performs `database.read<T>`,
and declares no positive `freshness`, must be reached by that command in one
of two ways:
- the command names it in `invalidates`;
- the command `emits` an event that the reader itself listens for with
  `invalidates_on`.

The diagnostic underlines the call that writes and names the reader. Its
repairs are the two clauses. When the reader already listens for events,
the repair names them.

This is not the dependency inference ADR-0007 defers. Nothing is invalidated
that the program does not declare. The check compares two things the
program already states:
- the effect rows, which say which domain a declaration reads or writes;
- the clauses, which say what a command invalidates.

A command's inferred row counts, as it does for placement and contracts
(`effective_effects`). So does a reader's in another module.

The domain is the type argument as written: `Carts` in
`database.write<Carts>`. That is how a capability is identified
(`contract.rs`).

A command whose `invalidates` or `emits` clause PW5100 or PW5103 already
refuses is set aside. What that clause meant is not known, and one defect
gets one diagnostic.

**(ruling needed)** Decisions that need confirming:
- **A positive `freshness` excuses the writer.** Such a reader has declared
  how stale its entries may be, and they expire. Charter §9.4 allows
  time-based freshness "as a fallback or explicit policy". PW0102 already
  refuses a staleness window on session data, so this exemption applies only
  to public reads. `consistency strong` together with a window is not
  examined.
- **Only an argued write meets an argued read of the same domain.** A row
  naming only the family (`!{ database }`) or no argument names no domain,
  and nothing matches it. Neither does `database.transaction`.
- **Only a command is held.** ADR-0100 refuses a write inside a query. A task
  or a resource that writes cannot declare `invalidates` or `emits`
  (ADR-0092), so the rule could only refuse it. Nothing in the corpus does
  this.
- **Keys are not compared.** The rule does not check that an emitted event
  carries the entry's key: `CartChanged(current_session())` against the
  listener's `session`. The materializer matches an event's values against
  a listener by position (ADR-0091).

Recorded, not changed here: a materialization that depends on a reader is not
reached through the reader. The materializer invalidates the direct listeners
of a committed event and nothing that reads them, while the compiler's
`Graph::affected_by` follows reads transitively. A-009's `MenuFragment`
depends on `Store(id)` and does not listen for `StoreChanged`. NEXT carries
this.

## Corpus changes

- **A-004:** `Cart` listens for `CartChanged(session)`.
- **A-005:** `add_to_cart` emits `CartChanged(current_session())`.
- **The five generality witnesses:** each command emits
  `CartChanged(current_session())`. The witnesses are
  `affine_not_consumed_once/{live-across-try, never-released,
  released-before-return}` and `optimistic_not_pure/{caught, neighbour}`.

Each witness keeps its invariant and its status. The store already did both
(`invalidates` and `emits`).

## Acceptance

- **`compiler/pw-core/tests/writes_invalidated.rs`**, 7 tests. Each fails at
  2cbe6e9, the code at e1e8cb1. The cases, each with its control where one
  applies:
  - a command writing the cart with neither clause, against naming the
    reader and against emitting an event it hears;
  - a clause naming nothing (PW5100 alone) and one naming an event (PW5103
    alone);
  - an event only another reader hears;
  - a second command writing the same cart;
  - a reader in another module;
  - a staleness window, against one of zero;
  - a reader of another domain;
  - the underline, related spans and repairs.
- **`effects.rs`'s unit test `a_database_effect_names_its_domain`.**
- **Corpus.** No rejected or rule fixture's diagnostics change. The
  generality harness passes with the five witnesses repaired. The store,
  kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/write_invalidation_mutations.py`,
  `just e10-writes-invalidated`, 13 mutants.
