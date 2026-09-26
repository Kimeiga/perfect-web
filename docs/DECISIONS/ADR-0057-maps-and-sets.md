# ADR-0057: maps and sets

Status: accepted under the owner's instruction of 2026-09-25 ("ok fix all the
known gaps"). Decisions marked **(ruling needed)** were made without a ruling
and are offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14
M10 task 2, the standard library).

## Context

KNOWN_LIMITATIONS: "no map or set type". A program looked a value up by key
by scanning a list, and removed repeats by sorting and grouping. The
language has no tuple type and the backend no generic records, so neither
could be written in Pleris as a library.

## Decision

### 1. Two built-in types

`Map<K, V>` and `Set<T>` are language types, as `List<T>` is, with modules of
the same names in the standard library:

- **`Map`:** `empty`, `size`, `get`, `contains`, `insert`, `remove`, `keys`,
  `values`, and `from_lists(keys, values)`. In `from_lists` a later key
  replaces an earlier one, and lists of two lengths stop the invocation.
- **`Set`:** `empty`, `from_list`, `size`, `contains`, `insert`, `remove`,
  `to_list`, `union`, `intersection` and `difference`.

Each operation answers a new value; a map does not change.

- **Keys.** A map's key, and a set's element, is an `Int` or a `String`:
  an `Int` ordered by value, a `String` by code point. **(ruling needed)**:
  no other type has an order the component and the module share, so
  another key is refused by name.
- **Order.** Entries are in ascending key order, each key once. That is the
  order `keys`, `values` and `to_list` answer in. **(ruling needed)**:
  sorted, rather than the insertion order a JavaScript `Map` keeps.
- **Empty.** `Map.empty()` and `Set.empty()` take their types from where
  they are used, as `[]` does.

### 2. What arrives from outside is checked

A map or set a query is given, or a host function answers, is checked when
it arrives: each key below the next, or the invocation stops. A binary
search answers wrongly over anything else. **(ruling needed)**: checked and
refused, not sorted into order. A host that repeats a key has said two
things, and neither is chosen for it.

A map or set inside another value (a parameter's list, a record's field, a
host's option) is refused by name, since nothing would check it.

### 3. In the component and the module

- **Component layout:**
  - a map is `list<tuple<K, V>>`, the way the world writes it at the
    boundary;
  - a set is `list<T>`;
  - WIT's anonymous tuple is the entry type, so no generic record is
    needed.
- **Component operations:**
  - a lookup is a binary search;
  - a change copies around the key's place;
  - `from_lists` and `from_list` sort stably, then keep the last of equal
    keys;
  - `union`, `intersection` and `difference` merge in one pass;
  - the merge sort behind `List.sort_by` is shared: it takes a comparison
    now.
- **Module:** a map is an array of `[key, value]` pairs in the same order,
  and a set an array. Keys compare as the component compares them, a
  `String` by code point, not by UTF-16 unit.

## Acceptance

- `compiler/pw-conformance/tests/maps.rs`, through the E8 host against
  `BTreeMap` and `BTreeSet`:
  - every operation;
  - repeated keys;
  - a record as a value;
  - maps out of order, with a key twice, and in the code point order
    UTF-16 reverses, given as a parameter and answered by a host.
- `compiler/pw-conformance/tests/javascript.rs`: nine map and set queries.
  The component and the module agree on 70 queries and 14,000 calls, and on
  each crafted map the entry check refuses or keeps.
- Mutation controls: `scripts/map_mutations.py`, `just e10-maps`,
  15 mutants, in the component, the lowering and the module.

## Not done

- **A `for` loop over a map or a set** reads it through `Map.keys` or
  `Set.to_list`.
- **A map or set inside a parameter's value or a host's answer** is
  refused.
- **Another key type**, or a key ordered another way, is refused.
