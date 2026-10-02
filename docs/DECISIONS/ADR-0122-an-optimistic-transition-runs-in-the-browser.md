# ADR-0122: an optimistic transition runs in the browser, and is undone by restoring

Status: accepted under the owner's instruction of 2026-10-02 ("implement
optimistic and idempotency first"). Date: 2026-10-02. Milestones: E14 (it
blocks benchmark task T01), E10's browser backend. Implements ADR-0025.

## Context

ADR-0025 fixed what `optimistic` means:

```text
optimistic Cart(current_session()) as cart => Carts.with_line(cart, item, quantity)

target       which resource ENTRY is speculatively updated
binder       a lexical name for its current value
transition   a PURE function producing the speculative value
command succeeds  -> the authoritative result reconciles it
command fails     -> restore the value held before
```

`pw check` held the clause to that (PW0327, PW0330, PW0331, PW5107). Nothing
executed it (ADR-0120's finding): no backend emitted code for it, and the
count moved only when the server's patch arrived. Three more gaps were found
on the way:

- **`Carts.with_line` returned its cart unchanged.** Its body was `cart`, a
  stub from 2026-08-11. Executed, the speculation would have shown nothing.
- **The backend could not compile the page's own count.** `cart.line_count`
  reads a member function (ADR-0048); the backend knew only record fields.
- **The browser held no values.** A patch carries rendered text. A
  transition is a function of the entry's value, so the page had nothing to
  apply it to, and nothing exact to restore.

## Decision

1. **The compiler emits a speculation module per page**
   (`backend::speculation`, `pw emit-speculations`). For each command a
   handler on the page calls that declares an optimistic clause, it finds
   the page binding showing the targeted entry and compiles, through the
   backend IR and the pure-computation emitter (ADR-0044):
   - the transition, as a function of the binder's value and the command's
     arguments as the handler sends them;
   - each text part reading that binding, as a function of its value, from
     the page's own hole expression;
   - a decoder for the binding's value, by its type.
   The module is fetched on the first press that needs it (E7-L's rule).
2. **The browser holds the value of each entry it may speculate on.** The
   page embeds it (`entries`), and the server sends a new protocol frame,
   `entry_value`, each time that entry advances, only to the subscriber whose
   entry it is. The page is `cache private`; the value is its own session's.
3. **The runtime applies, reconciles and restores.** Before a command's
   request: the transition over the displayed value, re-rendered into the
   parts. Displayed is always `fold(pending speculations, held value)`. A
   commit's answer carries the versions it produced (`basis`); its
   speculation is dropped when the held value reaches that version. A refused
   or failed command's speculation is dropped at once. Either way the parts
   are re-rendered from what remains, so a later press's speculation survives
   an earlier one's rejection, and nothing is restored over a newer value.
4. **`Carts.with_line` is the transition the store states**: a held item's
   line grows by the quantity, and a new item is a new line. The backend now
   reads a member function as a call (`cart.line_count` is
   `line_count(cart)`), by the member table the checker types it with.

## Decisions made here, offered for reversal

- **A new line's price is zero until the server answers** (`domain.unpriced`).
  `add_to_cart` takes no price, rightly: pricing is the server's. A
  speculation cannot know it, and `CartLine.unit_price` is not optional. No
  page renders a price. The alternative is `unit_price: Option<Money<USD>>`.
- **`entry_value` is a protocol frame**, not a field of a patch: a patch is
  text for an address, and a value is not addressed.
- **A commit's answer carries versions, not values.** E6's position that a
  response must not drive the UI (`resource-path.spec.mjs`) stands: the value
  still reaches the page from the resource.
- **The page binding is matched to the clause's target by resource and by
  key, each key an invocation-context call resolving to the same function**
  (`current_session()`). A key from a page parameter is not matched yet.
- **Only parts outside a block are speculated.** A speculated binding read
  inside `{#each}`, `{#match}` or `{#if}` is refused by name, not left stale.

## Found while building

Nine browser tests used "the count shows N" as their evidence that the
server's patch had arrived, and read server-side state next. With the count
moving before the round trip, that is no longer evidence of anything about
the server, and each now waits for the patch itself. The E14 contract
fixture's press waits for its request's answer, in every stack, for the same
reason.

## Acceptance

- `compiler/pw-conformance/tests/speculation.rs`: the store's module, run
  under Node: counts 2, 3 and 6 over three values; the held value unchanged
  by a transition over it; a new item a new, unpriced line; a speculated read
  inside a block refused by name.
- The development server: `a_speculating_page_is_sent_its_carts_value`, with
  the control that a page built without speculations is sent no value.
- `spikes/own-renderer/e2e/optimistic.spec.mjs`, five tests in each engine:
  the count moves while the request is held at the network; a refused and an
  aborted command restore it; two pending presses show two and reconcile;
  one refused among two keeps the other's speculation.
- `scripts/optimistic_transitions_mutations.py`, `just e14-optimistic`.

## Not claimed

- That the speculation is never visibly wrong: a transition the server's
  data layer disagrees with shows its own answer until the server's arrives.
  That is what an optimistic update is.
- Speculation for a part inside a block, or for a key other than an
  invocation-context call.
- Anything for the Marko adapter, which is not the store's renderer.
