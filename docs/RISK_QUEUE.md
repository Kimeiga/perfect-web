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
| **RQ-1** | Is Marko's resumption real, in Chrome **and** Safari? | **done** | 11/12 pre-registered checks pass in both engines → Marko accepted as the behavioural oracle for E7 |
| **RQ-2** | Does Koka propagate effects through higher-order abstraction? | **done** | Outcome 1, clean pass → Koka remains the effects oracle; `pw` effect checker not pulled forward |
| **RQ-3** | `pw`-owned exhaustiveness and canonical typed ABI decoding | next | — |
| **RQ-4** | Minimal static task-scope checker (E2A-S) + runtime structured concurrency (E2A-R) | queued | — |
| **RQ-5** | Final-artifact declared-vs-actual component import verification | queued | — |
| **RQ-6** | Backfill direct / helper-hidden / generic-callback rejection cases for **every** effect family | queued | — |
| **RQ-7** | Generate and adopt the E→P claim/evidence table before publishing P0 | **done** (first draft) | `docs/EVIDENCE_LEDGER.md` |

---

## RQ-1 — Marko resumption (done)

**Pre-registered rule.** All core properties pass in both engines → behavioural
oracle for E7. Passes Chrome, fails Safari → architectural donor only. Component
replay, eager handler, DOM replacement, or lost browser state → stop calling
Marko resumable for our purposes. Only payload scaling passes → report payload
scaling and nothing more.

**Result.** Core properties (no replay, first click works, node identity, focus,
second interaction) passed in **Chrome 150 and Safari 26.5.2**. One check failed
in both: the interaction artifact loads eagerly at 40–55 ms, not on demand — so
charter §1.11 "interaction-lazy code" is **not** satisfied by this configuration.

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

## RQ-3 — `pw` exhaustiveness and typed ABI (next)

Delivers the core of E1A. Pre-registered acceptance:

- an incomplete match is rejected **regardless of the function's effect row** —
  the case Koka provably does not catch;
- `Option<T>` and `List<T>` cannot be confused at the boundary, even though both
  collapse to `null` in Koka's JS output;
- a raw primitive cannot enter a nominal position through an untyped binding;
- decoding is type-directed: `decode(payload, ExpectedType)`, never
  `infer_runtime_type(payload)`;
- every rejection carries a stable `PW####` code, a primary span, and an origin
  span.

Fails if any accepted-corpus program is rejected, or if any rejected-corpus
program compiles.

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

## RQ-5 — artifact import verification

Every build emits declared capabilities, actual component imports, and the
difference. Any undeclared import fails the build (`PW4007`). Prototype exists:
the `ambient` check in `spikes/wasmtime-component/host`.

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
