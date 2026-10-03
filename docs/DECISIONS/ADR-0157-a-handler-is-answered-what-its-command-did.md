# ADR-0157: a handler is answered what its command did, never its value

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The first gap the audit of the canonical store found
(`docs/research/charter-15-store-audit.md`): item availability, end to end.
Charter §15.4 and §15.6 test 10.

## Context

1. **The store accepted any item.** Charter §15.4 says `add_to_cart`
   "revalidates item availability before commit". §15.6 test 10 says an
   "item becoming unavailable during mutation produces a typed error and
   consistent cart". The canonical store declared
   `CartError.ItemUnavailable` and never produced it.
2. **A refusal could not reach the page.**
   - The backend typed a handler's call to a command as `Unit`.
   - The server answered a refusal with `{"committed": false}`.
   - So a handler could not tell a refusal from a commit, nor which error
     it was.
3. **The obvious repair breaks a ruling.** The obvious repair gives the
   handler the command's declared result, `Result<Cart, CartError>`. The
   architect ruled on E7-R, 2026-08-06, that the proof is that "the UI
   changed because the declared resource dependency changed", not because
   an endpoint returned the cart.
   - A handler holding the cart could show it.
   - The page would then have two sources for one value.
   - They disagree whenever the resource's next patch is newer or older than
     the answer.

## Decision

1. **A handler's call to a command is typed `Result<(), E>`** (`ResolvedType::answered`).
   - For a command declaring `Result<T, E>`: `Result<(), E>`.
   - For a command declaring anything else: `()`.
   - The checker's typer, the value relations and the backend all use this
     type.
   - The handler learns whether the command committed, and if not, the
     error the command declares.
   - It never learns the value, which reaches the page from the query the
     command invalidates.
2. **The server answers in the wire form a handler's code decodes.**
   - A commit: `{"committed": true, "basis": [..], "result": {"$case": "ok"}}`.
   - A declared error: `{"committed": false, "result": {"$case": "err", "value": ..}}`,
     the error whole.
   - A command that trapped answers no `result`. Its press fails visibly
     (`data-pw-handler-error`), as before.
   - An interaction keeps the answer, so a retried request is given the
     first one's answer (ADR-0121).
3. **A command is called only by a page's handler** (PW0339). Before this,
   a command could be called from another declaration:
   - its body then ran without its own policies, so what it invalidates
     stayed stale and what it emits went unheard;
   - the checker typed that call as answered and the backend as declared.

   Logic two commands share goes in a `fn`, as the store's `add_to_cart`
   calls `Carts.add`.
4. **A handler that binds a command's value is refused where it is written**
   (PW0620). Two forms are refused:
   - `Ok(cart)` on a command's answer, whether directly or through a name
     the answer was bound to;
   - a name bound to what a command declaring no `Result` answered.

   The backend refused both before, but in the value's terms: "`.line_count`
   is read from a value of no declared record type".
5. **The canonical store revalidates availability inside `add_to_cart`.**
   - The command is `transaction serializable`.
   - It asks `Menus.is_available(item)` before writing, and refuses with
     `Err(CartError.ItemUnavailable(item))`.
   - The Add handler matches the answer, and shows "That item just sold
     out." in a `role="status"` paragraph.
   - The development server answers `store:data/menus#is-available` from a
     sold-out set. The control `POST /bench/stock?item=..&available=..` sets
     it: charter §15.5's "forced stale item".

## Alternatives

- **The declared result, value and all.**
  - React Router gives an action's return value to the page as
    `actionData`, then revalidates every loader.
  - SvelteKit gives it as `form`, then reruns the page's `load` functions.
  - In both, the page can render the action's data until the reload lands:
    two sources for one value.
  - Their guidance is to return the submission's feedback, and to read the
    page's data from the loaders. Pleris makes that guidance the type.
- **Committed or not, as a boolean.** The page cannot say which error, and
  test 10 asks for a typed one.
