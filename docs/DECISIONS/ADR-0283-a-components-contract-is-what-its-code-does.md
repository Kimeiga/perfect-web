# ADR-0283: a component's contract is what its code does

Status: accepted under the owner's delegation of 2026-10-02, on W6's
finding of 2026-10-09 and the integrator's probes of it. Date: 2026-10-09.
Milestone: E14. Amends ADR-0078 (what reads its effects) and E8-0's
handler rule (what a contract defers).

## Context

- **W6's finding** (track `kiokun`, kiokun's word page, 2026-10-09). A
  query that read kiokun's entries through a function of its own checked
  clean, and `pw build` refused it: "`Word` calls
  `kiokun:data/entries#read` and the world `kiokun-site-word` does not
  import it". The query now calls its host in its own body.
  - **The cause.** `contract.rs`'s `host_calls` read the declaration's own
    body. The backend compiles every function the export reaches into the
    component: inlined (ADR-0039 §4), or beside the export where it recurses
    (ADR-0050), or as a function value's code (ADR-0052). Its host calls are
    the component's, and the Wasm encoder held them to a world that lacked
    them (`wasm.rs`).
- **Probing it, at `33dd1ce` on 2026-10-09, found a hole in capabilities.**
  A query that read the database through a function passed by name,
  `List.map(names, found)`, or inside a lambda handed to a list operation,
  `List.map(names, n => found(n))`, required no capability. Its contract
  allowed `build`, `browser`, `edge` and `origin`, where the same read
  called directly is `origin` alone with `database.read<Thing>`. And a
  cached query that read so was no reader of what it read: a command that
  wrote it was never asked to invalidate it (PW5106), and the query kept
  what it had read after every write. Two causes:
  - **What a declaration performs had two derivations.** Since ADR-0078 the
    row check and the fixed point over helpers count what a body calls, the
    members it reads and the declarations it names as values.
    `effective_effects`, which placement, the source checks (ADR-0207) and
    the contract read, counted calls alone.
  - **The contract deferred every lambda's body**, as E8-0 ruled for a
    page's handlers (`on:press={.. => add_to_cart(..)}`), whose work runs
    when the event arrives. A lambda handed to `List.map` runs where it is
    written, in a query as anywhere.
- **The build caught what it could.** A function compiled into a component
  calls only what its world imports, so no host call ran unauthorized: the
  build refused. But the contract and placement under-stated what the code
  does, and the solver placed a database read in the browser.

## Decision

1. **A component imports each host operation its compiled code calls.**
   That is its body's calls, and those of each function compiled into it
   with it: a `fn` with a body and no `host` or `intrinsic` binding, called
   or named as a value. Each is walked once, transitively, in its own unit,
   resolved as the backend resolves it.
   - A local that shadows a function is the local's (ADR-0068), and nothing
     of the function is compiled in.
   - A query or a command the body calls is another component, its
     dependency (`component_calls`), and not walked.
   - A handler's code is the handler's: an `on:` attribute's lambda, and the
     declaration it names (ADR-0199), are compiled into the handler, not into
     the page that renders it.
2. **What a body performs is one walk**, the row check's: its calls, the
   members it reads and the declarations it names as values (ADR-0078).
   `effective_effects` reads it, and placement, the source checks and the
   contract read `effective_effects`. It is the fixed point's own last
   finding for each body, kept, not walked again.
3. **Only a handler's work is deferred**: the body of an `on:` attribute's
   lambda, and the declaration an `on:` attribute names (ADR-0199). A
   `<stream>`'s query stays deferred, as ADR-0148 rules. A lambda handed to
   a list operation, a function passed by name and anything else a body
   computes are the declaration's own.

## Alternatives

- **Refuse a host call reached through a function** (W6's second option).
  Refused: a helper is how a program factors its reads, and the backend
  already compiles one. The contract was what fell short.
- **Read the imports off the lowered IR**, so they are the encoder's by
  construction. Refused for now: a contract is made for every declaration,
  and the backend lowers only what it supports. Decision 1 resolves names as
  the backend does instead, and the build's audit still holds the two to
  each other.
- **Count a function value's effects in the contract alone**, leaving
  `effective_effects` as it was. Refused: placement read the same short
  answer, and two derivations of one fact is how this happened.

## Acceptance

- **`compiler/pw-core/tests/what_a_component_does.rs`**, one test a case,
  each with its control:
  - a host call one function down is imported, and the program builds;
  - a handler's code is the handler's: a page whose button calls a function
    imports nothing of it, and a view that calls it while it renders does;
  - a function passed by name is compiled in: imported, required, and placed
    at the origin alone;
  - a lambda handed to `List.map` runs where it is written: its read is
    required, and a pure lambda's control requires nothing;
  - a write reaches a cached query that reads through a function value
    (PW5106), and a reader whose entries expire is left to its window;
  - a handler's work is still deferred, a lambda's and a command named as
    one, while the command requires its write;
  - a local that shadows a function is the local's: nothing imported;
  - a recursion is walked once.
- **The store's contracts and worlds are byte-identical**
  (`evidence_is_current.rs` holds `docs/evidence/E8/` to the compiler). A
  first draft of decision 1 walked into commands a page's handlers call, and
  gave the store's two pages their commands' host imports; that test caught
  it.
- **`scripts/what_a_component_does_mutations.py`**: each piece undone fails
  the tests. Its first local run left "effective effects count calls alone"
  alive: the tests read the contract, which reads `effective_effects`
  through the exclusion, and no test read the checker's own readers. The
  PW5106 test reads one, and kills it.

## Not claimed

- **A call through a local is still charged its namesake's effects.** The
  effect inference's call walk resolves a callee's name without its scope,
  so a local `found` that shadows `fn found` is charged `fn found`'s row.
  That over-states, which refuses work rather than grants authority; it is
  the loop-name shadowing finding NEXT holds.
- **The imports are not read off the lowered IR.** Decision 1 mirrors the
  backend's resolution; the build's audit is the backstop.
- **A page's row still holds its handlers' work.** The row check keeps an
  `on:` handler's effects in the page's row, as later work (`contexts.rs`),
  and the grant check holds them to the page's placement. The contract
  defers them (decision 3). So a page placed at `build` whose button sends a
  command is refused, "`database.write<Thing>` is not available at placement
  Build" (PW5005), where its contract allows `build`. That over-states and
  refuses; it does not grant. The handler runs in the browser, and the
  command it sends runs at its own placement. Found probing this ruling,
  2026-10-09, and queued in NEXT.
