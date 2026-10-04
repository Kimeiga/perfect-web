# ADR-0170: a loop over a list inside a query's value, and a part a speculation would not reach

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. The first step of the audit's fourth gap (§15.3): a cart that
lists its lines iterates `cart.lines`, a list inside the cart's value.

## Context

The benchmark's store was measured on 2026-10-03, with this added:

```text
{#each cart.lines as line (line.item_id)}<li>× {line.quantity.count}</li>{/each}
```

- **The plan recorded the binding, `cart`, as the list.** At the cart's first
  change the development server asked for `cart` as a list, found a record,
  and could not show the session's document ("`cart` is not a list").
- **A row's member read was planned only for a loop over a binding itself**
  (ADR-0169). `line.quantity.count` was refused as a read no host computes.
- **A value computed for a row was set inside the row's fields** (ADR-0169).
  `quantity` is a number, and a number has no field to hold `count`. The
  server rendered the row without it ("MissingValue `line.quantity.count`").
- **The speculation module looked at text alone.** The store's `cart` is
  speculated by `add_to_cart`'s `optimistic` clause (ADR-0122). These parts
  would have kept the value the page held while the count showed the
  speculated one:
  - a loop over the cart's lines;
  - a block that decides by the cart;
  - an attribute that reads it.

  Nothing said so. Two of the server's own tests served such a page:
  ADR-0146's block, `{#if cart.lines}`, beside the speculated count.

## Decision

1. **A list a loop iterates is recorded by its path**, `menu` or
   `cart.lines`. A host renders it again from the binding's value, at that
   path (ADR-0145).
2. **A row of such a list reads members of its item**, as a row of a query's
   list does (ADR-0169).
3. **A value a host computes for a row is set in it whole, by its path from
   the row**, as `quantity.count`. At each record, the renderer reads the rest
   of a path whole before it steps into a field. No field's name holds a `.`,
   so the two cannot meet.
4. **The speculation module refuses a part it does not render again that
   reads a speculated value.** Such a part is an attribute, what a block
   decides by, or a loop's list. The page would otherwise show two values of
   one thing at once, which ADR-0168's rule forbids: a change reaches every
   part that reads it.
   - ADR-0146's block tests now serve the store without `add_to_cart`'s
     speculation, which those tests are not about.

## Alternatives

- **Record the binding, and let the host find the list by the loop.** The
  host would search the template for what the plan exists to state.
- **Set a computed value inside its receiver**, as ADR-0169 did. It works
  where the receiver is a record, as `price.display` is. It has nowhere to go
  where the receiver is a number or a string.
- **A map of computed values beside each row.** It is one more structure
  that the renderer and every host carry, for what a dotted key says.
- **Let a speculation leave such a part as it was.** The count would say 2
  while the list said 1, for as long as the command takes: the
  inconsistency a speculation exists to hide. Rendering such a part again in
  the browser is the next ruling's.

## Acceptance

- `compiler/pw-core/tests/nested_lists.rs`:
  - a list inside a value is recorded by its path, and its rows' member
    reads are planned;
  - a loop's list, a block's subject and an attribute that read the
    speculated cart are refused;
  - the store as it is builds.
- `runtime/pw-render/tests/paths.rs`: a value computed for a row is read
  whole where a field has none, and set in place when it changes.
- The development server's tests: a list inside a query's value is rendered
  and patched where it is. It is inserted, and then its count is set in its
  instance.
- `scripts/nested_lists_mutations.py`: 6 mutants, recorded by
  `just e14-nested-lists`. ADR-0169's two mutants whose code moved are
  anchored again, and are still killed.

## Not claimed

- **A speculation reaching a loop, a block or an attribute.** This ruling
  refuses one. The browser rendering one again is the next ruling's, and the
  store's per-line cart waits on it.
- **A list inside a shared query's value.** It is rendered with the document
  and not again, as before. Only the menu's public fragment is patched for
  every reader (E7-P).
