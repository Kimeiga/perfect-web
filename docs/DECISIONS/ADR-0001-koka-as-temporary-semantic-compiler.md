# ADR-0001 — Koka is a temporary semantic oracle, not the permanent compiler

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** 0

## Context

Charter §14 M1 proposes using Koka's existing ADTs, effect rows and handlers as a
bootstrap oracle rather than writing a type-and-effect checker first. That is only
sound if Koka can actually (a) express the semantics, (b) hand values to
JavaScript, and (c) expose inferred types and effects to our tooling.

`spikes/koka-js-interop` measured all three against Koka 3.2.3 on macOS arm64.

## Decision

Adopt Koka **3.2.3** as a temporary semantic oracle for Milestone 1, reached
through the `--target=js` backend, with two hard boundaries:

1. **No `pw` authoring syntax may be derived from Koka syntax.** Koka is an
   implementation detail behind a generated boundary (charter §4 "tape together
   temporarily").
2. **Effect and type metadata is read from the `.kki` interface file**, parsed by
   `spikes/koka-js-interop/node/kki.mjs`, which pins the Koka version it was
   verified against and throws on any other.

**Deletion condition** (charter §4 requires one): Koka is removed from normal
builds when Milestone 9's own checker accepts the validated accepted corpus and
rejects the validated rejected corpus, with differential agreement on the shared
subset. It then survives only as an optional conformance tool.

## Consequences

**What we get.** ADTs, effect rows, handlers and inference for free, plus
machine-readable effect rows without a fork — `calculate-subtotal` shows `!{}`
and `load-store` shows `!{database-read, trace-effect}` straight out of `.kki`.

**What we must build ourselves anyway** — measured, not assumed:

- **Exhaustiveness.** Koka enforces it only for functions whose effect row
  excludes `exn`; a non-exhaustive match in an `exn`-declaring function compiles
  cleanly and fails at runtime (spike finding F-8). Charter §7.1 and §16.2 need
  an independent rule. Milestone 9A already lists this.
- **Nominal domain types.** Single-field `value struct`s are erased at the JS
  boundary: `Money_usd(350)` *is* `350` (finding F-4). `Money<USD>` gets zero
  runtime protection from Koka.
- **Generic `Result<T,E>`.** Koka's `error<a>` fixes the error side to
  `exception` (finding F-3). We declare our own.
- **Absence.** `Nothing` and `Nil` are both `null` and are runtime-
  indistinguishable (finding F-7).

**Therefore:** the `pw` compiler must carry its own type manifest. It cannot
recover nominal identity, exhaustiveness, or absence from Koka's JS output.

**Risk accepted.** `.kki` is an internal format with no stability guarantee.
Mitigated by the version assertion and by keeping the reader to ~120 lines.

## Revisit when

- Koka 3.2.3 is upgraded (the `.kki` reader will throw; re-verify before trusting).
- Milestone 1's gate cannot be met through the `.kki` route.
- Koka's async/concurrency story on the JS target blocks charter §7.6.
