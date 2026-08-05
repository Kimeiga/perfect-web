# Next

The next executable tasks, in order, with acceptance criteria. Charter §3.4.

**Do not start Milestone 2 while Milestone 1 is open.** Charter §0: *"Do not
attempt all milestones concurrently."*

---

## N-0 — Close the Milestone 0 shortfall: Linux CI

**Why now:** the only unmet part of the M0 gate. Risk R11 is the
highest-likelihood open risk. Cheap now, expensive at Milestone 3 when generated
file paths and asset casing start to matter.

**Do:**

1. `.github/workflows/ci.yml`: run `just ci` on `ubuntu-latest` (x64 and arm64).
2. Give `scripts/bootstrap.sh` a Linux branch — the same upstream releases have
   `koka-v3.2.3-linux-{arm64,x64}.tar.gz` and
   `wasmtime-v47.0.3-{aarch64,x86_64}-linux.tar.xz`. Keep the SHA-256 verification.
3. Add a case-collision check (two paths differing only in case) — macOS will not
   catch this.
4. Add `cargo-deny` (licenses + advisories) and `pnpm audit`. Charter §3.6
   requires license and vulnerability checks in CI, and they currently do not run.

**Acceptance:** `just ci` green on Linux; a deliberately case-colliding path
fails the build; `cargo-deny check` passes against the licenses recorded in
`docs/research/technology-matrix.md`.

---

## Milestone 1 — Koka semantic kernel

**Question (charter §14 M1):** can Koka's ADTs, effect rows, handlers and memory
model represent the desired application semantics well enough to serve as a
bootstrap oracle?

Milestone 0 already answered part of this. Three amendments are **required** by
what it measured — see `docs/milestones/M0.md` "Next decision".

### N-1 — Promote the `.kki` effect reader to a tool

Move `spikes/koka-js-interop/node/kki.mjs` to `tools/kki-effects/` with its own
tests. Milestone 1's gate item *"every effectful example has a visible inferred
effect"* is demonstrated **through this tool**, so it must not stay spike-quality.

Keep the `VERIFIED_AGAINST_KOKA` assertion — `.kki` is an internal format.

**Acceptance:** `just test-unit` covers it; it parses every module in
`stdlib/koka/` and reports effect rows.

### N-2 — Effect libraries under `stdlib/koka/`

Charter §14 M1 task 1:

```text
database_read  database_write  network  clock  random  trace
session  secret  storage  task_scope  resource_scope
query  command  subscription
```

**Note from M0 finding F-6:** a multi-operation effect must be discharged by ONE
`handler` block; chained `with fun op(..)` shorthands leave later operations
unhandled.

### N-3 — Domain ADTs and opaque wrappers

`StoreId`, `ConsumerId`, `MenuItemId`, `InteractionId`, `Money`, `Option`/`Maybe`,
`Result`, `StoreError`, `CartError`, `OrderState`.

**Amendment required:** do **not** claim nominal domain types are enforced.
Single-field `value struct`s are erased at the JS boundary (`Money_usd(350)` is
`350`). Document the gap the way charter §14 M1 task 7 already instructs for
linearity. `Result<T,E>` must be declared ourselves; Koka's `error<a>` fixes the
error side to `exception`.

### N-4 — Development handlers

In-memory/SQLite-boundary database, deterministic test clock, seeded random,
collecting trace, fixture session, named local fixture secret. All deterministic —
no wall clock, no real randomness.

### N-5 — Compile-test harness over the corpus

Runs Koka across accepted and rejected snippets and snapshots diagnostics.

**Amendment required (M0 finding F-8):** every exhaustiveness compile-fail case
must declare a **total** effect row. Koka accepts a non-exhaustive match in any
function that declares `exn`, so a case written without that constraint **passes
vacuously**. The harness should assert this property of its own fixtures.

### N-6 — Prove a pure view cannot call an effectful query

Charter M1 gate item. The effect must appear in the inferred row and the pure
wrapper must reject it.

### N-7 — Scoped cleanup prototype

Charter §14 M1 task 7 — and its warning: *"Do not claim Koka statically proves
linear usage if it does not; document the gap."* Milestone 0 did not test
linearity at all, so assume nothing.

### Milestone 1 gate (charter §14 M1)

- Pure examples compile and run.
- Every effectful example has a visible inferred effect.
- A network request in a declared pure view fails the harness.
- External data decoding returns explicit errors.
- ≥30 compile-pass and ≥30 compile-fail cases automated.
- The Koka adapter boundary and its limitations are documented.
- No application authoring syntax is permanently coupled to Koka syntax.

---

## Then — Milestone 2 (source language, parser, lowering to Koka)

Not before Milestone 1's gate passes. Two things Milestone 0 already settled:

- **Diagnostics stack decided** (ADR-0009): hand-written lexer with byte spans +
  `annotate-snippets` 0.12.16. Charter §16.3 quality is reachable; proven.
- **Still open:** the *lossless* tree needed for idempotent `pw fmt`. The spike
  discards comments and whitespace. `rowan` 0.17.0 and `logos` 0.16.1 are the
  candidates recorded in the version log.

Two policies Milestone 0 discovered that Milestone 2 must adopt:

- **Suppress semantic checks when parse diagnostics exist** (finding F-4), as a
  general derived-error policy, not a one-off.
- **Test `pw explain` for generated-file leakage**, don't merely intend it. The
  assertion already exists in the spike.

Also: ADR-0010 (`.pw` extension) is **Proposed**, not Accepted. Milestone 2
task 1 owns the real decision and should supersede it.

---

## Standing obligations (every milestone)

Charter §3.1:

1. Inspect the repository and `docs/STATUS.md` first.
2. **Re-verify upstream versions and APIs against primary sources.** Milestone 0
   found seven wrong assumptions this way (`docs/research/version-verification.md`).
3. Write or update an ADR **before** a consequential design change.
4. Add failing tests or a reproducible benchmark before implementation.
5. Implement the smallest end-to-end vertical slice.
6. Run all relevant checks.
7. Record measured results, limitations and unexpected findings.
8. Make a focused local commit.
9. Update the milestone gate checklist.
10. Proceed only when the gate passes, or document precisely why it cannot.
