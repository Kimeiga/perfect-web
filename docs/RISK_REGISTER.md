# Risk register

Charter §20 names the risks and their fallbacks. This file tracks their **current
state** against Milestone 0 evidence, plus risks the spikes discovered.

Likelihood/impact are judgements; the "evidence" column is not.

| # | risk | state after M0 | likelihood | impact | fallback / mitigation |
|---|---|---|---|---|---|
| R1 | **Koka interoperability is insufficient** | **REDUCED** | low | high | Measured: `--target=js` emits plain `.mjs` ES modules importable from Node with no adapter; ADTs, handlers and `maybe`/`result` all cross. Charter fallback (keep Koka as an isolated oracle, pull M9 earlier) remains available. |
| R2 | **Koka lacks structured metadata** | **CLOSED** | — | — | The `.kki` interface file carries full signatures *with inferred effect rows*, source spans and constructor tags. `node/kki.mjs` parses it in ~120 lines. **No fork needed** (ADR-0001). Residual: `.kki` is internal/unstable — mitigated by a version assertion that throws on any Koka other than 3.2.3. |
| R3 | **Koka's semantics are weaker than assumed** | **NEW, REALIZED** | — | high | Not in the charter's list; found by measurement. Exhaustiveness is enforced only for `exn`-free functions; nominal value structs are erased; `Nothing`/`Nil` are both `null`. **Mitigation:** `pw` owns exhaustiveness (M9A) and carries its own type manifest; M1 compile-fail cases must use total effect rows or they pass vacuously. |
| R4 | **Marko 6 APIs are unstable or incompatible** | **REDUCED** | low | medium | Pinned to marko 6.3.32 / @marko/run 0.11.8 / adapter 2.0.6 / vite 8.2.0, all measured working. Marko 6 is `latest` on npm and actively released. Kept behind a generated adapter with a deletion condition (ADR-0002). |
| R5 | **Resumption serialization is fragile across deployments** | **OPEN** | medium | high | Untested. Marko emits content-addressed handler assets and an inline manifest, and the corpus specifies `on_version_mismatch safe_refetch` (A-014) and rejects private data in the manifest (R-030). Rolling-deployment compatibility tests are Milestone 7. |
| R6 | **WASI 0.3 support lags** | **REALIZED, HANDLED** | — | low | Measured: `wasm32-wasip2` emits `@0.2.9`. Charter's own fallback taken deliberately (ADR-0008) with a re-measurement trigger on any Rust/Wasmtime upgrade. |
| R7 | **Privacy type system becomes too complex** | **OPEN** | medium | medium | Start explicit and conservative. One rule is implemented end to end and produces charter §16.3-quality output; 8 corpus rejections cover the rest. Fallback: require annotations at shared caches and external boundaries, add inference only once diagnostics are understandable. |
| R8 | **Automatic invalidation inference is unsound or opaque** | **DEFERRED BY DESIGN** | low | medium | ADR-0007: explicit typed events + transactional outbox are the ground truth; inference may only ever be an auditable optimization layered on top. |
| R9 | **Browser Wasm cannot efficiently access DOM/Web APIs** | **OPEN** | medium | medium | Untested. Fallback per charter: minimal generated JS shim, Wasm for pure compute, direct DOM binding reserved for Servo/standards experiments (M13). |
| R10 | **Multi-agent implementation creates architecture drift** | **OPEN** | medium | high | One integrator; separate worktrees; non-overlapping assignments; mandatory charter+ADR reading (`AGENTS.md`); small merges; full gate tests. No subagents were used in Milestone 0. |
| R11 | **macOS/arm64 hides Linux deployment issues** | **OPEN — actively unmitigated** | **high** | medium | **Linux CI is not wired up.** This is the top infrastructure gap. `just bootstrap` is macOS-only. Charter §13.5 and §20 both require Linux CI for case-sensitivity and reproducibility. First item in `docs/NEXT.md`. |
| R12 | **Performance work compromises semantics** | **OPEN** | low | high | Rule: never weaken a guarantee silently; require an explicit unsafe or policy boundary; benchmark before and after; record the semantic cost. No performance work has happened yet. |

## Risks discovered in Milestone 0

Not in the charter's list. Each was found by running something.

| # | risk | evidence | mitigation |
|---|---|---|---|
| R13 | **Ambient authority arrives through the language runtime, not the application.** A `std` Rust guest imported 15 WASI instances for a one-capability WIT world. | `spike-wasmtime-component.txt` | Generated components must be `no_std`-equivalent. The build must **assert** the component's import list against its declared world; the `ambient` check in the spike host is the prototype. |
| R14 | **Under-measuring our own client JS.** Counting only `<script src>` reported 176 bytes where the true transitive cost was 3,921 — a 20× understatement. | `spike-marko-stream-resume` F-7 | `measure.mjs` walks the ESM import graph. Milestone 3's baseline comparisons against Next/SvelteKit must use the same method, or the benchmark is dishonest in our favour. |
| R15 | **Semantic checks running on parser-recovered syntax produce noise.** The checker reported a warning about a construct the user never wrote. | `spike-compiler-diagnostic` F-4 | Suppress semantic checks when parse diagnostics exist. Milestone 2 needs a general derived-error suppression policy, not a one-off. |
| R16 | **Silent miscompiles in the temporary renderer.** Marko's `>`-in-attribute truncation "usually still compiles clean", producing a handler that never binds. | `spike-marko-stream-resume` F-8 | Generated Marko is machine-produced by the adapter, so `pw` authors never hit it — but the *adapter* must not emit constructs that trigger it. Golden output tests at Milestone 3. |
| R17 | **Benchmark numbers are not comparable to the charter's target machine.** Host is M2 Pro / 16 GiB, not M3 Max / 64 GB. | `docs/environment/macbook.md` | Every benchmark record carries the host string (charter §18.5 already requires it). Milestone 11 VM sizing revised in A-001. |

## Review

Re-read at the start of every milestone (charter §3.1 step 1) and update the
state column against new evidence. A risk is only closed when a measurement
closes it, and the measurement is named.
