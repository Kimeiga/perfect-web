# ADR-0032: compiled components — Canonical ABI adapters, upstream wrapping, host execution

Status: accepted under the owner's instruction of 2026-09-24 ("close E10-I");
implemented with the evidence linked below. Decisions marked **(ruling
needed)** were made without an architect ruling and are offered for reversal.
Date: 2026-09-24. Milestone: E10-I (locked order steps 4-9 of 2026-08-20).

## Context

E10-I was deferred from E8. It requires compiling `add_to_cart` through the
production Pleris→component backend and executing it through the E8 host, with
the alternate Rust closure path deleted. Its closing condition
(`docs/EVIDENCE_LEDGER.md`) is that a `.pw`-authored command runs through
`pw_host::engine::call_within` and the closure is deleted rather than left
beside it.

The E10-A encoder held every non-scalar value as an `i32` handle. Its modules
validated but could not be components: `carts#add` had three core parameters,
where the Canonical ABI gives six. There was no memory, no `cabi_realloc` and
no post-return. The dev server's `add_to_cart` was a Rust closure incrementing
a counter.

## Decision

### 1. The world is the ABI authority; `wit-parser` supplies every number

`backend/wasm.rs` was rewritten, not supplemented: there is one encoder. For
one exported function of one world it emits a core module whose facts all come
from upstream `wit-parser` 0.257.1:

- core signatures from `Resolve::wasm_signature`;
- flattening from `push_flat`;
- sizes, alignments and field offsets from `SizeAlign`;
- core import and export names from `wasm_import_name` / `wasm_export_name`
  with the legacy mangling and the synchronous ABI.

The module imports only what the function calls. It exports the function,
`memory`, `cabi_realloc` and the function's `cabi_post_*`.

A value is held either **flat** (in locals) or **in memory** (the address of
its canonical layout). A value moves from memory to flat form only through a
layout-driven `load`. Passing a result on, or returning it, is copy-free when
the two positions have one component type: `carts#add`'s result is
`add_to_cart`'s result.

Everything outside the supported set is refused by name: constructing a record,
projecting a field, calling another Pleris declaration, multi-block bodies,
constants needing memory, and loading variants.

### 2. The invocation region

Linear memory is one bump region. `cabi_realloc` aligns, grows memory, and
traps on overflow or failed growth. The post-return resets the region after the
caller has lifted the result. Nothing can express a value outliving its
invocation, which keeps the 2026-08-20 ruling's provisional status explicit.

### 3. Component-level identity, checked twice

`result<domain-cart, ..>` and `result<domain-store, ..>` flatten identically.
So identity is compared at the component level:

- **During encoding.** `same_type` compares named types by declaration and
  anonymous constructors structurally. A value is never passed or returned
  across a component-type mismatch.
- **After wrapping.** `component::audit` decodes the artifact with
  `wit-component` and compares every imported and exported function's type
  with the world, and reports how many it compared.

The permanent negative control: `carts#add` returning `result<domain-store,
..>` is refused by both checks, although its core signature is identical.

### 4. Wrapping is upstream

`wit-component` 0.257.1 does the wrapping: `embed_component_metadata`, then
`ComponentEncoder` with validation on. It is in exact lockstep with the pinned
`wit-parser`, `wasm-encoder` and `wasmparser` 0.257.1. Wasmtime 47.0.4 reads the
result with its own 0.252.0 set, making it a third, independent consumer.

### 5. The contract locates each export (ruling needed)

`Export` gains an optional `component: { interface, function }`, produced by
`wit::component_export`, the one place that names it. It is not part of
`abi_schema`, and a contract written before it parses, like `binding`. The
alternative was for the host to re-derive the compiler's WIT naming from
`name`; that is the second answer ADR-0020 exists to prevent.

### 6. The host runs a component with the deployment's operations

- `engine::call_within` takes `interface#function → HostFn`. A granted import
  with no implementation is a refusal, not a stub. Exports are found by path,
  and result arity is read from the artifact.
- `engine::imports_of` reports an instance's functions and resources and never
  its types. A type-only interface such as `pw:types/types` grants nothing.
  This corrects E8's audit, which listed the spike guest's `#store` record type
  as an import.

### 7. The dev server's commands are the compiled components

`add_to_cart` and `clear_cart` run `docs/evidence/E10/<component id>.wasm`, the
compiler's output held byte-for-byte by `evidence_is_current`. Admission uses
each artifact's real imports. The server supplies the deployment:

- the platform session operation;
- `store:data/carts`, its data layer, which **stages** a write.

The staged write and its event are committed in one `materializer.command` only
if the compiled command returns `Ok` (ADR-0019). The Rust closures are deleted,
and a structural test keeps them deleted.

### 8. Handler arguments (ruling needed)

**Superseded 2026-09-25 by [ADR-0033](ADR-0033-compiled-handlers.md):** the
handler's body is compiled, and it sends the arguments it computes. The text
below is the decision as it stood at E10-I.

The browser handler for `add_to_cart(item.id, PositiveInt(1))` sends the
pressed loop instance's address and the literal quantity. The server resolves
the address to the item with the derivation that rendered it. Resumable handler
**bodies** are not compiled from Pleris yet; the command they reach is. This is
recorded in KNOWN_LIMITATIONS; compiling handler bodies is E7-L/E10 follow-up
work.

## Acceptance

- `compiler/pw-core/tests/component.rs`: validity, exact imports, the audit
  with a floor, and the core-identical negative control.
- `wasm_encoding.rs`: all five store commands and queries compile and audit.
- `runtime/pw-host/tests/pleris_component.rs`: the compiled `add_to_cart` reads
  the host's session and calls `carts#add` with it, and its `Ok` and `Err`
  results pass through. Also covered: refusals by admission and by the engine,
  an unimplemented grant, and fuel.
- The dev server's unit tests, and the three-engine browser suite.
- Evidence: [e10-i-2026-09-24.md](../evidence/E10/e10-i-2026-09-24.md).

## Consequences

E10-I is closed. E10's remaining obligations are below; the evidence file lists
exactly what the component backend supports:
- compiled resumable handlers;
- records, variants and constants written into the region;
- calls between compiled declarations;
- replacing `store:data/carts` with compiled Pleris over narrower primitives
  (step 10);
- an automatic memory strategy beyond the invocation region.