- **The error as text.** The handler cannot match on it, and nothing checks
  that every case is shown.
- **The error as a resource**, a query of the last refusal. The error belongs
  to the press, not to every page that reads the resource.
- **Answered typing only for a handler's calls, with a command callable
  anywhere.** A call from a declaration would keep its value, and its
  policies would still not apply. PW0339 refuses that call instead.

## Corrections found on the way

1. **A compiled handler dropped a command whose answer decodes as `()`.**
   - The handler module's decoder for `()` returned `undefined` without
     reading its argument, and its argument was the
     `await context.command(..)` itself.
   - So a command declaring no `Result` was never sent.
   - `handlers.rs` caught it before anything was committed.
   - The command is now sent on its own line, and its answer decoded after.
2. **A kept answer lost the difference between `null` and no answer.** A
   retry of a command declaring no `Result` would have been answered
   nothing, and its press would have failed where the first had succeeded.
3. **A handler matching on its command's answer had no name** in its event
   part (`called_name`). A `match` on a call is now named for the call.
4. **ADR-0154 called its rule PW0339.** The registry, the tests and every
   other document say PW0338. ADR-0154 is corrected.
5. **E14-A's contract ran against the canonical store**, although ADR-0156
   says its parity is with the benchmark's store.
   - It now serves `dist-baseline`, which `baseline-store.sh` builds from
     `benchmarks/baselines/pleris`.
   - Its mutant now anchors on that store.
6. **The value relations typed a command's call by its declared result.**
   - A mutant that reverted the checker's typer survived: no test could tell
     the two types apart.
   - Looking for one found a third typer, the value relations behind
     PW0605. It still gave the call the declared result.
   - So a handler passing its answer to a function that expects the value,
     `describe(add(1))`, checked. Only the backend refused it, reading a
     field of nothing.
   - The value relations type the call as answered now, and PW0605 refuses
     it at `pw check`.
   - One test observes each typer: PW0605's message, and PW0618's naming
     `Result<Unit, CartError>`.

## Acceptance

- `compiler/pw-core/tests/answers.rs`:
  - PW0339 and PW0620, each with controls that check clean;
  - a command's answer has one type in every typer: PW0605 refuses it where
    the value is declared, and PW0618 names it as the handler is given it.
- `compiler/pw-core/tests/handlers.rs`: the store's Add handler, run under
  Node.
  - An `Ok` answer clears the notice.
  - `ItemUnavailable` sets it.
  - An undeclared case traps.
  - A command declaring no `Result` is sent.
- `compiler/pw-conformance/tests/oracle.rs`: the compiled `add_to_cart`
  agrees with its reference for an available item and a sold-out one, 300
  generated cases each. A reference that skips the availability read is
  caught.
- `runtime/pw-host/tests/pleris_component.rs`: in the host, a sold-out item
  is refused by name, and nothing is written.
- `pw-dev-server`:
  - a sold-out item is refused by name, and a retry is given the same
    answer;
  - back in stock, the item commits, answered `Ok` without the cart;
  - a kept answer is the answer.
- The browser, in Chromium, Firefox and WebKit:
  - `e2e/availability.spec.mjs`: the refusal is shown, and the count goes
    back;
  - `e2e/resource-path.spec.mjs`: a commit's answer is `{ $case: "ok" }`,
    with no value.
- `scripts/command_answers_mutations.py`: 16 mutants, recorded by
  `just e14-command-answers`.

## Not claimed

- **A handler can still drop its command's declared error.** `=> clear_cart()`
  returns its answer to a runtime that discards it, so a `CartExpired` goes
  unshown.
  - PW0618 refuses a `Result` dropped as a statement, but not one returned
    as a handler's value.
  - Ruling on it changes the language for every handler a benchmark task's
    patch writes, so it is the next decision. *Settled by ADR-0159.*
- **The page does not show availability before the press.** `MenuItem` has
  no `available` field, and nothing emits `InventoryChanged` (§15.1, §15.2).
