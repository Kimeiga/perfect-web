# ADR-0044: pure computation compiles to JavaScript modules

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14 M10
task 2).

## Context

Charter §14 M10 task 2 orders the browser backend: "first: generated modern
JavaScript modules for handlers and pure computation; then: Wasm for pure
compute-heavy modules". Handlers compile to modules since ADR-0033, but only
as one command call. Pure computation compiled only to Wasm components, run
on the server (ADR-0039, ADR-0040).

## Decision

### 1. A second encoder of the same IR

`backend::js_pure` encodes a lowered function (`backend::ir`) as an ES module
that exports `run`. The lowering is the component backend's own:
`component::lowered` and `component::pure_queries` hand the encoder the
functions `compile` would encode. So the two backends cannot type, inline or
resolve a declaration differently. Only the encoding differs.

A query is pure computation when it reaches no host: it is a query, and its
lowered function needs no capability. `pw build` writes each one to
`modules/<id>.mjs`. kiokun's shard rule is the first: `shards.Place` and
`shards.Places`, 7.8 and 8.5 KB, beside their Wasm components.

### 2. Pleris semantics, written out

Every place JavaScript's semantics differ from Pleris's is encoded
explicitly, never inherited:

| Pleris | JavaScript's own | the module |
|---|---|---|
| `Int`: 64 bits, trap on overflow | a double, exact to 2^53 | a `BigInt`, checked after `+` `-` `*` and negation |
| `/` and `%`: Euclidean, trap on zero | `BigInt` truncates toward zero | `div` and `rem`, adjusted |
| `String` order: by code point | `<` by UTF-16 unit, so U+1F600 < U+FF61 | `compare`, by code point |
| `String.length`: code points | `.length`: UTF-16 units | `Array.from(s).length` |
| `String.trim`: Unicode `White_Space` | also strips U+FEFF, keeps U+0085 | `trim`, over the component's set |
| `from_codepoints`: a surrogate traps | `fromCodePoint` makes a lone surrogate | checked first |
| a trap | — | a thrown `Error` whose message begins `trap:` |

A record is an object keyed by its Pleris field names. `Some(v)` is
`{ $case: "some", value: v }` and `None` is `{ $case: "none" }`; `$` begins
no Pleris name, so a case is never mistaken for a record. This representation
is **(ruling needed)**: it is what a browser caller of `run` must build and
read.

## Acceptance

- `compiler/pw-conformance/tests/javascript.rs` compiles each query twice,
  runs the component through the E8 host and the module under Node, and
  compares them on the same generated arguments:
  - 32 queries, 200 calls each: arithmetic at the 64-bit bounds, division and
    remainder, floats, strings in both orders, every standard-library list
    and string operation, records, `Option`, interpolation and
    `Float.from_int`. They agree, and 418 calls trap in both.
  - kiokun's `shards.Place` and `shards.Places`, 400 calls: agree.
- Mutation controls: `scripts/javascript_mutations.py`,
  `just e10-javascript`. Each of the ten mutants puts back one of
  JavaScript's own semantics from §2, and each is caught.

## Not done

- **A handler that computes.** A handler still compiles to one command call
  (ADR-0033). Computing its arguments with this encoder is the next step.
- **Wasm in the browser.** The charter's second step, Wasm for
  compute-heavy modules in the browser, is not started. The components run on
  the server.
- **Node is the only engine the modules are tested in.** The browser suite
  does not load them yet.
