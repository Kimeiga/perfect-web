# ADR-0008 — Target WASI 0.2; WASI 0.3 is not reachable through the stable toolchain

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §10.3: "Prefer the newest stable WASI version that the selected
Wasmtime/toolchain supports reliably. Verify support experimentally. If WASI 0.3
async/component tooling is incomplete for a required language or binding
generator, use stable WASI 0.2 behind an adapter and record the migration plan."

This is a decision the charter explicitly refuses to make on our behalf, so it
had to be measured.

## Decision

Target **WASI 0.2** via Rust's `wasm32-wasip2` target. Measured on 2026-08-05
with Rust 1.97.1, `wit-bindgen` 0.60.0 and Wasmtime 47.0.3, the emitted component
imports resolve at **`@0.2.9`**.

Version-specific bindings are isolated behind the WIT worlds in
`spikes/wasmtime-component/wit/`, so a later migration changes the world
definitions and the host, not application semantics.

## Consequences

- The stable, well-supported path is used, and the charter's fallback branch is
  taken on evidence rather than on assumption.
- **Migration plan:** re-run `just spike-wasmtime` after any Rust or Wasmtime
  upgrade; if the emitted imports move to `@0.3.x` and the four capability checks
  still pass, supersede this ADR.
- WASI 0.3's async story is unavailable, which matters for charter §7.6
  structured concurrency at the component boundary. Recorded in
  `docs/KNOWN_LIMITATIONS.md`.

## Revisit when

Rust ships a stable `wasm32-wasip3` target, or Wasmtime + `wit-bindgen` support
0.3 worlds reliably. Re-measure with the existing spike.
