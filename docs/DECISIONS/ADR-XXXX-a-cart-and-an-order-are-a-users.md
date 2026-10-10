# ADR-XXXX: a cart and an order are a user's

Status: proposed by track `store-accounts` (W8, `track/store-accounts`),
under the integrator's rulings of 2026-10-09 (docs/PARALLEL.md, "the
DoorDash track (W8's plan)" and "W8 launched, and its three questions
answered") and its answers below; numbered at the merge. Date: 2026-10-09.
Milestone: E14, the DoorDash store's customer side, milestone 1.

## Context

- **A cart was a session's.** The store read `session query
  Cart(session: Session<SessionId>)`, and its data layer kept a cart's lines
  and an order by the session's id. A reader who signed in on a phone and a
  laptop held two carts, and an order was followed only from the session that
  placed it. Accounts and sign-in (ADR-0258) and the reader's user as the
  host's (ADR-0270) had reached the feed, and not the store.
- **The rulings** (PARALLEL.md): a signed-in reader's cart and orders are
  their user's, a guest's its session's guest's (ADR-0270's guest model); at
  sign-in the guest's cart joins the user's, each line added, quantities
  summed within each item's bounds (ADR-0179); an order is read by its user
  alone, as a notification is (ADR-0274); a tab whose reader signed out
  elsewhere is refused at its next press, and told so. Tests: two users'
  carts never share a line, in memory and on PostgreSQL; a guest's cart
  follows them in; an order is no other user's to read.
- **What was found on the way**, each its own decision below:
  - a record's scope was its *last* scoped producer's, in the order of the
    units (`boundary.rs`'s `TypeFacts`), and `private` gave a record none;
  - `private` meant two things, the user's scope and not importable, and the
    language had no word for the first alone.

## Decision

### 1. The program says a cart and an order are the user's (Q1, (b))

- `user query Cart(reader: User<UserId>) -> Result<UserCart, CartError>`,
  `key reader`, `invalidates_on UserCartChanged(reader)`; `user query
  Order(reader: User<UserId>)` alike, on `UserOrderChanged(reader)`. Every
  page binds them with `current_user()`, the host's (ADR-0263, ADR-0270).
- **The commands** invalidate `Cart(current_user())` and emit
  `UserCartChanged(current_user())`: an event that carries the handle, which
  only the host makes, so the command names its own reader and no one else.
  ADR-0270's listener rule binds it to every entry at that user, in each of
  their sessions.
- **The operations by a user are their own interfaces**,
  `store:data/user-carts` and `store:data/user-orders`
  (`examples/lib/UserCarts.pw`, `UserOrders.pw`). `Carts`, `Orders`,
  `CartChanged` and `OrderChanged` keep their session's forms: about twenty
  compiler tests, the corpus's witnesses (A-004, A-005, R-011, R-029, R-062)
  and the benchmark's copy of the store call them, and one build holds both
  without one WIT function taking two types.
- **The store's data layer keys rows by an opaque owner**: the user's id
  for an operation by the reader's handle, the session's for one by the
  session. So the benchmark's copy of the store runs unchanged on the same
  layer (`served_from_patches`' T02 to T10 tests). On PostgreSQL, migration
  `0003_owners` renames `cart_lines.session` and `orders.session` to
  `owner`; rows written before it keep their values, each its session's,
  which the benchmark's copy still reads it by.
- **The host keys a session's cart entry by the cart's owner**
  (`cart_identity(owner, session)`), so the program's event naming the user
  reaches it; `Server::owner(session)` is the session's user where the
  program reads the cart by its reader (`Cart(current_user())`, found in the
  pages' plans), the session where it reads it by the session.
- **A user's cart is a record of its own, `UserCart`**, with its own
  `line_count`, `subtotal` and optimistic transitions (each line changed by
  `Carts`' own `grown` and `shrunk`). See 4.
- **Anyone fills a cart; a reader signed in places an order.** The cart's
  five commands require no one: a guest fills its session's guest's cart.
  `place_order` keeps `requires SignedIn`: an order is a user's, followed
  from any of their sessions, and a guest signs in to place one, its cart
  following it in.
- **A cart's change is told to its user's sessions alone**: telling by
  principal, 5.

### 2. A guest's cart joins its user's at sign-in (Q2)

- **One hook, generic**: `Identity::answer` is given `on_sign_in`, which the
  callback calls with `SignedIn { guest, session, principal }` once the new
  session's principal is opened and before the sign-in is answered, so the
  page the browser asks for next shows the joined cart. The identity knows
  nothing of a store. It is called only where the session the browser came
  with was a guest's: a signed-in session that signs in again is signed out,
  and what it held stays its user's.
- **The join is one store transaction** (`store:host/carts#join`, through
  `Server::store_write`): each of the guest's lines added to the user's cart
  in the guest's order, one item's quantities summed on the user's line, a
  new item appended as the guest's line recorded it (ADR-0172); the guest's
  cart emptied; the user's `UserCartChanged` committed with it, and the
  user's other sessions told.
