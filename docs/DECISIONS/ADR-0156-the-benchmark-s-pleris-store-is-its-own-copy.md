# ADR-0156: the benchmark's Pleris store is its own copy

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14, so the canonical store can grow toward charter §15.

## Context

Each benchmark task's patches are written against a store:
- the Next.js and SvelteKit ones against `benchmarks/baselines/next-react`
  and `benchmarks/baselines/sveltekit`, copies of their own;
- the Pleris ones against `examples/store`, which is also the canonical
  store (charter §15) that the browser suite and the development server's
  tests run.

The audit of the store against §15 (`docs/research/charter-15-store-audit.md`)
leaves the store to grow: availability, the route, per-line controls, the
recommendation and estimate slots. Each growth moves the context the tasks'
108 patches are anchored on, so every change would re-base the benchmark,
and a benchmark whose starting point keeps moving is not reproducible.

## Decision

1. **The benchmark's Pleris store is `benchmarks/baselines/pleris`**:
   `domain.pw`, `lib/` and `store/`, as `examples` held them when all twelve
   tasks were written. It is frozen, as the other two stacks' baselines are.
   A task's sandbox, `pw diff`'s comparisons, the unsafe table and the keyed
   store all build from it.
2. **The canonical store, `examples/store`, grows** toward §15 and the
   DoorDash the project is for. The browser suite and the development server
   serve it.
3. **The development server's tests choose their store.** A test that
   applies a task's patches builds the benchmark's store
   (`served_from_patches`). A test of the canonical store's own features
   builds `examples` (`served_from_patches_in`).

## Consequences

- E14-A's parity is now with the benchmark's store: the three baselines
  match each other. The canonical store may have more.
- The development server serves both. A feature the canonical store adds is
  a host function or a hook the benchmark's store never calls.

## Acceptance

- The benchmark's store is byte-identical to `examples` at the commit that
  makes it. The tasks' controls, `pw diff`'s tests and the server's tests
  hold as before.
