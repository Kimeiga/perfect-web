# Risk-retirement queue

Experiments that test a load-bearing assumption **early**, using temporary
dependencies if that is what it takes.

> Passing an `RQ` retires a risk. It does **not** close the corresponding
> engineering milestone. `docs/MILESTONES.md` holds that distinction.

Each entry records its **decision rule before it runs**. That is not ceremony:
three separate measurements in this project produced plausible, favourable
numbers that turned out to be measuring nothing (see "the recurring bug" below).

Order was fixed by the project architect after reviewing E0's evidence.

| id | question | status | ruling |
|---|---|---|---|
| **RQ-1** | Is Marko's resumption real, in Chrome **and** Safari? | **done** | 11/12 in both engines → oracle for **E7-R** (resumption, DOM preservation) and partially **E7-P**. **Not** the oracle for **E7-L**: check 4 failed. |
| **RQ-2** | Does Koka propagate effects through higher-order abstraction? | **done** | Outcome 1, clean pass → Koka remains the effects oracle; `pw` effect checker not pulled forward |
| **RQ-3** | `pw`-owned exhaustiveness and canonical typed ABI decoding | **partial** | exhaustiveness + type-directed ABI land in `compiler/pw-core`; 30 tests. Remaining: wire to a real parser (E2) so corpus files can drive it |
| **RQ-4** | Minimal static task-scope checker (E2A-S) + runtime structured concurrency (E2A-R) | **partial** | E2A-S landed: `compiler/pw-core/src/scope.rs`, PW2001-PW2004, 10 tests. E2A-R (runtime) not started |
| **RQ-5** | Final-artifact declared-vs-actual component import verification | **partial** | rule + `PW4007` in `compiler/pw-core/src/capability.rs`, tested against E0's real 15-vs-1 import lists. Remaining: call it from the build, not just from tests |
| **RQ-6** | Backfill direct / helper-hidden / generic-callback rejection cases for **every** effect family | **partial** | layout/DOM families done: 10 accepted + 13 rejected, including both hidden cases. Remaining: backfill the other families |
| **RQ-7** | Generate and adopt the E→P claim/evidence table before publishing P0 | **done** (first draft) | `docs/EVIDENCE_LEDGER.md` |

---

## RQ-1 — Marko resumption (done)

**Pre-registered rule.** All core properties pass in both engines → behavioural
oracle for E7. Passes Chrome, fails Safari → architectural donor only. Component
replay, eager handler, DOM replacement, or lost browser state → stop calling
Marko resumable for our purposes. Only payload scaling passes → report payload
scaling and nothing more.

**Result.** Core properties (no replay, first click works, node identity, focus,
second interaction) passed in **Chrome 150 and Safari 26.5.2**.

**Check 4 failed in both engines** — the interaction artifact loads eagerly at
40–55 ms. That was a *pre-registered property*, so it is not a footnote: it
falsifies charter §1.11's "interaction-lazy code" for this configuration, and
E7 is subdivided accordingly (`docs/MILESTONES.md`).

> Marko is the behavioural oracle for resumption, DOM preservation and
> patch-placement semantics. **It is not the oracle for interaction-lazy code
> delivery.**

Safari's autorun path is acceptable for what it can observe and **must not** be
described as equivalent to the Chrome stream-timing harness.

Evidence: `docs/evidence/M0/spike-browser-resumption.txt` ·
`spikes/browser-resumption/README.md`

## RQ-2 — Koka higher-order effects (done)

**Pre-registered rule.** Koka may overstate an effect but must never silently
understate or erase one. Four enumerated outcomes with a fixed decision table.

**Result.** Outcome 1 — automatic inference of effect polymorphism on
*unannotated* generic helpers, propagation through two layers, correct selective
discharge, and no capability hiding. E1 clean pass on effects.

Evidence: `docs/evidence/M0/spike-koka-row-polymorphism.txt` ·
`spikes/koka-row-polymorphism/README.md`

## RQ-3 — `pw` exhaustiveness and typed ABI (partial)

Delivers the core of E1A. Pre-registered acceptance, with what is done marked:

- **done** — an incomplete match is rejected **regardless of the function's
  effect row**, the case Koka provably does not catch
  (`tests/differential_vs_koka.rs`);
- **done** — `Option<T>` and `List<T>` cannot be confused at the boundary, even
  though both collapse to `null` in Koka's JS output;
- **done** — a raw primitive cannot enter a nominal position undetected: debug
  mode catches a `Money<EUR>` payload arriving in a `Money<USD>` slot;
- **done** — decoding is type-directed: `decode(path, ExpectedType, payload)`,
  and there is deliberately no `decode_unknown(payload)` entry point;
