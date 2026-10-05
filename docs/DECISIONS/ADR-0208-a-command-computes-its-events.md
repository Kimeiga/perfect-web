# ADR-0208: a command computes its events, and the outbox commits them with its writes

Status: accepted under the owner's delegation of 2026-10-02. It builds
ADR-0195's ruling 11's other half, after ADR-0207's. Date: 2026-10-05.
Milestone: E14, the app layer.

## Context

- **The server computed a command's events from its keys' text** (ADR-0104).
  - It read `emits CartChanged(current_session())` off the compiler's graph
    and computed `current_session()` itself, the one value it knew.
  - Anything else was refused before the command ran:
    `emits ItemChanged(item)`, an event carrying the command's own argument.
    A feed, where a post's likes are keyed by the post, could not be written.
- **ADR-0104 named the two ways out**: the component returns its events, or
  calls an outbox host function.
- **Ruling 11** (ADR-0195): "the compiled command evaluates its `emits` keys";
  "the host writes the events in the same transaction as the writes"; "the
  server never evaluates a key's text".
- **The transactional outbox** (microservices.io): messages are sent "if and
  only if the database transaction commits". The application writes the
  outbox row itself, in its transaction. Debezium's outbox router reads the
  key the application wrote (`aggregateid`, "the event key") and computes
  nothing.
- **Found reading the path**:
  - An event's value reached a query's key as a `String` always, so an `Int`
    key would have been `"47"` against the entry's `47`, and missed. No
    command emitted one, since the server refused them.
  - The outbox joined an event's values with a separator, so one empty value
    was read back as none, and reached every entry.
  - A key's `current_session()` added nothing to the command's contract. The
    store's commands worked because their bodies read the session too.
  - **Correction:** `docs/evidence/E8/component-contracts.json` was last
    emitted at ADR-0181, and nothing compared it with the compiler. The
    pages and the order of ADR-0190 to ADR-0193 were in no committed
    contract, and the host's planning tests placed a store without them.

## Decision

- **An event is a function of the platform's outbox.**
  - `event CartChanged(session: Session<SessionId>)` is
    `pw:host/outbox#cart-changed`, taking what the event carries:
    `cart-changed: func(arg0: capability-session-id)`.
  - Two events of one name are refused where they would share it, as two
    host operations are (`WitError::Claimed`).
- **A command calls it, for each event it emits, before its body.**
  - Each key's values are lowered at the event's parameters, by name or in
    order, as the checker related them (ADR-0088).
  - They are evaluated on what the command was given, as the server did
    before it ran.
- **Emitting is an effect, `outbox.write`.**
  - It is declared in `packages/pw-platform-web/effects.pw`, with its
    capability.
  - A command that emits performs it, so its contract requires it. A node
    grants it where it keeps an outbox, as one grants `session.read`.
  - A command whose events a node cannot keep does not run there: its
    outbox is not linked. The alternative is its writes committing and its
    events lost.
- **What a key performs is the command's** (ruling 9, for `emits`). A new
  context, `Emitted`, contributes its effects and its imports: a key's
  `current_session()` is the command's `session.read`.
- **The host stages what the command hands it, and commits it with the
  writes.**
  - Each outbox import of a contract carries its event's path,
    `Events.CartChanged`.
  - The server links each to a function that stages the values. It commits
    them in the materializer's transaction with the state, or drops them
    with a declared error or a trap.
  - A key's text is its value's: a `String` itself, an `Int` in decimal, a
    `Bool` as `true` or `false`. Another type keys no entry and is refused
    (PW5308 holds a key to these three).
  - `declared_events` is deleted.
- **A query's key is bound to the value as the command computed it**, so an
  `Int` keys as an `Int`.
- **The outbox keeps an event's values as a JSON array.**

## Alternatives

- **The export returns its events with its result**, `tuple<R, list<event>>`,
  as ruling 11 says.
  - The IR has no tuple, and a result leaves through three exits (the body,
    `return`, `?`). Every caller reads the result as the one value it is.
  - And the host would have to check the returned events against what the
    command declares. A linked import cannot be called by a command that did
    not declare it.
  - The outbox call is the pattern itself: the application writes the
    event, in its transaction. So this keeps the ruling's substance (the
    command computes, the host commits with the writes, no text is
    evaluated) and refines its mechanism, which ADR-0104 left open.
- **Evaluate keys after the body.** A key names what the command was given,
  and the server evaluated keys before. A key reading what the body wrote is
  not something any program does.
- **One generic `emit(name, values)` import.** It would carry no types, and
  every value would have to be text before it left the component.

## Acceptance

Recorded by `just e14-command-events` in
`docs/evidence/E14/command-events.txt`:

- **`compiler/pw-conformance/tests/events.rs`**, 3 tests. A command emits
  `Moved(to + 1)` and `Named(to: to, who: "mover")`. Its component calls
  the outbox with 42, then with `"mover"` and 41, then the data layer. Its
  contract imports each event with `outbox.write` and requires it. Emitting
  nothing, it imports no outbox.
- **`runtime/pw-materialize/tests/outbox_values.rs`**: an event's values are
  read back as written, `""` among them.
- **The dev server's tests**:
  - `add_to_cart` imports exactly `CartChanged` and moves its cart's entry;
  - built without its `emits`, its write commits and no entry hears of it;
  - without `outbox.write` it is refused, and nothing is written.
- **`scripts/committed_events_mutations.py`**: ADR-0104's four controls of
  the server's evaluation are retired with it, and three are added.
  - **Correction:** at `558e35b` its first control, `CartChanged` committed
    whatever the command computed, survived. ADR-0104 had killed it by
    removing the graph's `emits` edge, and this ADR deleted that half of the
    test with the graph's part. The emits-nothing test above restores it.
- **The store's WIT, contracts and components** are emitted again, and
  `evidence_is_current.rs` now compares the contracts too. The planning
  tests' origin grants the order's `Orders` and `outbox.write`.

## Not claimed

- **`invalidates` keys are still read off their text by the server**:
  `current_session()`, or else the whole query. It is sound, since it drops
  more, but it is the same work. ADR-0209 builds it.
- **An event whose value is not a key** (a record, a list) checks, and is
  refused when the command runs. A rule for an event's parameters belongs
  with ruling 9's labels.
- **Events of a command that writes nothing are not committed**, as before:
  there is no transaction to commit them in.
