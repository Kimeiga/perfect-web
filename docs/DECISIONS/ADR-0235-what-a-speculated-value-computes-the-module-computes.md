# ADR-0235: what a speculated value computes, the page's module computes

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. Ruling 0073-a for a speculated value's block subject and the
values inside its block, and a gap found beside it. The feed's thread page
needs both for an optimistic reply: "No replies yet." is decided by
`List.length(thread.replies) == 0`.

## Context

- **A text part computed from a speculated value whole is the module's**
  (ADR-0228). The browser computes it from the value a press speculates, and
  sets it in place.
- **A block's subject computed from one was refused**: "computes a block's
  subject from `thread`, which the page speculates on" (ADR-0229).
- **One computed inside a block was skipped.** Found with a session's thread,
  a count at the top of the page and the same count inside `{#if flag}`,
  `flag` another query's value:
  - the module computed the count at the top;
  - the one inside the block was no part, no region, and nothing refused it;
  - a press would have shown three replies at the top and two inside the
    block, until the server answered.
- **A region the browser renders is given the speculated value and the
  signals** (ADR-0172). A value computed inside it, read by the path the
  compiler names (ADR-0226), was not among them.

## Decision

1. **A block whose subject is computed from a speculated value whole is a
   region** of the speculation, rendered again as a block the value decides
   by a path is.
2. **What a region computes from the value is the module's.** That is its
   subject, and each value computed inside it. Each is compiled into the
   page's module, exported by the path the template reads it:
   `export const computed = { thread: { "#m.P~2": f2 } }`.
   - The browser computes each from the speculated value. It gives them to
     the renderer with the value and the signals, as it gives a block the
     browser decides what the page computes from its signals (ADR-0229).
3. **A value computed from a speculated value inside a block no region
   renders is refused by name**, as a value read there is (ADR-0172): "computes
   a value from `thread` inside a block, which a speculation would not
   reach".
4. **An attribute's value computed from a speculated value is still
   refused**, and so is one computed from a field of the value. The message
   now names what the browser does compute: "a text part or a block's subject
   from the value whole (ruling 0073-a)".

## Acceptance

- **`compiler/pw-core/tests/speculated_values.rs`, 2 tests**, each module run
  under Node:
  - a block decided by `List.length(thread.replies) == 0`, holding the count,
    is a region. Its subject and the count are the module's: false and 1 for
    a thread with one reply, and 2 after the reply a press speculates;
  - the count inside a block another query's value decides is refused by
    name; the control, inside the block its own subject decides, builds.
- **ADR-0229's `computed_conditions.rs`**: its refusal of a subject computed
  from the home page's `feed` is gone, since that subject now builds.
  **ADR-0228's `computed_rows.rs`**: the attribute's refusal names what the
  browser computes.
- **`scripts/speculated_values_mutations.py`: 5 mutants**, recorded by `just
  e14-speculated-values`.
- **Two older mutants re-anchored**:
  - `computed_rows_mutations.py`'s "an attribute computed from a speculated
    value is built";
  - `cart_lines_mutations.py`'s "a region's read of what the browser does
    not hold is not refused".
- **Run whole, their tests or anchors changed**: `computed_rows_mutations.py`
  12 of 12 killed, `computed_conditions_mutations.py` 12 of 12.
- **The workspace, 2,112 tests; the browser suite, 773 in three engines.**

## Not claimed

- **In browsers.** No page of the feed speculates on a value a block's
  subject is computed from until the thread page's optimistic reply (ruling
  0122-d), whose browser test this is. The renderer's call is the one a block
  the browser decides by a computed subject takes (ADR-0229).
- **An attribute's value computed from a speculated value**, or one computed
  from a field of it.
