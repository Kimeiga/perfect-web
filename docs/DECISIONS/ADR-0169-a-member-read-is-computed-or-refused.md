# ADR-0169: what a template reads through a member function, a host computes or the build refuses

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Charter §15.1's `price`: each menu row shows its item's price,
which the row reads as `item.price.display`.

## Context

A template's values are paths. The renderer reads a value's fields. A member
function read as a property, as `cart.line_count` reads `line_count`, is a
host's to call (ADR-0125): the plan names the member's component, and the
host runs it. ADR-0125 planned one place for it, text at the top of a page
read from a query's value, and refused a query's member read inside a block.

Showing the menu's prices needed a row to read `item.price.display`.
Measured on 2026-10-03, before this ruling:

- **A member read in a loop's row was neither planned nor refused.** The page
  checked and built, and the development server failed at its first render:
  the renderer found no `display` in the item. The plan's own comment said
  such a read "is refused below". Nothing refused it.
- **The plan did not look at values outside text.** These were not recorded:
  - an attribute's value;
  - what an `{#if}`, an `{:else if}` or a `{#match}` decides by;
  - the list an `{#each}` iterates.

  A copy of the store with `title={cart.line_count}` built with exit 0. The
  plan had nothing for the attribute, so the page would fail when rendered.
- **The renderer read one field after a name.** `item.price.display` named
  nothing, even in a row that held it.
- **The development server rendered a menu change's fragment from its own
  list**, not from the `Menu` query's answer that a page renders from. A
  value computed for a row would have been missing from it.

## Decision

1. **A loop's row reads members of its item.** For a loop over a query's
   list, the plan states each path a row reads of its item through a member
   function (`rows`):
   - the query binding the loop iterates;
   - the item's name;
   - the path and its steps.

   A host computes each one for each row before the row is rendered. It runs
   the member's component on what the path reached, and sets the value in
   the row. There is one read for each path, however many parts read it.
2. **Every value a template reads is looked at, not text alone.** The
   lowering records each value it reads outside text:
   - an attribute's value, and each value written in one;
   - what an `{#if}`, an `{:else if}` or a `{#match}` decides by;
   - the list an `{#each}` iterates.
3. **Whether a read calls a member is the value typer's answer**
   (`values::member_reads`). It returns each `.name` that names a function
   declared on its value's type, typed as the checker types the read, and
   each loop whose list is read through one.
4. **A member read no host computes is refused where it is.** The build
   names the part and the path, so the page is not built to fail when it is
   rendered. Such a read is:
   - in an attribute, or in what a block decides by, at the top of a page;
   - in the row of a list that is not a query's;
   - of a signal, which the browser reads by field.
5. **The renderer reads a path field by field**, from the longest name bound.
6. **The store shows each price.** `display(price: Money<USD>)` writes it as
   en-US writes US dollars:
   - `$3.50`, and `$1,234.50`, grouped by thousands;
   - `-$3.50` for an amount owed back;
   - exact for every `Int` of cents, the least included.

   `/` and `%` are Euclidean (ADR-0039), and `Int` arithmetic traps rather
   than wraps. So an amount owed back is written from its quotient and
   remainder by 100, and never negated whole.
7. **A menu change renders its fragment from the query's answer**, read
   again after the change, as a page renders it. The rows before the change
   are what the fragment was rendered from, which is what the open pages
   show.
8. **The data layer prices a cart's line at its item's price.** Until
   2026-10-03 it priced every line at 450 cents, whatever its item. Shown
   beside the menu, a line would have disagreed with its item.

## Alternatives

- **The data carries a formatted string**, as `MenuItem.price_display:
  String`. Every reader would format a price the same way only by agreement,
  and the item would hold a second form of a value it already holds. A
  member is one function, which the type checker checks.
- **Templates as code, evaluated where they render**, as Svelte, Vue and JSX
  evaluate any expression in markup. The renderer would run the program's
  functions on the server and again in the browser. The plan keeps what a
  host computes declared and audited (ADR-0125).
- **The renderer calls the member itself.** `pw-render` renders from values.
  Calling a component would tie it to the engine, and the browser's runtime
  has no engine.
- **The browser formats it with `Intl.NumberFormat`.** The page is whole
  before any script runs (E7). The platform's formatter also takes a float:
  `-9223372036854775808` cents came out as `-$92,233,720,368,547,760.00`.
  The exact amount is `-$92,233,720,368,547,758.08`. Measured with Node on
  2026-10-03, which also gave the forms in decision 6.
- **Refuse every member read but ADR-0125's.** The store needs the row's
  price. A row's read of its item is computed for each row, as ADR-0125
  computes a part.

## Found on the way

- **A test assumed the order in which frames arrive.** `transport.spec.mjs`'s
  "one subscription carries two different resources" read the page's
  entries once, after a rename showed.
  - The cart's count shows the press's speculation (ADR-0122) before the
    cart's frame comes, so that frame can come after the menu's.
  - In Firefox, under the full suite's load, the test read one entry.
  - It now waits for both.

## Acceptance

- `compiler/pw-core/tests/row_reads.rs`:
  - A row's read is planned once, from text and attributes alike.
  - What a row decides by, and a list it iterates, are computed for it.
  - Refused at the top of a page: an attribute, a value written in one,
    `{#if}`, `{:else if}` and `{#match}`.
  - Refused in a list that is not a query's, in text and in an attribute.
  - A signal's member is refused.
  - Controls: fields anywhere, and ADR-0125's part.
- `runtime/pw-render/tests/paths.rs`:
  - a value two fields deep;
  - a whole path read before its name;
  - a missing field is missing, not empty;
  - a row whose computed value changed is set in place.
- `runtime/pw-host/tests/display.rs`: the compiled `domain.display`, from
  nothing to the least `Int`, against the forms decision 6 gives.
- The development server's tests:
  - each menu row shows its price, each store's its own;
  - an item E7-P inserts arrives with its price;
  - a menu change reads that store's menu once, through its cache, and
    drops no other store's.
- `e2e/stores.spec.mjs` and `e2e/store.spec.mjs`, three engines: each price,
  and an inserted item's.
- The browser suite: 529 passed, 2 skipped.
- `scripts/row_reads_mutations.py`: 23 mutants, recorded by
  `just e14-row-reads`.

## Not claimed

- **A row of a list that is not a query's.** This covers a loop over a field
  of its item, and a loop over a stream's answer. Its member reads are
  refused.
- **A member read at the top of a page outside text.** This means an
  attribute, or what a block decides by. It is refused. A top-level part is
  planned as text (ADR-0125), and a host patches it as text.
- **A signal's member**, which the browser would have to call. Refused.
- **Other locales and currencies.** `display` is `Money<USD>`'s, in en-US.
