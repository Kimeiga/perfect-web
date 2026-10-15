# ADR-0337: a read is bounded by construction

Status: accepted under the owner's delegation of 2026-10-02, by the
integrator's ruling of 2026-10-14 (below); a design note by W9 for NEXT 4c
(track `bounded-reads`), merged on train-8. Docs only: nothing here is
built yet. Date: 2026-10-14. Builds on ADR-0207 (a data source states what
it guarantees), ADR-0101 (a declaration's inferred row), ADR-0125 (a page
shows what its queries return), ADR-0218 (a host serves any program; its
data is the deployment's), and NEXT 4b (the store's set-oriented `Rows`
methods and their statement counts), whose measurements this note will
carry before it is ruled on.

## Ruling (the integrator, 2026-10-14)

Accepted under the owner's delegation of 2026-10-02: a read keyed by a
loop's element is refused above the seam by the program's text (PW0405),
its set read the repair; below it each source operation states what one
call costs (`sets`) and is measured, and a budget that grows on a source
that does not batch is warned of (PW0406); the store's composites move
above the seam. The two cases below (Measurements) are settled as they
say. Built in this order, each its own track:

1. **The render merge** (track `render-merge`): one snapshot per render,
   its calls run in one read transaction, and each identical operation
   call made once within it (the second case). The host's alone; no
   program changes.
2. **PW0405, `set` pairs, `sets` and PW0406** (track `bounded-checker`):
   decision 1's refusal and decision 2's statement, with the corpus
   checked against them (decision 4).
3. **The conformance suite**, which is one suite with the three-database
   note's decision 5.5 (ADR-0338, ruled 2026-10-15): written once against the
   `DataLayer` seam, run first against today's hand-written layers (memory
   and PostgreSQL, at two data sizes), and generated for every declared
   table later.
4. **The store's composites moved above the seam**, one at a time
   (decision 3's table), each measured before and after by `Counted`; the
   first case's savings come with them.

## Context

- **The owner asked that N+1 be impossible by construction, whatever the
  database.** Today it is only measured. The integrator's N+1 finding of
  2026-10-12 found it in the store's host operations, not in its programs:
  `hours#for-stores` and `hours#for-cart` run a statement per store, the
  search's reach runs a coverage read per store, and `checkout#parts` maps
  a cart's lines to their stores by reading every store's menu. The
  KNOWN_LIMITATIONS lines (ADR-0328, ADR-0329) say so.
- **Above the seam, the programs are already set-shaped, but nothing makes
  them be.** `StoresOpen` reads every store's hours with one call:

  ```
  let stores = Stores.list()
  let hours = hours_of_stores(List.map(stores, s => s.id))
  List.map(stores, s => listed(s, hours))
  ```

  and the checkout reads a cart's stores' fees with one
  (`fees_of_stores(List.map(parts, p => p.store))`). The integrator found no
  program in the store or the feed that reads per element today. But
  `List.map(stores, s => listed(s, hours_of(s.id)))` checks as well as the
  set read does, and runs one host call per store.
- **Below the seam, what a call costs is unstated.** A host operation is a
  name and a signature (ADR-0218). Nothing says whether `fees#for-stores`
  is one statement or one per store; 4b measured it, per layer, by hand.
- **What others do.**
  - Rails' `strict_loading` "will raise an error if the record tries to
    lazily load an association" (`ActiveRecord::StrictLoadingViolationError`;
    Rails API, `QueryMethods#strict_loading`, read 2026-10-14). A refusal
    whose repair is the set read (`includes`).
  - DataLoader coalesces "all individual loads which occur within a single
    frame of execution" (a tick of the event loop) and calls the batch
    function "with all requested keys" (graphql/dataloader README, read
    2026-10-14). Merging at run time, per tick.
  - Haxl (Marlow, Brandy, Coens, Purdy, "There is no Fork: an Abstraction
    for Efficient, Concurrent, and Concise Data Access", ICFP 2014, pp.
    325-337) batches the fetches an applicative computation makes in one
    round, so the rounds are the data-dependency levels, not the fetches.
