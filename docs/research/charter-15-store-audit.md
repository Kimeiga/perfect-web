# The canonical store against charter §15 — audit of 2026-10-03

An audit of `examples/store` and the development server that serves it
against charter §15 (§15.1-§15.6), requirement by requirement. Each line
names the evidence: a file, or a test by name. Line numbers drift, and are
left out. A feature that exists only in a benchmark task's patch is not in
the canonical store, and is marked so.

It also corrects a count this project had repeated: §15.6 lists **18**
required tests, not 17.

## Summary

The cart and command core holds:
- a private, session-scoped cart;
- idempotency by interaction;
- an optimistic update with its rollback derived;
- invalidation;
- commit with its event.

What is missing is mostly the store's breadth:
- availability (since met inside the command, ADR-0157);
- the route and more than one store;
- the cart's per-line controls;
- the recommendation and estimate slots, which only T05's and T10's patches
  add;
- most of §15.5's failure controls.

Two of the gaps were runtime defects, and fixing them found a third. All three
are fixed (ADR-0155):
- **A page whose subscription failed once stopped hearing changes for
  good.** `subscribe()`'s fallback to the long poll could never fire, since
  `chosenTransport()` gives the same answer every time.
- **A handler whose version did not match its document left its button
  dead** (test 16). The runtime logged the recovery the resume check
  computed and did not act on it.
- **The recovery codes were read one place off**, so the log named the wrong
  recovery for every refusal.

## §15.1 Domain model