- **Within each item's bounds**: a quantity is a `PositiveInt` (ADR-0179),
  at least one, so a sum of two is one too; a line holds at most what a
  bigint does (PostgreSQL's `bigint`, Rust's `i64`), and a sum past it is
  refused rather than wrapped, and nothing moves.
- **Run twice, the second finds nothing to join** (`false`, and nothing
  commits).
- **A join that fails leaves the sign-in done** and the guest's lines where
  they were, and the host's log says so. The guest's session is the
  browser's no longer, so those lines are reachable by no one: stated, not
  claimed otherwise.

### 3. A record's scope is its scoped producers', joined

- **`private` gives a record the user's scope**, as `resume.rs`'s
  `manifest_scope`, `check.rs` and `rules.rs` read it. Until 2026-10-09 a
  record only `private` queries produced carried no scope in
  `boundary.rs`'s `TypeFacts`, so a view's parameter of it crossed into the
  public shell unrefused (R-063, R-030 at the user's level): a soundness
  fix.
- **Disagreeing scoped producers are joined.** `TypeFacts` kept the last
  producer's scope alone, by the order of the units and the declarations
  (`HashMap`-free but order-dependent): a record a `session query` and a
  user's query both made carried whichever came later. That was a bug: which
  page could hold the record depended on the order of the files. Joined, it
  holds both scopes, and neither a session's page nor a user's holds it.
- **A type made of several scoped components joins them all**, where the
  first found was taken.

### 4. `user`, a visibility: the user's, and importable (the integrator's ruling)

- **`private` is two things today**: the user's scope (resume, boundary,
  check, rules), and not importable (`resolve.rs`, PW0023). The store's
  `Cart` is the user's and is read from other modules (`views_compose.rs`'s
  pages, which speculate on the store's own `Cart`).
- **`user` is `session`'s peer**: the user's in every rule that reads
  `private` as the user's (`Restriction::User("UserId")`, `Privacy::Private`,
  `User<UserId>`), with no arm of its own that disagrees, and importable as a
  `session` declaration is. A `user page` holds the user's records in its
  manifest (`page_scope` says `user:`).
- **A contextual keyword**: a visibility only at a declaration's start,
  before the declaration's keyword (`query`, `page`, `command`, `fn`,
  `type`, and the rest). A field, a parameter, a binding or an event's
  parameter named `user` is a name, as before (`a_name_user_is_still_a_name`).
- **`private` is unchanged**: the user's and not importable. Whether it
  should stay the user's or mean only "not importable" changes every program
  using it, the feed's notifications and messages among them, and is queued
  as the integrator's.
- **The store says `user`**: `user query Cart`, `user query Order`, and
  `user page` for each page that shows a user's cart or order.
- **Why the store's cart is `UserCart`** (3 and the integrator's answer):
  the library's `session query Cart` (`Resources`) is in the store's build,
  so `Cart` has a session's producer, and joined, a `Cart` a user's query
  answered would hold the session's scope too: no user's page could hold it
  whole. A record of its own, which only the user's query makes, is the
  user's alone. Moving the library's session `Cart` away would have changed
  some twenty fixtures and compiler tests.

### 5. Telling by principal: a user's entry is that user's sessions' alone

