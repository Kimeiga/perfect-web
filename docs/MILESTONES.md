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
| **E1** | Koka **effect-system** feasibility (narrowed — effects and handlers only) | in progress |
| **E1A** | `pw` value semantics and boundary ABI *(inserted, ADR-0011)* | **in progress** — exhaustiveness, opaque nominal types, platform `Option`/`List`/`Result`, type-directed ABI decoder all landed in `compiler/pw-core` |
| **E2** | Source language parser, modules, lowering to Koka | not started |
| **E2A** | Structured concurrency = **E2A-R** (runtime) + **E2A-S** (static scope checker) *(inserted)* | **E2A-S done**, E2A-R not started |
| **E3** | Marko rendering adapter, streaming SSR, first resumption | not started |
| **E4** | Typed resource model and the first complete store page | not started |
| **E5** | Privacy-flow, cache-safety, and placement checker | not started |
| **E6** | Materialized resource graph — the ISR successor | not started |
| **E7** | Own document-parts compiler and browser runtime | not started |
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
New work uses the `E`/`P`/`RQ` prefixes.
