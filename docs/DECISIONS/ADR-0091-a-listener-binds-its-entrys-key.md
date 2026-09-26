# ADR-0091: a listener binds its entry's key

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.4, §14 M6;
ADR-0007).

## Context

Charter §9.4 says "`MenuChanged(store_47)` should invalidate only affected
resource snapshots and page fragments". ADR-0007 made invalidation explicit
typed events, and a declaration names the events it listens for:
`invalidates_on MenuChanged(id), InventoryChanged(id, _item: MenuItemId)`.
ADR-0088 related the keys of `depends_on`, `invalidates` and `emits`, and
left `invalidates_on` as text. On 2026-09-26, at af3978b:
- **The materializer read an event's values as a set.** It asked whether
  each was somewhere in the entry's key. `InventoryChanged(47, item 3)`
  reached no menu, since no menu is keyed by an item, and it was deferred
  forever: store 47's menu stayed fresh after its inventory changed. The
  store's own `MenuFragment` and A-009 listen for exactly that event. A value
  at one position also matched a key at another: `Pair(1, 2)`, listening for
  `MenuChanged(b)`, was invalidated by the menu of store 1.
- **Nothing checked what a listener wrote.**
  - The store's `MenuFragment` and two `graph_edge_unresolved` witnesses
    wrote `InventoryChanged(id, item)`, with `item` naming nothing.
  - A-009 wrote `_item: MenuItemId`, which parses as a named argument the
    event does not have.
  - A witness gave a store's event a consumer, `StoreChanged(consumer)`.
  - Three witnesses keyed a fragment by a `SessionId` and listened for
    `CartChanged`, which carries the session (ADR-0088).

## Decision

- **A listener's argument is one of the declaration's parameters, binding
  that part of its key, or `_`, any value** (PW5104).
- **Arguments are lowered like a key's**, into `Policy::keys`, as roots in
  their own `Listener` context. They are related to the event as a call's
  arguments are: count (PW0604), names (PW0617) and types (PW0605). A
  parameter's type is the event's; `_` is undecided. They are not resolved
  as terms, since nothing in them is evaluated.
- **The materializer compares position by position.** An `invalidated_by`
  edge's key is the listener's arguments as written, and
  `Graph::listens` reads each one:
  - a parameter's name binds the event's value at that position to that
    part of the entry's key;
  - `_`, or a position the list leaves out, binds nothing;
  - an event that carries no values still reaches every entry.
- **The corpus is corrected**:
  - the store's `MenuFragment`, A-009 and the two witnesses write
    `InventoryChanged(id, _)`;
  - the consumer's witness declares its own `OrdersChanged(consumer)`;
  - the three session witnesses are keyed by `Session<SessionId>`.

  The committed store graph is regenerated. Its one change is `item` to `_`.

## Acceptance

- **`compiler/pw-core/tests/listener_keys.rs`**, 2 tests, and
  **`runtime/pw-materialize/tests/listeners.rs`**, 3 tests, each with
  controls. All 5 fail at af3978b, the commit before.
- **`tests/policy_term_positions.rs`'s frozen list** loses `invalidates_on`.
  What remains is `privacy` and `requires`.
- **The corrected fixtures are the only ones that change.** Each reports the
  new rules uncorrected, and no rejected, rule or generality fixture's
  diagnostics differ after. The store, kiokun and the accepted corpus check
  clean. Their artifacts are byte-identical, apart from ADR-0058's two
  handlers. The materializer's gate tests hold: their listeners name their
  parameters.
- **Mutation controls:** `scripts/listener_mutations.py`,
  `just e10-listeners`, 9 mutants. ADR-0088's control for what `emits`
  names is re-anchored to the line that now names the listener too, and is
  still killed.
