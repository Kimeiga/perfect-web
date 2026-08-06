# Milestone numbering

Two numbering schemes exist in this project and they are **not** the same thing.
Before this file, both used a bare `M` prefix and both started at 0, which was a
real source of confusion.

> **Engineering work is numbered `E0`–`E15`. Public demonstrations are numbered
> `P0`–`P9`. A proof may depend on several engineering milestones, and an
> engineering milestone may support several proofs. Documents and issues must use
> the prefix. Never use a bare `M` again.**

There is a third category, added after E0:

> **Engineering milestones describe what we have implemented. The
> risk-retirement queue (`RQ-*`, see `docs/RISK_QUEUE.md`) describes experiments
> that may use temporary dependencies to test assumptions early. Passing an early
> experiment can retire a risk but cannot close the corresponding implementation
> milestone.**

---

## Engineering milestones (E)

Derived from `PROJECT_CHARTER.md` §14. `E1A` and `E2A` were inserted after E0's
evidence; see ADR-0011.

| id | name | status |
|---|---|---|
| **E0** | Repository bootstrap, research matrix, specification corpus, feasibility spikes | **complete** |
| **E1** | Koka **effect-system** feasibility (narrowed — effects and handlers only) | **CLOSED** — RQ-2 Outcome 1: open-row inference, higher-order propagation, selective handling, `.kki` observability |
| **E1A** | `pw` value semantics and boundary ABI *(inserted, ADR-0011)* | **algorithmically implemented, source integration pending** — 42 tests on constructed data. May not support public source-language claims yet. |
| **E2** | Source front end **and semantic integration** — lossless syntax, core body parsing, HIR, source-to-checker wiring, the ≥40 rejected gate | **COMPLETE** — all six gate items. Rowan tree, body grammar (68/68 files), HIR (ADR-0014), `pw fmt` (ADR-0013), Koka execution (ADR-0015), and **44 of 44** rejected programs caught, each for its declared invariant with no unexplained extra diagnostic |
| **E2B** | Program graph and name resolution *(inserted by architect ruling, 2026-08-06)* | **COMPLETE** — module graph, namespaces, import AND use resolution, integrated rather than merely checked: `member_of` resolves by receiver type, `infer::Types` follows a chain, and the by-name fallback is deleted with a lint keeping it gone |
| **E2C** | Core library and platform contracts — signature manifests *(inserted)* | **COMPLETE** — 14 platform modules with real effect rows, the deletion gate met, and the library itself checked as compiler input: it parses, resolves, checks clean, is internally consistent, its rule-read declarations are exercised, and the trusted contract is content-hashed |
| **E2D** | Source effect inference *(inserted; pulled forward out of E9)* | **COMPLETE** — direct, helper-hidden and callback-hidden propagation with the chain in the diagnostic; charter §7.5A frame phases; the layout relations; and effect identity that keeps its type argument |
| **E2A** | Structured concurrency = **E2A-R** (runtime) + **E2A-S** (static scope checker) *(inserted)* | **COMPLETE** — both halves. E2A-S in `compiler/pw-core/src/scope.rs`, rejecting R-013 and R-039 from source; E2A-R in `runtime/pw-tasks` (ADR-0016), 12 behaviour tests, 0/25 flaky |
| **E3** | Marko rendering adapter, streaming SSR, first resumption | **COMPLETE** — all six gate items. Three `.pw` apps generating Marko (ADR-0017); `/static` and `/streamed` ship **zero** JS; the shell is usable 1,201 ms before a 1,200 ms region arrives; the counter resumes without re-execution; 30 tests × 3 engines |
| **E4** | Typed resource model and the first complete store page | **COMPLETE** — all seven gate items. `runtime/pw-resource` with per-key dedup, idempotent commands, ref-counted cancellation and public/private cache separation; and `examples/store/app.pw` generating, building, serving and passing in three engines |
| **E5** | Privacy-flow, cache-safety, and placement checker | **COMPLETE** — all five gate items. Set-of-restrictions label algebra with verified lattice laws, a placement solver over build/browser/edge/origin, **value-based** label propagation (not name-based), markup rules, and five cross-file integration tests where the invariant cannot be decided from any single file |
| **E6** | Materialized resource graph — the ISR successor | not started |
| **E7V** | Resume-version compatibility *(inserted)* | **COMPLETE** — all ten gate items. Content identity, versioned hash schemes, strict matching with explicit migrations, per-construct recovery, build-time artifact agreement with mutation controls, and attachment unreachable without a decision |
| **E7-R** | Own renderer: resumption and DOM preservation | not started — **Marko is the accepted oracle**. E7V decides *whether* a manifest may attach; nothing in a browser calls it yet |
| **E7-P** | Own renderer: streamed patch semantics | not started — Marko accepted in Chrome; Safari *timing* unmeasured |
| **E7-L** | Own renderer: interaction-lazy code loading | not started — **no oracle**; Marko fails this property |
| **E8** | Rust capability host, WIT worlds, Wasmtime execution | not started |
| **E9** | Permanent value type checker and algebraic effect compiler (9A–9D) | not started |
| **E10** | Own backends and automatic memory strategy | not started |
| **E11** | Multi-node MacBook network lab | not started |
| **E12** | HTTP/3 and prioritized application delivery | not started |
| **E13** | Servo and native browser primitive experiments | not started |
| **E14** | Developer tooling, semantic diffs, AI benchmark | not started |
| **E15** | Hardening, production research, standards path | not started |