- **What a commit's drops reach says whose they were** (`Reached`): a
  private entry whose key position a binding fills with `current_user()`
  names the user the drop names there; any other names no one.
- **`others_reading` tells a user's drop to that user's sessions alone**,
  by `principals.user_of`, the guest model included (a guest's
  `u-<session>`). A session-pinned entry stays its session's, unchanged; a
  shared entry reaches every reader, with its empty set where nothing it
  shows changed (ADR-0219's "sent with no patches too", which a basis and a
  speculation read), unchanged. ADR-0297's live-only derivation and its
  tellers are as they were.
- **The feed's notifications and messages get the same precision**: a
  reader's mark-read reaches their other sessions and sends another user's
  open page nothing.
- It retires ADR-0270's queued "telling by principal".

### 6. A tab whose reader changed is refused (Q3): next

The integrator's ruling, built once the refusal ruling (ADR-0299) merges: the
runtime sends its document's id with each command (`pw-document`), and the
host refuses, before the command runs, one whose document it served to
another session ("You signed in or out in another tab. Reload this page to
go on.") or no longer holds ("This page is out of date. Reload it to go
on."), told where the press was. Not in this ADR's first merge; recorded
here when built.

## Questions to the integrator, and its answers

All 2026-10-09, relayed through the coordinating session.

1. *How a cart and an order become a user's: (a) the program unchanged and
   the layer mapping a session to its owner, or (b) the program keyed by
   `current_user()`?* **(b)**: not (a), which would add the store to
   `identified_by`, which ADR-0270 retires, and leave the program saying a
   cart is a session's. Key the layer's rows by an opaque owner, so the
   benchmark's copy runs unchanged; regenerate what pins the store's text by
   each artifact's own recipe; until telling by principal lands, a cart's
   change is derived for every live session reading `Cart`.
2. *A hook in `identity.rs` for the guest's cart?* **Yes**, generic,
   `on_sign_in(guest_session, principal)`, after the principal is opened and
   before the reply; the join one store transaction, summed, past a bigint
   refused, the guest's emptied, the user's `CartChanged` committed with it;
   idempotent; a failed join leaves the sign-in done and is stated. Built as
   an argument of `Identity::answer` rather than a hook registered on the
   identity: the tests serve a `Server` by value, and a registered closure
   reaching the server would need it behind an `Arc` everywhere.
3. *The stale tab: a predicate the program declares, or the platform's?*
   **The platform's** (6): every program needs it.
4. *`boundary.rs`'s scope, last writer wins, `private` unmapped?* **Map
   `private` to the user's; join disagreeing producers** (not drop the scope,
   which is R-030's whole check); a rejected fixture for the user's level
   (R-063); the precision is the store's to resolve, by a record of its own
   or by moving the library's session `Cart` (4).
5. *`private` means the user's and not importable: a `user` visibility?*
   **Yes** (4), with an accepted and a rejected fixture (A-033, R-064) and
   mutation controls for the parse and the import. R-063 is `private`'s
   case, and R-064 `user`'s: no duplicate.
6. *The superset telling breaks two browser tests (store.spec's part ids,
   transport's frame order): telling by principal, a narrower change to
   ADR-0219's empty sets, or the tests made to ignore other sessions'
   frames?* **Telling by principal, built here** (5). Not the narrower
   change, which is wrong for public entries; not the tests weakened.

## Found

- **The superset accepted in Q1 sent another user's activity to every
  reader.** Under it, user B's open page received a frame, a version and an
  empty set, each time user A changed A's own cart: noise, and a timing
  channel on another user's activity, which the integrator's ruling of Q1
  accepted as "a cost of derivation, not of exposure". It is a correction to
  that ruling: a private entry keyed by a user is that user's sessions'
  alone to be told of (5). Found when the browser suite's concurrent pages
  read one another's frames.
- **A record's scope depended on the order of the files** (3): the last
  scoped producer's, and none for `private`.
- **`private` was two things** (4).

## Alternatives

- **The session mapped to its owner inside the layer** (Q1's (a)): refused,
  above.
- **The library's session `Cart` moved out of the store's build**: some
  twenty fixtures and compiler tests import it as `Resources.{ Cart }`.
- **`Carts` changed to take a user**: the corpus's witnesses and the
  benchmark's copy take a session; their history is kept (CORPUS.md).
- **A hook registered on the identity**: see answer 2.
- **The join in a later request** (the first read of a signed-in session):
  a read would write, and the page asked for first would show the cart
  before it.
- **A store predicate for the stale tab**: refused (answer 3).

## Consequences

- A reader's cart and order follow them across their sessions, and a guest's
  cart follows them in.
- The store's artifacts move: its contracts, its components (`UserCarts.*`
  in place of `domain.line_count` and `domain.subtotal`, which no store
  component reaches now), its WIT, its graph and its plan. Its handlers do
  not: a handler calls its command by id.
- A program may say `user` of a declaration; `private` is unchanged.

## Acceptance

Recorded by `just e14-store-accounts` in
`docs/evidence/E14/store-accounts.txt`, against PostgreSQL 18.6:

- **The compiler**: `tests/user_visibility.rs` (a `user` query imported
  and read on a `user page`, `private`'s refused as not public; a `user`
  query's record refused in a public page's manifest and a session's, and in
  a shared cache as `private`'s is; a field, a parameter, a binding and an
  event's parameter named `user` still names; `user` lowered as a query's, a
  type's and a function's visibility); `tests/boundary_matrix.rs` (a
  `private` query's record the user's; a record a session's and a user's
  query both make held by neither's page, in either order of the units, and
  each held by its own scope's page alone); `boundary.rs`'s own (the join
  refuses what either scope alone would).
- **The corpus**: corpus-check, R-063, A-033 and R-064 (C19).
- **The server**, in memory and with the store on PostgreSQL
  (`tests/store_accounts.rs`, `store.rs`'s joins):
  - two users' carts never share a line: each adds to their own, reads their
    own, their pages show their own, a user's second session holds the same
    cart, a guest's is its own, and the layer answers a user's cart for that
    user's handle alone;
  - a user's cart reaches each of their sessions live, and another user's
    open page is sent no frame at all; a shared entry still reaches every
    reader;
  - a guest's cart follows them in: summed, appended, the guest's emptied,
    shown on the first page after; run twice, nothing moves; a signed-in
    session that signs in again joins nothing, on the canonical store and on
    the benchmark's, which keys its cart by the session;
  - a join past a bigint is refused, the sign-in completes, and the guest's
    rows stay;
  - an order is no other user's to read: by the layer, by page, in each of
    its user's sessions, and none for another user or a guest.
- **Identity's sign-in** (`tests/sign_in.rs`) and **the feed's
  notifications** (`tests/notifications.rs`, mark-read tells no other user's
  page, in memory and on PostgreSQL).
- **`scripts/store_accounts_mutations.py`**: see the report.

## Not claimed

- **The stale tab** (6), with the refusal ruling.
- **Telling by principal for a key that is not the reader's handle**: a
  private entry keyed by a user's id the binding does not fill with
  `current_user()` reaches every reader of it, as before.
- **A failed join's lines** are reachable by no one (2).
- **Several orders a user keeps**: a user's order is their latest, as a
  session's was (ADR-0193).
- **The browser suite on development accounts** (`STORE_ACCOUNTS_PORTS`):
  with 6.
- **`private` made "not importable" only**: the integrator's, queued.

## Report

### What was built

- **The compiler**: `user`, a contextual visibility (`grammar.rs`,
  `lower.rs`, `check.rs`, `rules.rs`, `manifest.rs`, `pw explain`);
  `boundary.rs`'s scope joined and `private`'s the user's. Corpus C19:
  R-063, A-033, R-064, and the library's `Saved`.
- **The program**: `examples/lib/UserCarts.pw` (`UserCart`, its members and
  transitions, `store:data/user-carts`), `UserOrders.pw`,
  `UserCartChanged` and `UserOrderChanged`; the store's `user query Cart`,
  `user query Order` and `user page`s; the cart's commands for anyone,
  `place_order` for a reader signed in.
- **The store's data layer**: rows keyed by an opaque owner, in memory and
  on PostgreSQL (migration `0003_owners`); the cart's and the order's
  operations built once for the session's interface and the user's;
  `store:host/carts#join`.
- **The host**: `Server::owner`, the cart's and the order's entries and
  events by owner; `Identity::answer`'s `on_sign_in` and `Server::joined`;
  telling by principal (`Reached`, `others_reading`).
- **The artifacts** regenerated by their recipes: contracts, components
  (`UserCarts.line_count` and `UserCarts.subtotal` in place of
  `domain.line_count` and `domain.subtotal`), WIT, graph, plan.
- **Tests**: `tests/store_accounts.rs`, `store.rs`'s joins,
  `tests/user_visibility.rs`, boundary tests, a notifications test; the
  expectations of the tests that pin the store's text updated.
- **`scripts/store_accounts_mutations.py`** and **`just
  e14-store-accounts`**, on CI's database job.

### Tests and mutants

- `just e14-store-accounts` (recorded locally, PostgreSQL 18.6): every test
  above green on both layers; **17 of 17 mutants killed** (the recording
  now holds milestone 2's twelve as well, 29 of 29: "a delivery goes to the
  reader's chosen address"), each by a failing
  test, and the memory bound stopped no process. Its first run left one
  survivor, "a signed-in session that signs in again is joined to the next
  user": on the canonical store a signed-in session's guest owner holds
  nothing once the sign-in closes it, so nothing moved. It is killed now by a
  test on the benchmark's copy of the store, which keys its cart by the
  session, where the identity's guard is what keeps a reader's cart theirs.
- The workspace's tests in memory; the server's whole suite, 362 tests, in
  memory and with the store on PostgreSQL; the browser suite in three
  engines at `PORT=7700` (778 passed, the feed's suites' 108 at
  `PORT=7600`), after telling by principal. Before it, the suite failed five
  (Found).
- `just fmt-check`, `lint`, `case-check`, `evidence-gates`,
  `mutation-anchors`, `test-compile` and `audit` green.
- **Re-anchored, not yet run whole here**: `command_retry`, `cross_session`,
  `every_entry`, `identity`, `last_known_good`, `optimistic_transitions`,
  `orders`, `query_values`, `shared_output`, `store_postgres`,
  `test_controls`; and the scripts anchored in `grammar.rs`, `lower.rs`,
  `resolve.rs` and `boundary.rs` and in the telling (`documents`,
  `tell_at_once`, `derived_outside`). CI's verification plans each of their
  recipes, whose files changed, and runs them whole (ADR-0281).

### Merge notes

- From `28c8781`, `origin/master` merged at `51c33f5` (kiokun's 1d,
  ADR-0299): no conflict.
- **Shared files**: the compiler (`grammar.rs`, `lower.rs`, `check.rs`,
  `rules.rs`, `manifest.rs`, `resolve.rs`'s comment, `boundary.rs`,
  `pw-cli`), `identity.rs` (`SignedIn`, `answer`'s `on_sign_in`),
  `main.rs` (`TRACK SEAM (store-accounts)` at the identity's call, the
  tests' module and `cart_by_reader`; the owner, the entries, the join, the
  telling), `store.rs`, `store_pg.rs`, `tests/sign_in.rs` (`pub(super)` for
  the flow's helpers), `tests/notifications.rs` (one test),
  `tests/store_pg.rs` (an owner's rows), `scripts/ci_plan.py`
  (`NEEDS_DATABASE`), the root `justfile` (its import line, the
  `e10-component` list, `e10-build`'s note), `tools/corpus-check`,
  `docs/CORPUS.md`.
- **Codes**: `Owner::StoreAccounts => "PW62"`, no code yet.
- **Ports**: the browser suite here ran at `PORT=7700` and `7600`, outside
  the track's range: at `PORT=7741` its hosts would take 7768, which Spotify
  holds on this machine, and its feed's hosts 7801, which a Python process
  holds. No port in 7741 to 7778 avoids both. CI runs at its default.
- **Not yet built, the rest of milestone 1**: the stale tab (6), after the
  refusal ruling merges.
- **The four status documents are untouched.**
