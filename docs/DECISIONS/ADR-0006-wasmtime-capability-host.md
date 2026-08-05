# ADR-0006 — Wasmtime is the capability host; effects become WIT imports

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §10.2 maps language effects onto host interfaces and asserts "a component
without an imported capability cannot use it". §14 M0 task 10 required proving
this with one real component.

## Decision

Use **Wasmtime 47.0.3** with the **WebAssembly Component Model** and
**`wit-bindgen` 0.60.0**. Each language effect family lowers to a WIT interface;
each deployment target is a WIT world listing exactly what it may import.

Generated guest components must be **`no_std`-equivalent**, and the build must
**assert the component's import list against its declared WIT world**.

## Consequences

**The boundary is real, and measured.** All four checks pass on the minimal
component: imports are exactly `["perfect-web:store/stores@0.1.0"]`; the
capability works when granted; instantiation is **refused** when withheld, with a
diagnostic naming the missing import; and a 1,000-unit fuel budget traps
execution.

**The `no_std` requirement is not stylistic.** A `std` Rust guest on
`wasm32-wasip2` imports **15** WASI instances when its world declares **one** —
`wasi:cli/environment`, `wasi:cli/exit`, stdio, terminal, clocks, io. That is
ambient authority injected by the language runtime, exactly what charter §14 M8
task 4 says to deny. `no_std` removes all 14 and is 8.3x smaller
(5,276 B vs 43,837 B), at the cost of a hand-written allocator, panic handler and
`cabi_realloc`.

**Calibrate limits by measurement:** one component instantiation creates several
core wasm instances, so `instances(1)` rejects a valid component.

## Revisit when

- WASI 0.3 component tooling becomes reliable (see ADR-0008).
- A required capability cannot be expressed in WIT.
- Wasmtime's MSRV exceeds the Rust pin.
