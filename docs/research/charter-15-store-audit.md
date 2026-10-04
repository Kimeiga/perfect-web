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
- the cart's per-line controls (since met, ADR-0172);
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
| Store.description | met since ADR-0166 |
| Store.menu_version | met by the runtime, by ADR-0181's ruling: the menu's materialized entry's version, which every patch to it carries; not a second field |
| MenuItem.id, name | met |
| MenuItem.description | met since ADR-0166 |
| MenuItem.price | met since ADR-0169: `Money<USD>`, shown in each row as `item.price.display`, and the price the data layer gives a cart's line (it gave every line 450 until then) |
| MenuItem.available | met since ADR-0178: what `menus#is-available` answers, shown in each row before the press |
| MenuItem.store_id, category | met since ADR-0181: the store's `Menu` answers `MenuSection`s, and the page gives each category a heading and its items |
| Cart.id, consumer_id, version | met by the runtime, by ADR-0181's ruling: a cart is its session's entry, keyed by the session, and its version is the entry's |
| Cart.items | partial: `lines: List<CartLine>` |
| CartLine.item_id, unit_price | met (`Money<USD>`) |
| CartLine.quantity: PositiveInt | met since ADR-0179: `PositiveInt` states `value >= 1`; every construction is shown to hold it at build, and the host checks a command's quantity and every cart a data layer answers |
| DeliveryEstimate.min_minutes, max_minutes, generated_at | met since ADR-0180: each bound a `PositiveInt`, the host checking an estimator's answer, and `generated_at` an `Instant`, which the platform's `clock` declares; the slot says the range in words |

## §15.2 Privacy and placement

| data | status |
|---|---|
| store and public menu | partial: shared queries and a public materialized fragment; the edge placement is declared, not run (one origin node) |
| item availability | met since ADR-0178: read inside `add_to_cart` (ADR-0157), shown in the shared menu, and a stock change is `InventoryChanged`, which the `Menu` query and its fragment hear, told to every page open |
| recommendations | met since ADR-0165: public, shared, kept ten minutes and dropped by `MenuChanged(id)`, streamed |
| delivery estimate | met since ADR-0165: the session's, private, kept for no time, streamed |
| cart | met: a `session` query, `read_your_writes`, `cache private`, keyed by session; `consistency` is not enforced as a mode |
| payment methods | not needed in the first demo (the charter says so) |
| device location | missing in the store; the platform type exists, and the shared-cache rule refuses a `Device` value |

## §15.3 Page behavior

| item | status |
|---|---|
| route `/stores/:store_id` | met since ADR-0160-0163: `route "/stores/{id}"`, held to the page's parameters; served at it, each document reading its own; a second store; an unknown store answered 404, as the page declares (`not_found_on StoreError.NotFound`, PW0342) |
| semantic heading, menu content early, cart summary slot | met: since ADR-0172 the cart lists its lines, with its subtotal |
| delivery estimate slot, recommendation slot | met since ADR-0165: each a named `<stream>` region, the estimate's said to a screen reader when it comes; painted before they are filled in WebKit too since ADR-0166 |
| add item | met |
| increment quantity | met since ADR-0172: each line's +, `increase_in_cart`, optimistic and idempotent, its item's availability read again |
| decrement quantity, remove item | met since ADR-0172: each line's − and Remove; at one, − takes the line away; focus passes to the next line, or the cart's heading |
| retry recoverable failure | met since ADR-0173 for a command: sent again where no answer came, as its `retry` clause bounds it; a failed handler load is retried on the next press |
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
| bounded retry for transport failures only | met since ADR-0173: `retry transport_only(max = 2, jitter = true)`, sent again by the runtime where no answer came, never where one did |

## §15.5 Delays and failure injection

| control | status |
|---|---|
| recommendations 1200 ms, estimate 400 ms | met (`/bench/recommendations`, `/bench/estimate`), and the store's page reads both since ADR-0165 |
| store delay, cart delay | met since ADR-0174: `/bench/store?delay=`, every reader's, and `/bench/cart?delay=`, one session's |
| one-shot database error | met since ADR-0174: `/bench/fail?next=write` or `next=read` fails the session's next cart write or read, once; the add of the server's own is retired |
| one-shot network error | met since ADR-0175: `/bench/drop?next=command&at=before\|after` closes the session's next command connection with no answer; the page sends it again (ADR-0173) and it is one mutation |
| forced stale item | met since ADR-0157: `POST /bench/stock?item=..&available=false` |
| forced duplicate click | met (`idempotent-command.spec.mjs`, T08) |
| forced reconnect | met since ADR-0175: `/bench/reconnect?for=ms` ends the session's subscriptions and refuses new ones for a while; what changed meanwhile reaches the page when it is back |
| materializer failure | met since ADR-0176: `/bench/materializer?fail=next` fails the session's next regeneration; it sends nothing, is tried again at the next drain, and the change reaches the page |

