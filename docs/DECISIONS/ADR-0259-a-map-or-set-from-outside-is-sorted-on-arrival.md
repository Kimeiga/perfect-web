# ADR-0259: a map or set from outside is sorted on arrival

Status: accepted under the owner's delegation of 2026-10-02; the owner's
ruling 0057-c (ADR-0210). Date: 2026-10-07. Milestone: E14.

## Context

- **A map or set from outside was checked on arrival** (ADR-0057): a
  query's parameter, or a host's answer, its keys ascending and each once,
  or the invocation stopped. A binary search over anything else answers
  wrongly.
- **Ruling 0057-c** (ADR-0210): "A map or set arriving from outside is
  sorted on arrival (String by code point). A repeated key still stops the
  invocation, by name. Record the deliberate divergence from WIT `map`
  (last wins), and why `Map.from_lists` (program's choice, last wins)
  differs from the boundary rule." Its reason: "Out-of-order is not 'two
  things'; refusing it ties every host to one collation, against database
  independence."
- **Out of order is what a host gives without trying.** A database's
  collation puts `a` before `B`, where code point order puts `B` first; an
  order by UTF-16 unit, JavaScript's and Java's, puts U+1F600 before
  U+FF61, where code point order puts it after; and a hash map's iteration
  has no order at all. Each was refused.
- **The Component Model's own `map`** (its Explainer, gated 🗺️) lets a key
  repeat: "Bindings generators *may* deduplicate and reorder keys as long
  as the *last* (key, value) pair in the original list defines the final
  value of the key." Its order is unspecified. Read 2026-10-07.

## Decision

1. **A map or set from outside is sorted on arrival**, by its key, as
   ADR-0057 and ADR-0248 order keys: an `Int` by value, a `String` by code
   point, a `Bool` `false` first, an opaque type as its representation. In
   the component, by the merge sort `List.sort_by` and `Map.from_lists`
   use, into a copy; in the browser's module, by the array's own sort with
   the comparison its lookups use. Whether either sort is stable shows
   nowhere, since a key twice stops the invocation.
2. **A key twice still stops the invocation.** Two entries under one key
   are two things the sender said, and which it meant is not the
   receiver's to choose. Sorted, a key twice is two neighbours, found in
   one pass. The browser's module names it, "a map or set from outside
   repeats a key". It names what happened, not the key: a trap's message
   is not labelled, and a key can be a secret's.
3. **The divergences, recorded:**
   - **From the Component Model's `map`**, whose bindings keep the last of
     a repeated key. A Pleris map crosses as `list<tuple<K, V>>`
     (ADR-0057), not as that `map`, and a key twice is refused: the
     sender's two values for one key are a defect of the sender's, and
     keeping either hides it. Should a Pleris map ever cross as the
     Component Model's, this is the difference its binding keeps.
   - **From `Map.from_lists`**, which keeps the last of a repeated key
     (ADR-0057): the program built both lists, and the choice is the
     program's, written where it is made. At the boundary no one inside
     chose.

## Acceptance

- **`compiler/pw-conformance/tests/maps.rs`**: a map given in descending
  order is sorted, its keys and lookups answering as Rust's `BTreeMap`
  does; a key twice still stops the invocation, beside itself or apart;
  a set in the order UTF-16 gives is sorted by code point; 200 maps, each
  shuffled, answer their keys and a lookup as `BTreeMap` does; a host's
  answer out of order is sorted, and a key twice in it stops the
  invocation; a map keyed by a `Bool` given `true` first is sorted.
- **`compiler/pw-conformance/tests/javascript.rs`**: the browser's module
  agrees with the component on each crafted map, the key twice the only
  one stopping; and on a `Bool`-keyed map given `true` first.
- **`scripts/arrival_mutations.py`: 3 mutants**, the component reading
  what arrives as it came, its sort reversed, and the module's sort
  removed; and **`scripts/map_mutations.py`**, ADR-0057's 15, three
  re-anchored to the sort. Recorded by `just e14-arrival`.
- **The whole workspace's tests**, the corpus among them.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **The component's trap names no cause**, as none of its traps does
  (KNOWN_LIMITATIONS, "Traps are not distinguished by cause"): the host
  reports a failed call. The ruling's "by name" holds in the browser's
  module; in the component it waits for a trap carrying its cause, queued
  in NEXT.
- **A map in order is sorted anyway.** The check this replaces cost one
  comparison per entry and no copy; the sort costs n log n comparisons, a
  copy, and a scratch array as long. A first pass that skips the sort for a
  map already in order
  would restore that cost, and waits for a measurement showing the sort's
  matters.
- **A map or set inside another value from outside** stays refused when
  compiled: nothing would sort it.
