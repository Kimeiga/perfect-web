# ADR-0040: the standard library's lists and strings, compiled

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14 M10 task 2).

## Context

`packages/pw-std/list.pw` declared `map`, `fold`, `filter` and `length` with
placeholder bodies: `map` returned `[]` and `length` returned `0`. Programs
type-checked against them and could not compute with them, and there was no
string module at all. kiokun's shard rule reads a word's code points, and its
ranking compares, prefixes and orders strings.

## Decision

### 1. An operation the compiler supplies is declared `intrinsic`

```text
fn length<T>(items: List<T>) -> Int !{}
    intrinsic "list.length"
```

The declaration has no body. `intrinsic` is declaration metadata, read by one
function (`backend::intrinsic_binding`) as `host` is (ADR-0032): the backend
decides by what the declaration says it is, never by its name. An `intrinsic`
the backend does not know is refused by name. Its signature is an ordinary
signature, which the checker and the value relations read as they read any.

### 2. The operations

| module | operation | meaning |
|---|---|---|
| `List` | `length` | the number of elements |
| | `get(xs, i)` | `Some` of element `i`, `None` outside `0 ..< length` |
| | `take(xs, n)` | the first `n`, all of them when `n` is larger, none when negative |
| | `concat(a, b)` | `a`'s elements, then `b`'s |
| | `map`, `filter`, `fold`, `any`, `all`, `find` | as usual; `find` is the first match |
| | `sort_by(xs, compare)` | stable; `compare(a, b) > 0` puts `b` first |
| | `group_by(xs, key)` | runs of adjacent elements with equal `String` keys, each a view of `xs`; added by ADR-0041 |
| `Float` | `from_int(n)` | the nearest `Float`, ties to even; added by ADR-0043 |
| `String` | `length` | code points, not bytes |
| | `codepoints`, `from_codepoints` | a `List<Int>` of Unicode scalar values; a value that is not one traps |
| | `starts_with`, `ends_with`, `contains` | by code points, which is by bytes for UTF-8 |
| | `join(parts, separator)` | |
| | `trim` | strips Unicode `White_Space` from both ends, as Rust's `str::trim` does |
| | `to_lower_ascii` | `A`–`Z` only **(ruling needed)**: Unicode case mapping needs tables and context, and is not decided |

### 3. A function argument is compiled where it is called

`List.map(xs, x => x + 1)`, `List.map(xs, double)` and `xs |> List.map(f)`:
the function is a lambda or a named declaration, known at the call, and its
body is compiled into the loop as a region with its parameters bound, as an
inlined call is (ADR-0039 §4). A function value stored, returned, or passed to
a declaration that is not an intrinsic is refused.

### 4. How a list is held

A list is its canonical layout: a pointer and a length, the elements
contiguous at the size and alignment `SizeAlign` gives. `take` is a view of
the same elements; lists are immutable, so a view is safe. Every operation
that makes a list allocates it in the invocation region.

### 5. `sort_by` is a merge sort

Bottom-up, with one scratch buffer: `O(n log n)` comparisons, stable. The
comparator's body is emitted once and run for every comparison.

## Found while building it

1. **`(a, b) => e` never parsed.** The lambda grammar describes it, and the
   HIR lowers a parenthesised list as a lambda's parameters, but the parser
   read one expression inside parentheses and reported the comma as an
   unclosed parenthesis. The corpus wrote `fn(a, b) e` or passed a named
   function, so nothing had tried. A parenthesised list now parses; outside a
   lambda's parameters it is an error, because the language has no tuple
   value.
2. **The backend and the typer each had a reading of a lambda's
   parameters.** `fn(a, b) e` lowers to one unnamed constructor pattern, which
   the typer unpacks (`values::lambda_names`). The backend now reads
   parameters through that function, so the two cannot disagree.

## Consequences

- The placeholder bodies are gone, so programs that called `List.map` and
  type-checked against a stub now compute with it.
- The E9 counts move: the three placeholder bodies that agreed with their
  declared results (`seed`, `items`, `0`) and the one that was undecided
  (`[]`) are not relations any more, and the new signatures add annotations.
- `sum`, `maximum` and `enumerate` stay as they were: nothing compiled calls
  them, and `enumerate` needs a tuple type the language does not have.
- The trusted platform contract's hash changed, and
  `tests/platform_contracts.rs` records why.

## Acceptance

- `compiler/pw-conformance/tests/stdlib.rs`: every operation over generated
  inputs, against `Vec`, `str` and `char`, with an `Int` overflow inside a
  function argument, and an invalid code point, trapping where the reference
  has no value.
- Refusals by name: a function passed to a declaration, an empty list whose
  type nothing fixes, an `intrinsic` the backend does not know.
- Mutation controls: `scripts/stdlib_mutations.py`, recorded by
  `just e10-stdlib` in `docs/evidence/E10/stdlib.txt`.