## §15.6 Required tests

| # | test | status |
|---|---|---|
| 1 | readable without JavaScript | met |
| 2 | static public shell holds no private cart data | partial: the store page is `cache private`, so it has no public shell |
| 3 | recommendations stream after core content | met since ADR-0165 in the store's response, and in three engines since ADR-0166 (`e2e/slots.spec.mjs`) |
| 4, 5 | one interaction one mutation; two interactions two | met |
| 6, 7, 8 | one key one request; a changed key cancels stale work; leaving cancels | met on T07's store, three engines (ADR-0152) |
| 9 | optimistic rollback | met |
| 10 | an unavailable item gives a typed error and a consistent cart | met since ADR-0157: refused by name, the handler shows it, the count goes back, in three engines (`e2e/availability.spec.mjs`) |
| 11 | `MenuChanged(store_47)` invalidates store 47 only | met since ADR-0162 for pages, end to end in three engines (`e2e/stores.spec.mjs`), and in `pw-materialize`'s tests; since ADR-0164 for the query cache too, which had dropped every store's kept menu |
| 12 | A's cart never observed by B | met |
| 13 | shared caches hold no session or secret fields | partial: synthetic values only |
| 14 | keyboard and screen-reader semantics | partial: since ADR-0168 each Add is named by its item, renamed with it, and the count is said in a polite live region (the cart's notice has one since ADR-0157); no automated audit |
| 15 | focus preserved | met, and since ADR-0172 through a speculation and the server's answer: a kept row's nodes stay, and focus in a row that goes passes on |
| 16 | handler version mismatch recovers | partial: refused safely, the recovery not acted on (fixed) |
| 17 | slow recommendations do not block Add | met since ADR-0165 in Chromium and Firefox, and in WebKit since ADR-0166, which gave the store enough text to be painted before its slots (`e2e/slots.spec.mjs`) |
| 18 | last-known-good only for declared public data | met since ADR-0177: PW0343 keeps `fallback last_known_good` to public data, the query runtime serves it for public data alone, and the store's page is shown with its last store while its origin fails, in three engines |

## Gaps, most important first

1. ~~**Item availability, end to end**~~ (§15.4, §15.5, test 10): met by
   ADR-0157, a check before commit, a stale-item control, and the typed
   error returned to the page; and §15.1's and §15.2's by ADR-0178:
   `available` on `MenuItem`, shown before the press, and told.
2. ~~**The route, and more than one store**~~ (§15.3): met by ADR-0160 to
   ADR-0163.
3. ~~**The recommendation and estimate slots**~~ (tests 3 and 17): met by
   ADR-0165, and in WebKit by ADR-0166.
4. ~~**Decrement, remove, and a per-line list**~~ (§15.3): met by
   ADR-0172.
5. ~~**A command retried on a transport failure**~~ (§15.4): met by
   ADR-0173.
6. ~~**§15.5's missing controls.**~~ Met by ADR-0174 to ADR-0176.
7. ~~**Last-known-good** (test 18)~~: met by ADR-0177.
8. ~~**§15.1's fields.**~~ The descriptions are met by ADR-0166, the price by
   ADR-0169, `available` by ADR-0178, `PositiveInt` checked at every
   construction and boundary by ADR-0179, `DeliveryEstimate`'s range by
   ADR-0180, and a category and a store for each item by ADR-0181, which
   rules a menu's version and a cart's identity the runtime's.
9. ~~**Accessibility** (test 14)~~: met by ADR-0182, but for the title. Each
   Add named by its item, and a live region for the count, were met by
   ADR-0168.
   - The audit reads the store in three engines, as served and after each
     kind of change, and tests the keyboard, the accessibility tree, live
     regions, reflow, a phone's width and reduced motion.
   - It found that one Add said its count up to four times, and that no page
     had a viewport. Both are fixed by ADR-0182.
   - Left open: the store's title is "Store" for every store (WCAG 2.4.2).
     A page declaring its title is ADR-0183.
   - Seen on the way, not an accessibility finding: store 48's page shows a
     line added at store 47. §15.1's `Cart` names no store, so a cart is the
     session's, across stores, as the charter has it.
10. **Tests 2 and 13** against the running store's shared output.

The benchmark's Pleris store is its own copy since ADR-0156, so changing
the canonical store's markup moves no task's patch. Until then, items 1-4
and 9 would have re-based the tasks' patches with them.