- **done** — rejections carry a stable `PW1004` code, a primary span and an
  origin span.

**Open:** there is no parser, so the checker runs on a constructed pattern
matrix rather than on `.pw` files. The original acceptance line — *"fails if any
accepted-corpus program is rejected, or any rejected-corpus program compiles"* —
cannot be evaluated until **E2** puts a front end in front of it. RQ-3 is
therefore `partial`, not `done`.

## RQ-4 — structured concurrency

Split, because a runtime task tree cannot make a compile-fail fixture fail at
compile time:

- **E2A-R (runtime):** owner scopes, cancellation propagation, cleanup, leak
  detection, results not committing into dead scopes. These are *behaviour*
  tests and must never be presented as static guarantees.
- **E2A-S (static):** `task.spawn` owned by the lexical scope; handles that
  cannot be returned, stored, or captured into a longer-lived scope; no
  detachment without a durable capability; effects propagating through helpers.

**E2A closes only when both halves pass.** Koka representing `task.spawn` in an
effect row would be useful but insufficient — it does not prove scope
non-escape, and `pw` owns that property.

**E2A-S status: implemented** in `compiler/pw-core/src/scope.rs`. A scope tree
plus handle-lifetime rules, with four diagnostics:

| code | rule |
|---|---|
| `PW2001` | a handle escapes into a scope that outlives its owner |
| `PW2002` | an ordinary task is detached without the `durable_job.enqueue` capability |
| `PW2003` | a handle is used outside its owning scope — the late-result-into-dead-component case |
| `PW2004` | a subscription declares a scope longer than the component that created it |

Each carries an **origin span** as well as a violation span, per charter §16.3.
Escaping into a *descendant* scope is allowed, because a descendant dies no
later than the handle does. `durable.spawn` is the legal escape hatch and is
exempt by capability, not by convention.

**E2A-R status: not started.** Runtime cancellation, cleanup ordering, and leak
detection remain unproven, and the static rules above must never be described as
covering them.

## RQ-5 — artifact import verification (partial)

Every build emits declared capabilities, actual component imports, and the
difference. Any undeclared import fails the build (`PW4007`).

**Implemented** in `compiler/pw-core/src/capability.rs`, deliberately free of any
Wasm dependency so the rule is unit-testable without a runtime. Its tests use the
**real import lists E0 measured** — 1 for the `no_std` guest, 15 for the `std`
guest — rather than invented fixtures.

Two runtime profiles, because charter §14 M8 task 4 requires denying ambient
authority by default while still allowing a richer runtime *explicitly*:

| profile | allowance |
|---|---|
| `minimal` | declared interfaces only; `no_std`-equivalent |
| `wasi-cli` | additionally tolerates `wasi:cli/`, `clocks/`, `io/`, `random/`, `filesystem/` |

Choosing `wasi-cli` does not hide the surface: the extra imports are still
listed under `allowed_by_profile`. And an import outside the allowance —
`wasi:sockets/tcp` — still fails.

The `PW4007` text carries the architect's terminology correction, because it is
the difference between a true and a false claim: these imports are a *requested
authority surface*, not authority already held. The host still decides what to
link.

**Open:** the audit is called from tests, not from a build step. Wiring it into
`spikes/wasmtime-component/host` (which already computes the actual list) and
then into the E8 build is the remaining work.

## RQ-6 — effect-family rejection coverage

For **every** effect family, three cases are required before the family counts as
implemented:

1. one accepted use;
2. one **direct** rejected use;
3. one rejected use **hidden behind a helper or generic callback**.

The third is the one that matters. An effect checker that catches
`element.offsetWidth` in a component body and misses it two calls deep inside
`measure_tooltip` is worth approximately nothing, and it is exactly the failure
that survives a green test suite. RQ-2 showed Koka handles this for its own
effects; the `pw` checker must be shown to do the same.

---

## The recurring bug

Three measurements in this project produced plausible, favourable numbers while
measuring nothing:

| where | what it reported | what was actually happening |
|---|---|---|
| layout spike F-2 | 79.7 ms → 0.3 ms, looking like a fix | the write stopped invalidating layout, so no work was done |
| RQ-1 F-1 | 72 "component replacements" | the browser's own initial HTML parse |
| RQ-2 F-2 | a generic helper reported as **pure** | an effect-polymorphic row variable fell through to the "total" branch |

Two flattered the system, one damned it. All three were caught by looking at the
raw number and asking whether it was plausible.

**Rule adopted:** every check ships with a negative case proving it can go red.
A measurement that cannot fail is not a measurement.
