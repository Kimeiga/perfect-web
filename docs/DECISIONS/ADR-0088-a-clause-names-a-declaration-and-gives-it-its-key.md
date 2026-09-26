# ADR-0088: a clause names a declaration of its kind, and gives it its key

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4, §14
M6).

## Context

Four clauses name a declaration and pass it values:
- `depends_on Menu(id)` and `invalidates Cart(current_session())` name a
  resource and its key;
- `emits CartChanged(current_session())` names an event and what it
  carries;
- `invalidates_on MenuChanged(id)` names an event a declaration listens for.

The architect ruled on 2026-08-10 that "being inside a policy never exempts
an executable term from resolution". The frozen list in
`tests/policy_term_positions.rs` classified these as "names a DECLARATION,
keys are terms". Only `optimistic` had been parsed since. The other values
were text:
- the graph split the text at commas and looked each name up in any
  namespace. It reported a name that found nothing (PW5100), and drew an
  edge to whatever else it found;
- nothing resolved, counted or typed a key.

On 2026-09-26, at 2eeff94, each of these checked:
- `invalidates Cart(nosuch)`, a key naming nothing;
- `invalidates Cart(item)`, a `MenuItemId` where `Cart` is keyed by a
  session, so the command would drop an entry that cannot exist;
- `invalidates Cart(current_session(), 1)`, and `invalidates Cart`, with no
  key;
- `invalidates CartChanged(..)`, an event, and `emits Cart(..)`, a
  resource.

Nine keys in six files had the wrong type, each now corrected:
- **The store's own `emits CartChanged(current_session())`** gave a
  `Session<SessionId>` to an event declared with a `SessionId`, a different
  opaque type. Nothing produces a `SessionId`, so the event is corrected: it
  carries the session the cart is keyed by. No artifact changes, since the
  graph records a parameter's name and not its type.
- **R-045 and its rule twin** wrote `depends_on Menu(id), Cart(id)`,
  passing a store id to `Cart`. Each now depends on
  `Cart(current_session())`: the session whose request regenerates the
  entry, which is the leak they exist to show. Keying the fragment by a
  session would have given each session its own entry, and hidden the leak.
- **Three `private_in_shared_materialization` witnesses** keyed a fragment
  by a `SessionId` and passed it to `Cart`. Each is keyed by
  `Session<SessionId>` now.

Found on the way: `Policy::value` collapses runs of whitespace, and an
optimistic clause was parsed from it. With extra spaces in the clause, every
diagnostic inside it underlined the wrong columns. `itemz` was reported at
column 72 of a line where it starts at column 84.

## Decision

- **A clause names a declaration of its kind** (PW5103):
  - `depends_on` and `invalidates` name a resource: a query, a subscription
    or a resource;
  - `emits` and `invalidates_on` name an event.

  A name of another kind is refused by the graph, which drew the edge before.
- **A key's arguments are terms.** `depends_on`, `invalidates` and `emits`
  are lowered into `Policy::keys`: each name, and each argument lowered into
  the declaration's arena as a root in the new `Key` context. Each argument
  is resolved like any term (PW0021), and related to the declaration the
  name denotes as a call's arguments are:
  - count (PW0604);
  - names (PW0617);
  - types (PW0605).

  The name is looked up in the clause's namespace, never as a term. A name
  written alone is the declaration given no key.
- **An event has a signature**, so what it carries is related. It is in no
  term's namespace, so nothing calls it (ADR-0087).
- **A key's effects are not the declaration's.** They are not its work, as
  an optimistic clause's target is not: `current_session()` in a key reads
  the session to name an entry.
- **`invalidates_on` is its own domain**, `Listener`: nothing in it is
  evaluated, and what each argument binds is not settled. It keeps its text,
  and it is next (docs/NEXT.md).
- **A clause's terms are read where the source writes them.** An optimistic
  clause and a clause's keys are parsed from the source text, not from
  `Policy::value`.

**(ruling needed)**
- Whether `invalidates Cart`, a name alone, should mean every entry. It is
  refused now, as a declaration given no key.
- Whether a materialization may depend on another materialization. It is
  refused now, as a clause naming another kind.
- Whether a key's labels are checked: a secret in `emits` reaches the
  materializer unlabelled.

## Acceptance

- **`compiler/pw-core/tests/clause_keys.rs`**, 6 tests, each with controls.
  Five fail at 2eeff94, the commit before. The sixth, a clean program,
  holds there too, and guards the change.
- **`tests/policy_term_positions.rs`'s frozen list** loses `depends_on`,
  `invalidates` and `emits`, whose values are parsed now.
- **The five fixtures and the event are corrected**; no rejected, rule or
  generality fixture's diagnostics change. The store, kiokun and the
  accepted corpus check clean. Their artifacts are byte-identical, apart
  from ADR-0058's two handlers.
- **Mutation controls:** `scripts/clause_key_mutations.py`,
  `just e10-clause-keys`, 10 mutants.
