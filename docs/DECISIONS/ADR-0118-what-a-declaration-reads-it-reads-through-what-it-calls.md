# ADR-0118: what a declaration reads, it reads through what it calls

Status: accepted under the owner's instruction of 2026-10-02 ("restore it and
finish it as ADR-0118"). Date: 2026-10-02. Milestone: E10 (charter §7.8,
§14 M5, §14 M6 gate item 4).

The work began on 2026-09-26 as "ADR-0115". That number went to the
authorization decision merged first, so this record is ADR-0118.

## Context

The privacy rules read a declaration's label from its own declaration and
from what its body calls **one call deep**. A callee's own reads were never
followed. So a query declared `public` that reads the session, directly,
through a helper `fn`, or through another query, returned one reader's value,
and every rule that relies on the label treated it as public. At `671931b`
each of these passed `pw check`:

- **PW5101.** A `partition public` fragment that `depends_on` a public query
  whose body is `Carts.current(current_session())`. The first reader's cart
  is written into the entry every later reader is served.
- **PW5004.** A `cache shared` public query whose body calls a helper that
  reads `current_session()`. One cache entry, keyed by nothing, holding a
  session's cart.
- **PW5001.** A `cache shared` page reading a public query that relays the
  `session query` `Resources.Cart(current_session())`.
- **The component contract.** That page's contract allowed `build`, so a
  session's data could be written into a file built before any request.

The contract built its labels with `check::label_of` directly, so the
checker and the contract disagreed in the same way.

## Decision

**A declaration's label is joined with what it reads, through what it calls,
to a fixed point.**

1. A body *reads* the declarations it calls and the queries and
   subscriptions it names (`query X(..)`, `subscribe X(..)`), resolved as its
   own unit resolves them (ADR-0112).
2. `check::Reads` joins two labels over those reads until neither changes.
   Joins only add, so recursion terminates:
   - **declared**: the declaration's own visibility (`label_of`) joined with
     the declared labels of what it reads. PW5001 and the contract read this;
   - **returned**: the labels of the result types of what it reads, joined
     through *their* reads. PW5004 reads this.
3. **A command's reads stay its own.** A call to a command is a request, and
   the command runs as its own component (ADR-0113). A shared page whose
   handler calls `add_to_cart` does not become a session's page.
4. **PW5101 reads a dependency's value label**, not just its `privacy` and
   `cache` heads. A `partition public` fragment depending on a resource that
   reads a reader's value is refused. The message names what the dependency
   reads: "which reads Session<SessionId>".
5. **A secret is left out of a resource's value label.** A query that uses a
   key to fetch public data returns public data (ADR-0085). A secret reaching
   a fragment is not PW5101's question.
6. **The contract uses the same derivation** (`Reads::of(..).labels()`), so
   the checker's labels and the contract's placements cannot drift.

## Acceptance

- **`compiler/pw-core/tests/reads_through_calls.rs`**, 4 tests, each failing
  at `671931b`:
  - a shared fragment, for five shapes: reading the session, through a
    helper, through another query, relaying a session query, and reading the
    user. Controls: the same fragment `partition private`, a public relay of
    the public menu, and a query that signs with a secret and returns public
    data;
  - a shared cache read through a helper, with a `cache private` control;
  - a shared page reading through a public query (PW5001 for a relayed
    session query, PW5004 for a direct `current_session()`). Controls: the
    same page `cache private`, and a shared page whose handler calls a
    command that reads the session;
  - the contract: such a page loses `build`, and a public relay of nothing
    keeps it.
- **Corpus.** No fixture's diagnostics change. `just ci` passes. The store,
  kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/reads_through_calls_mutations.py`,
  `just e10-reads-through-calls`, 7 mutants, one for each piece of the
  decision above.

## Not claimed

Reads are followed through declarations the unit resolves. A read through a
function *value*, such as a callback, carries its label by ADR-0079, not by
this derivation. Label polymorphism in signatures stays the open ruling
ADR-0085 names.