### What changed at the E0 → E1 boundary

`E1` was narrowed and `E1A` inserted because E0 measured that Koka does **not**
provide exhaustiveness independent of `exn`, nominal runtime identity, or a
generic `Result<T,E>` (ADR-0011).

**E1 scope:** inferred effect rows; handlers discharging effects; effect
propagation through direct calls; row-polymorphic callback effects; source-span
recovery from `.kki`; stable mapping from generated Koka diagnostics to `pw`
source.

**Explicitly out of E1 scope:** exhaustive application matching; nominal ABI
identity; generic `Result<T,E>`; boundary validation; linear resource use;
privacy or placement guarantees. Those are E1A.

**E1's gate is not weakened by the narrowing.** The ≥30 compile-pass /
≥30 compile-fail requirement stands, now scoped to effects. E1A carries its own
value-semantics corpus.

---

## Public proofs (P)

From `web-recompiled/proof-roadmap.md`. These are **communication artifacts**,
not engineering gates. None may be published before its row in
`docs/EVIDENCE_LEDGER.md` is backed by recorded evidence.

| id | name | depends on |
|---|---|---|
| **P0** | The executable thesis (Bug Museum) | E0, E1, E1A, E2, E2A-S, E5 |
| **P1** | The page that ships only what it uses | E3, E7 |
| **P2** | The outage pattern that cannot compile | E4, E2A |
| **P3** | The cache that refuses secrets | E5, E6 |
| **P4** | The marketplace under a hostile network | E4, E11, E12 |
| **P5** | One change, one recomputation | E6 |
| **P6** | Four roles, one domain model | E4, E5, E9 |
| **P7** | The AI proof | E14 |
| **P8** | Native mode | E13 |
| **P9** | The adoption proof | E15 |

The relationship is deliberately many-to-many: engineering milestones build
capabilities; proof milestones package several capabilities into falsifiable
demonstrations.

---

## Historical note

Everything committed before 2026-08-05 used `M0`…`M15` for engineering
milestones. `docs/milestones/M0.md` and `docs/evidence/M0/` keep their filenames
so that commit history and evidence paths stay stable; read `M0` there as `E0`.

**Prose always says `E0`.** The filename is a path-stability decision and not a
second numbering scheme — writing `M0` in a status report is exactly the drift
the `E`/`P`/`RQ` split was introduced to remove, and it happened once
(2026-08-06) and was corrected.
