# ADR-0294: a list renders in its length

Status: accepted under the owner's delegation of 2026-10-02, on W6's
finding of 2026-10-09 (kiokun's whole-dictionary sample). Date: 2026-10-09.
Milestone: E14.

## Context

- **W6's finding.** A sample of 5,938 of kiokun's words, each served over
  the server's HTTP path, gave p50 2.3 ms and p99 13.8 ms, and a tail of
  311 ms at さえこ, a word with 128 Japanese names. Its entry was read and
  decoded in 0.5 ms and its names computed in 1.3 ms; rendering them took
  about 215 ms for 163 KB of HTML. Rendering fewer of the same names, 32
  took 16.7 ms, 64 took 57.7 ms and 128 took 218 ms: about four times for
  each doubling.
- **The cause** (`runtime/pw-render/src/lib.rs`): a loop's item renders in a
  scope, `Env::with` for its binding and `Env::within` for its frame, and
  each cloned the whole `Env`: every binding's full value, the page's query
  value and so the list being looped with it, and every materialized
  fragment's HTML. A list of `n` items over a page of size `m` rendered in
  `n × m`, and the list is part of the page.
- **Why now.** kiokun's next sections hold lists of up to 200 items, three
  to a page; by the same curve a page would take seconds.

## Decision

1. **An item's scope shares the page's values.** `Env` holds each binding's
   value by `Arc`, and its capabilities, fragments and settled streams by
   `Arc` too. Entering a scope copies the map of bindings, pointers only,
   and adds the item's. A value is never copied to enter a scope.
2. **A list renders in its length**: `n` items in about `n` times one
   item's work, whatever else the page holds.

## Alternatives

- **A chain of scopes, each holding its own binding and its parent**: also
  linear, and with no map to copy, but every lookup walks the chain and
  every reader of `Env` changes. The map of pointers keeps the lookup as it
  was, and a scope holds a page's few bindings.
- **A persistent map** (a crate): a download for what `Arc` gives here.

## Acceptance

- **`runtime/pw-render/src/lib.rs`, `scopes`**: an item's scope shares the
  page's values, fragments, settled streams and capabilities by pointer,
  and its own binding is its own.
- **`runtime/pw-render/tests/list_scale.rs`**: every name rendered once and
  in order; and, timed (`--include-ignored`), eight times the names take
  less than 24 times as long. Here, with the fix, 250 names took 3.7 ms and
  2,000 took 31.2 ms, a ratio of 8.3, and 32 to 1,024 names doubled with
  each doubling (0.47 to 15.8 ms). With scopes copying again, 2,000 names
  took 7.1 s, a ratio of 62.0, and 1,024 took 2.67 s.
- **`scripts/shared_scopes_mutations.py`**: a scope that copies the page's
  values, and one that copies its fragments, each fail the tests.
- `just e14-shared-scopes` records all three.

## Not claimed

- **kiokun's sample is not re-measured here.** W6 records it again at low
  load against this ruling, beside the run that found it.
- **An item's own value is still cloned into its scope**, once an item:
  linear in the list, as the output is.