- **What each database gives for a set read**, read in each vendor's own
  documentation on 2026-10-14:
  - **PostgreSQL**: `expression operator ANY (array expression)`: "The
    left-hand expression is evaluated and compared to each element of the
    array" (Functions and Operators, 9.25.3). One statement for any number
    of keys.
  - **DynamoDB** `BatchGetItem`: "A single operation can retrieve up to 16
    MB of data, which can contain as many as 100 items"; more than 100 is a
    `ValidationException`; a partial result returns `UnprocessedKeys`, to
    retry "starting with the next item", and DynamoDB "strongly recommend[s]
    that you use an exponential backoff algorithm" (API Reference,
    BatchGetItem). So at least ⌈n/100⌉ calls, more when a response passes 16
    MB or is throttled.
  - **Firestore** `documents.batchGet` "Gets multiple documents", taking a
    list of names, its response "The streamed response for
    `Firestore.BatchGetDocuments`"; the page states no maximum (REST
    reference, projects.databases.documents.batchGet). A query's `in`
    combines "up to 30 equality (`==`) clauses on the same field" (Query
    data, Simple and compound queries). So by name, one call; by a field,
    ⌈n/30⌉ queries.
  - **Cassandra**: "The `IN` relationship is only allowed on the last column
    of the partition key or on the last column of the full primary key"
    (Apache Cassandra, CQL, DML, SELECT's WHERE clause); DataStax's guide
    says "The IN operator can impact performance on medium-large datasets"
    and that "There are cases in which the IN operator should not be used"
    (DataStax CQL for DSE 6.9, Querying). Neither states the coordinator's
    work per partition. So a source on Cassandra states one client call per
    key, issued concurrently, until a measurement says otherwise.
  - So a set read's cost is the source's to state. It is one statement on
    PostgreSQL, a function of n on DynamoDB, and per key on Cassandra.

## Decision

### 1. Above the seam: a read whose key comes from a loop's element is refused

Both diagnostics come from the inferred effect row, so they are Effects'
(PW04xx, beside PW0400 an undeclared effect, PW0401 a forbidden one and
PW0402 a wrong frame phase): PW0405 and PW0406 (the integrator's
assignment, 2026-10-14).

- **The rule (PW0405).** A call that performs a source read, a `database.read<R>`
  in its inferred row (ADR-0101) reached through a function bound by a
  `host` policy, is refused where both hold:
  1. **it is inside a loop over data**: inside a lambda handed to a `List`
     combinator (`List.map`, `List.fold`, `List.filter`, `List.find`,
     `List.any`, `List.all`, `List.sort_by`, or any function whose
     parameter is a function and which the call graph shows applying it to
     a list's elements), inside a `for x in xs` body, or in a function on a
     cycle of the call graph whose recursive call is passed part of its own
     argument (a recursion over data); and
  2. **its key comes from the loop's element**: an argument that names the
     lambda's parameter, the `for` pattern's binding, or the recursive
     parameter, directly or through a `let`, a field, or a call of a pure
     function on it.
  A read inside a loop whose key does not depend on the element (the same
  store's hours, read in a loop over its periods) is not refused: it is one
  key, and the host's cache answers the repeats.
- **The repair is the set read.** The diagnostic names the set-shaped
  operation where the source declares one (decision 2), and shows it, here
  on `StoresOpen` as it would be were it written per store:

  ```
  error[PW0405]: `hours_of` reads `Stores` once for each element of `stores`
    --> examples/store/hours.pw:392:41
     |
  392 |     List.map(stores, s => listed(s, hours_of(s.id)))
     |                                     ^^^^^^^^^^^^^^ one read per store
     |
     = note: its key `s.id` comes from `s`, the element `List.map` passes
     = help: read them together, before the loop:
             let hours = hours_of_stores(List.map(stores, s => s.id))
             List.map(stores, s => listed(s, hours))
  ```

  Where the source declares no set read for the operation, the help says so
  ("`hours_of` has no set read; its source can declare one") and the
  refusal stands: a read per element is the thing refused.
- **How the checker finds it, in Pleris's HIR.** Two analyses that already
  exist answer each half; the rule joins them and recomputes neither
  (ADR-0022: record, never recompute):
  - **"a source read"**: `effects.rs`' inferred row, with the span of each
    effect's source (`Inferred.sources`), already says which call performs
    `database.read<R>`, through every call it makes.
  - **"inside a loop"**: `effects.rs`' `enclosing_callback` already finds the
    lambda handed to another function that contains a call (by span, as HIR
    has no parent links). `Expr::For` is the loop syntax; `recursion.rs`
    finds recursive types, and the call graph `effects.rs` walks for rows
    gives a function's cycle.
  - **"its key comes from the element"**: new, and small. The lambda's
    `params` and the `For`'s `pat` are `PatternId`s; `lexical.rs` says which
    binding each local name means (ADR-0063), a lambda's parameters and a
    `for` loop's among them. The rule walks the
    read's arguments to the names they mention, following a `let` to its
    initializer, and asks whether any resolves to a loop's binding.
- **Later, the compiler rewrites instead of refusing** (Haxl's batching, by
  the compiler): a `List.map` whose lambda's only read is keyed by the
  element becomes the set read before the loop and a lookup inside it, as
  the help above shows, and the rounds are the data-dependency levels. On
  the checkout's real code (`checkout.pw`'s `charged_of`), with a keyed
  `fees_of` declared `set fees_of_stores`, the program as written:

  ```
  List.map(parts, part => part_charged(part.store, subtotal_of(part.lines), fees_of(part.store), promo_of(promo, part.store)))
  ```

  is rewritten into the store's own text today, one read before the loop
  and the lookup it already has (`fees_for`) inside it:

  ```
  let fees = fees_of_stores(List.map(parts, p => p.store))
  List.map(parts, part => part_charged(part.store, subtotal_of(part.lines), fees_for(part.store, fees), promo_of(promo, part.store)))
  ```

  The lookup is the pair's to supply: `set` names the set read, and a
  generated lookup by the key finds each element's result in its answer.
  Not in 4c: refusal first, so every program states its set reads, and the
  rewrite is a step whose output the refusal already accepts.
- **The host merges what it loops over itself.** A page's sibling bindings
  and its instances (ADR-0125) are the host's loop, not the program's: the
  host gathers the reads one render asks for, keyed alike, into one set
  read, as DataLoader does per tick. **Not inside a component**: a
  component that awaits each read before the next issues them one round
  trip apart whatever the runtime's async support (Wasmtime's
  component-model async ABI is opt-in and its builtins "very incomplete",
  Config docs, read 2026-10-14), since the next key may depend on the last
  answer. The program states the set read; the host does not guess it.

### 2. Below the seam: an operation states what one call costs

- **A source states each keyed read's set-shaped form and its cost**, as
  ADR-0207's guarantees are stated:

  ```
  source StoreData
      holds        Stores, Menus, Carts, Orders, Addresses
      transactions serializable
      reads        strong, snapshot, read_your_writes, eventual
      changes      none
      sets         one_call
  ```

  `sets` is `one_call` (PostgreSQL's `= ANY($1)`, Firestore's `batchGet`
  by name), `chunked(k)` (DynamoDB's `BatchGetItem`, k = 100: at least
  ⌈n/k⌉ calls; Firestore's `in`, k = 30), or `per_key` (Cassandra, a source
  that cannot batch). A clause left out guarantees nothing: `per_key`.
- **A host function pairs its keyed read with its set read**, so the
  checker and the rewrite know them:

  ```
  fn hours_of(store: StoreId) -> Result<OpeningHours, StoreError> !{ database.read<Stores> }
      host "store:data/hours#for-store"
      set  hours_of_stores
  ```

  `set` names a function of the same source whose parameter is a list of
  the keyed read's key and whose result is a list of the keyed result with
  its key. The checker holds the pair to that shape.
- **The seam stays one call per named operation.** Only keyed reads become
  set-shaped; there is no universal `read(keys) -> map`. An operation is
  what the host links a component to (ADR-0218), and its name is what a
  contract, a grant and a test say.
- **A conformance suite measures each layer.** 4b's method, generalized:
  for each set read a layer provides, the suite runs it through the layer's
  counted connection (`store_pg.rs`' `Counted`, ADR-0308 amended) at two
  data sizes, and asserts the calls the source's `sets` states: the same
  count at both for `one_call`, ⌈n/k⌉ for `chunked(k)`. A layer whose
  measured count grows where it states `one_call` fails the suite. Every
  layer runs it: memory, PostgreSQL, and any a deployment adds.
- **The budget.** ADR-0207's declared guarantees gain the set-read cost. A
  page's or a command's budget is its calls, each times its source's cost:
  a set read on a `one_call` source is 1; on `chunked(100)`, ⌈n/100⌉; a
  `per_key` source's set read is n.
- **PW0406**: the checker reports a page or a command whose budget grows
  with the data on a source that cannot batch (`per_key`), naming the read:

  ```
  warning[PW0406]: `StoresOpen` reads `Stores` once per store on `Ledger`, which cannot batch
    --> examples/store/hours.pw:391:17
     = note: `Ledger` states `sets per_key`: a set read is a call per key
     = help: hold `Stores` in a source that batches, or bound the list
  ```

  A warning, not a refusal: a source that cannot batch is the deployment's
  choice, and the program is right as written.
- **Evidence.** The compiler emits each page's and each command's bound
  (its calls by source, and each one's cost) beside its plan, and the
  server's tests check it against `Counted`'s count of what the page
  actually ran, per source operation as 4b counts them.

### 3. The store's composites move above the seam

The architect's ruling of 2026-08-20 expects the store's operations "to
become compiled Pleris over narrower platform primitives" (as the integrator
cites it; `docs/EVIDENCE_LEDGER.md`'s E10-S10, "Replace the host's
`store:data/carts` with compiled Pleris over narrower primitives"). These
composites move, each to the primitives named, each primitive a keyed or
set read the conformance suite measures:

| Composite (host today) | What it does in Rust | Moves to (Pleris) over primitives |
| --- | --- | --- |
| `search#find`, `#find-for`: the scan | `search::scan`, every store's menu matched on the host (memory) | the match in Pleris over `menus#matching(patterns)`, a set read (PostgreSQL's two statements, ADR-0308) |
| `search#find-for`: the reach | `store::coverage` per store found, a read per store | `coverage_of_stores(stores, reader)`, a set read: the reader's chosen place once, the stores' zones by set |
| `hours#for-cart` | each line's store by `store_of`, every store's menu read, then hours per store | `carts#stores(reader)`, one statement (cart lines joined to their items' stores), then `hours_of_stores` |
| `checkout#parts` | the same `store_of` per line | `carts#stores(reader)`, and the grouping in Pleris |
| `carts#add`: the price | `catalog_item(item)`, the item's name and price read on the host for the line | `menus#items(ids)`, a set read, and the line made in Pleris (E10-S10) |
| `user-orders#place`: the reach | `cart_reaches`, a coverage read per store of the cart | `coverage_of_stores`, as the search's reach, checked in `place_order` as its hours are |
| `coverage#for-store`, `#reaches` | the rule `places::delivers_to` on the host | the rule a pure Pleris function over `addresses#chosen(reader)` and `stores#zones(stores)` |

What stays below the seam: a single keyed read or write of one resource,
and its set form. `hours#for-stores` stays, as a set read, one statement
after 4b.

### 4. Migration

- **No program changes.** The integrator found no read keyed by a loop's
  element in the store's or the feed's programs; PW0405 is added with the
  corpus checked against it (`corpus-check`, the accepted fixtures), and a
  rejected fixture for each loop form (a `List` combinator's lambda, a
  `for`, a recursion over data) with its repaired control.
- **Sources state `sets`.** `StoreData` states `one_call`. A source that
  states nothing is `per_key`, so PW0406 tells the programs on it where
  their budget grows; none does today.
- **The pairs.** Each keyed host read with a set form names it (`set`):
  `hours_of` / `hours_of_stores` today; the rest as the composites move.
- **The composites move one at a time**, each its own track, each measured
  before and after by `Counted`.

## Alternatives

- **Merge reads at run time inside a component** (DataLoader per tick). A
  loop that awaits each answer before its next key issues the reads apart;
  merging needs the program to state the keys together, which is the set
  read.
- **The rewrite first** (Haxl by the compiler). It hides the read a reader
  of the program would want to see, and a rewrite that cannot apply (a key
  computed from the last answer) would leave a loop of reads unflagged.
  Refusal first makes the set read the program's own text.
- **A universal `read(keys) -> map`.** Every operation would become a
  key-value read, losing what an operation's name and signature say (a
  search, a cart's hours), and what a contract grants by name.
- **Count only, refuse nothing** (today). A count catches a regression a
  test exercises; it cannot make one impossible.
- **Refuse any read inside a loop.** A read whose key does not depend on the
  element is one key; refusing it would refuse correct programs.

## Acceptance (when built)

- `compiler/pw-core/tests/bounded_reads.rs`: PW0405 for each loop form, a
  key through a `let` and a field, and its control (a key not from the
  element; the set read); PW0406 on a `per_key` source and its control on
  `one_call`; the pair's shape refused where it does not fit.
- The conformance suite over memory and PostgreSQL, each set read at two
  data sizes.
- The emitted bound of each store page and command, checked against
  `Counted` in the server's tests.
- A mutation script, each rule's mutant killed.

## Not claimed

- **The rewrite** into one set read: a later step.
- **A recursion over data whose reads are keyed by computed values** the
  checker cannot trace to the element: refused only where the trace is
  syntactic (a name, a `let`, a field, a pure call).
- **Writes**: a write per element (`for x in xs { insert(x) }`) is the next
  question; this note bounds reads.
- **A source's cost under throttling**: DynamoDB's `UnprocessedKeys` and
  backoff make ⌈n/100⌉ a floor, not a bound.
- **Cassandra's internal fan-out**: a `per_key` source is charged its client
  calls; what a coordinator does per partition is the source's to measure.
- **The host's merging across pages or sessions**: one render's reads only.

## Measurements (4b)

NEXT 4b (track `bounded-statements`, ADR-0336) counted every operation of
the store's data layer, and every page and command, on PostgreSQL 18.6 at
the seed and with fifty stores more, each count equal at both and pinned
(`tests/bounded_statements.rs`). These are the numbers the budget composes
from, and the first conformance suite's (decision 2): "operation X makes k
statements per call, at both sizes". Memory runs no statement; its count
is not a cost.

- **Per operation**, statements per call, 63 operations: `stores#get` 1,
  `stores#list` 1, `menus#for-store` 2, `search#find` 2, `search#find-for`
  4, `coverage#for-store` 2, `hours#for-store` 3, `hours#for-stores` 3 for
  every store at once, `hours#for-cart` 6, `fees#for-stores` 1 for every
  store at once, `checkout#parts` 2, `user-orders#await-payment` 14. A
  write's count grows with what it writes (the cart's lines, a week's
  periods, a menu's items), never with the stores. So `StoreData` states
  `sets one_call`, measured.
- **Per page and command**, each command with its commit (the outbox's
  inserts, the interaction's record, the outbox read back): `add_to_cart`
  10 and 11, `add_address` 6, `choose_tip` 6, the home page 5, the store's
  page 16, the cart 13, a search 8, the address page 2, the order's page 5.
- **`place_order`, by scenario** (the integrator's ruling of 2026-10-14,
  both pinned): (a) a deployment taking no payments, placed at once, its
  operations 33, its commit 7, its queries read again 21; (b) a deployment
  taking payments, awaiting, its transaction 37 (its sweep scheduled, 2,
  since ADR-0334), then `settle_payment` on the provider's word in its own,
  17 (the sweep cancelled, 1); `cancel_order` 25 and the provider's word of
  its cancel 14; the sweep run thirty minutes on, `expire_order` 26 and the
  word of its cancel 12. (b) is the production path, (a) the
  development store's. "The development provider settles inside
  `place_order`" was wrong: the one transaction 4b measured first is
  ADR-0332's path for a deployment that takes no payments, where
  `begin_payment` answers `NotTaken` and `place_order` settles its order
  inline, in its own transaction.

### The first case: one command's repeated reads (the integrator's)

`place_order` in scenario (a), by operation (the layer's tally per
operation and per transaction; its commit counted with it since 4b's ruling
of 2026-10-14):

| Where | Reads | Statements |
| --- | --- | --- |
| its transaction: `closed_in(hours_of_cart(..))` | hours#for-cart: the cart, its items' stores, their names, their hours (3) | 6 |
| its transaction: `priced_for` | checkout#parts (the cart, its items' stores), fees#for-stores, checkout#chosen | 4 |
| its transaction: `UserOrders.await_payment` | user-orders#await-payment: the cart again, its reach (the addresses, the items' stores, the stores), the order, its lines and parts, the code, the choices cleared | 16 |
| its transaction: no payments taken, the order settled at once (ADR-0332) | user-orders#settle | 7 |
| its commit | the outbox's inserts, the interaction's record, the outbox read back | 7 |
| after the commit: the session's invalidated queries read again | `Cart` six times (user-carts#current), `CartHours`, `Checkout`, `Order`, the estimate | 21 |

In scenario (b), the production path, `place_order`'s transaction (37)
has no settle among its operations; `settle_payment` runs later, on the
provider's word, in its own (17).

In one transaction the cart's lines are read three times and their items'
stores three times, each by a composite that reads them for itself.
**Settled (the integrator agreed, 2026-10-14): read once above the seam,
not coalesced by the host per transaction.** The composites move (decision
3): `place_order` reads the cart's parts once (`carts#stores`, the lines
with their stores), and computes the hours from them
(`hours_of_stores(parts' stores)`, the three statements of the hours
alone) and the reach (`coverage_of_stores(parts' stores)`, the addresses
and the stores' rows), and `await_payment` takes the parts it was given
rather than reading the cart again for its reach. A per-transaction cache
in the host would need to know which writes of the transaction invalidate
which keyed read, a second model of the data beside the program's, and
would hide the repeated read the program wrote.

- **The number**: the transaction's operations, 33, become 28:
  `hours#for-cart`'s 6 becomes the hours' 3 (−3); the reach inside
  `await-payment` no longer looks up the items' stores (−1); and
  `await-payment` writes the lines it was given instead of reading the cart
  again (−1). Estimated from the measured breakdown, confirmed when the
  composites move.

### The second case: one render's identical calls (the integrator's)

After `place_order`'s commit the session's invalidated queries are read
again, and one render calls `user-carts#current` six times: once for each
document and binding that reads `Cart`. **Settled: within one render the
host makes each identical operation call once** (the operation and its
arguments as the key) and gives every binding that asks the same answer.
Nothing is kept past the render, so no invalidation model is needed; and
the render's calls run in **one read transaction**, so a render sees one
snapshot, where today each call opens its own and two bindings can
straddle a commit.

- **The number**: `Cart`'s 6 becomes 1, the commit's re-reads 21 become 16,
  measured by the Tally when built.
- **The control**: two bindings that call one operation with different
  arguments still read twice.
- **First of the two**: the render merge, the cheaper win (the integrator's
  view, which this note takes): it is the host's alone, and no program
  changes. The first case waits on the composites moving above the seam.

With both, `place_order`'s 61 in scenario (a) (33 + 7 + 21) becomes 51
(28 + 7 + 16). In scenario (b) the first case's savings are the same reads,
in `place_order`'s 37.
