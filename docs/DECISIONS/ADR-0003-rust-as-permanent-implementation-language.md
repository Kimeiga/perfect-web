# ADR-0003 — Rust is the permanent compiler and host language

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

The compiler front end, semantic IR, checkers and capability host need one
implementation language for the life of the project. Charter §4 lists the Rust
toolchain under "reuse permanently" and §6 shows a Rust front end.

## Decision

Implement the compiler, the capability host, and project tooling in **Rust,
pinned to 1.97.1** via `rust-toolchain.toml`, edition 2024, with
`wasm32-wasip2` installed for component work.

Diagnostics use **`annotate-snippets` 0.12.16** (`MIT OR Apache-2.0`), the
renderer rustc itself uses — see ADR-0009.

The parser is **hand-written** with explicit byte spans. Tree-sitter, if adopted,
is for editor support only and never the authoritative compiler parser
(charter §14 M2 task 2).

## Consequences

- `spikes/compiler-diagnostic` demonstrated that ~150 lines of hand-written lexer
  plus `annotate-snippets` reaches charter §16.3 diagnostic quality, including
  the two-span origin/boundary shape and multi-error recovery.
- One language spans compiler, host and tooling; Wasmtime and the component
  tooling are first-class Rust citizens.
- Licensing is compatible with the intended dual MIT/Apache-2.0 core.
- `just ci` enforces `cargo fmt --check` and `clippy -D warnings`, so lint debt
  cannot accumulate silently.
- **Open:** the lossless syntax tree needed for idempotent `pw fmt` is not yet
  designed. The spike discards comments and whitespace. Milestone 2 owns it.

## Revisit when

- A required dependency (notably Wasmtime) raises its MSRV above the pin.
- The hand-written parser cannot support incremental reparsing at Milestone 9's
  required latency.