| field | status |
|---|---|
| Store.id, name, hours | met (`domain.pw`; the server fills `hours`) |
| Store.description, menu_version | missing (the materializer's entry version stands in for the second) |
| MenuItem.id, name | met |
| MenuItem.store_id, description, price, available, category | missing; price exists only on `CartLine`, and the server prices every line at 450; a category exists only in T07's setup |
| Cart.id, consumer_id, version | missing as fields: a cart is keyed by its session, and its version is the runtime's entry version |
| Cart.items | partial: `lines: List<CartLine>` |
| CartLine.item_id, unit_price | met (`Money<USD>`) |
| CartLine.quantity: PositiveInt | partial: `PositiveInt` is `Int` underneath, unchecked at the boundary (KNOWN_LIMITATIONS) |
| DeliveryEstimate.min_minutes, max_minutes, generated_at | missing: `{ minutes: Int }`, and no `Instant` |

## §15.2 Privacy and placement

| data | status |
|---|---|
| store and public menu | partial: shared queries and a public materialized fragment; the edge placement is declared, not run (one origin node) |
| item availability | partial: read inside `add_to_cart` since ADR-0157; the page does not show it, and `MenuFragment` listens for `InventoryChanged`, which nothing emits |
| recommendations | missing from the store; T05's patches add them |
| delivery estimate | missing from the store; T10's patches add it |
| cart | met: a `session` query, `read_your_writes`, `cache private`, keyed by session; `consistency` is not enforced as a mode |
| payment methods | not needed in the first demo (the charter says so) |
| device location | missing in the store; the platform type exists, and the shared-cache rule refuses a `Device` value |

## §15.3 Page behavior

| item | status |
|---|---|
| route `/stores/:store_id` | missing: `StorePage(id)` has no route clause, and the server fixes `id` at "47" |
| semantic heading, menu content early, cart summary slot | met (the cart as a count) |
| delivery estimate slot, recommendation slot | missing from the store (T10's, T05's patches) |
| add item | met |
| increment quantity | partial: pressing Add again grows the line; no per-line control |
| decrement quantity, remove item | missing: the data layer has add, clear and current |
| retry recoverable failure | partial: a failed handler load is retried on the next press; a command is never retried |
| navigate away during a slow query | partial: run on T07's store (`e2e/keyed.spec.mjs`, ADR-0152); the canonical store has no slow keyed query |

## §15.4 `add_to_cart`

| clause | status |
|---|---|
| requires SignedIn | met (every development session is signed in) |
| idempotent_by InteractionId | met (ADR-0121), and required of every command a page sends since ADR-0154 |
| transactional write | partial: `transaction serializable` since ADR-0157, and the server commits state and event together on success; the data is in memory |
| optimistic transition, rollback on rejection | met (ADR-0122, ADR-0025) |
| invalidates the private cart | met |
| revalidates availability before commit | met since ADR-0157: `Menus.is_available(item)` inside the command, refused with `CartError.ItemUnavailable(item)` before anything is written |
| bounded retry for transport failures only | missing: no `retry` clause, and the runtime sends each request once (ADR-0154 makes a retry safe) |

## §15.5 Delays and failure injection

| control | status |
|---|---|
| recommendations 1200 ms, estimate 400 ms | met as controls (`/bench/recommendations`, `/bench/estimate`); the store's page reads neither |
| store delay, cart delay | missing |
| one-shot database error | partial: `/command/add_and_fail` runs one failing add; nothing arms the next real command or a read |
| one-shot network error | partial: aborts inside browser tests only |
| forced stale item | met since ADR-0157: `POST /bench/stock?item=..&available=false` |
| forced duplicate click | met (`idempotent-command.spec.mjs`, T08) |
| forced reconnect | partial: a forgotten page is told to reload; a dropped connection ended the subscription (fixed) |
| materializer failure | partial: injected only in `pw-materialize`'s own tests |

## §15.6 Required tests

| # | test | status |
|---|---|---|
| 1 | readable without JavaScript | met |
| 2 | static public shell holds no private cart data | partial: the store page is `cache private`, so it has no public shell |
| 3 | recommendations stream after core content | partial: the demo page and T05 |
| 4, 5 | one interaction one mutation; two interactions two | met |
| 6, 7, 8 | one key one request; a changed key cancels stale work; leaving cancels | met on T07's store, three engines (ADR-0152) |
| 9 | optimistic rollback | met |
| 10 | an unavailable item gives a typed error and a consistent cart | met since ADR-0157: refused by name, the handler shows it, the count goes back, in three engines (`e2e/availability.spec.mjs`) |
| 11 | `MenuChanged(store_47)` invalidates store 47 only | met in `pw-materialize`'s tests; the server holds one store |
| 12 | A's cart never observed by B | met |
| 13 | shared caches hold no session or secret fields | partial: synthetic values only |
| 14 | keyboard and screen-reader semantics | partial: every Add button is named "Add", and the count has no live region (the cart's notice has one since ADR-0157); no automated audit |
| 15 | focus preserved | met |
| 16 | handler version mismatch recovers | partial: refused safely, the recovery not acted on (fixed) |
| 17 | slow recommendations do not block Add | partial: T05's hidden test; the demo uses another button |
| 18 | last-known-good only for declared public data | partial: the materializer's tests; nothing limits the fallback to public data, and the server never serves one |

## Gaps, most important first

1. ~~**Item availability, end to end**~~ (§15.4, §15.5, test 10): met by
   ADR-0157, a check before commit, a stale-item control, and the typed
   error returned to the page. What remains is §15.1's and §15.2's:
   `available` on `MenuItem`, shown before the press.
2. **The route, and more than one store** (§15.3): `route "/stores/{id}"`,
   the server reading `id` from the address.
3. **The recommendation and estimate slots** (tests 3 and 17): in the store
   itself, with T05 and T10 re-based.
4. **Decrement, remove, and a per-line list** (§15.3).
5. **A command retried on a transport failure** (§15.4), with its
   interaction.
6. **§15.5's missing controls.**
7. **Last-known-good** (test 18): a rule limiting it to public data, and
   the server serving it.
8. **§15.1's fields**, and `PositiveInt` checked at the boundary.
9. **Accessibility** (test 14): each Add named by its item, a live region
   for the count, an automated audit.
10. **Tests 2 and 13** against the running store's shared output.

Changing the store's markup moves the context every benchmark task's patch
is anchored on, so items 1-4 and 9 re-base the tasks' patches with them.
