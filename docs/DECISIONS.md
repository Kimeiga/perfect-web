# Decisions

Index over `docs/DECISIONS/ADR-*.md`. One ADR per consequential decision, written
**before** the change it authorizes (PROJECT_CHARTER.md §3.1 step 3).

Status values: `Proposed` · `Accepted` · `Superseded by ADR-NNNN` · `Rejected`

| ADR | Decision | Status | Strategy | Deletion / revisit condition |
|---|---|---|---|---|
| [0001](DECISIONS/ADR-0001-koka-as-temporary-semantic-compiler.md) | Koka 3.2.3 is a temporary semantic oracle, not the permanent compiler | Accepted | tape | Milestone 9 checker reaches corpus parity |
| [0002](DECISIONS/ADR-0002-marko-as-temporary-renderer.md) | Marko 6.3.32 is the temporary renderer behind an adapter | Accepted | tape | Milestone 7 renderer passes the golden suite |
| [0003](DECISIONS/ADR-0003-rust-as-permanent-implementation-language.md) | Rust 1.97.1 is the permanent compiler/host language | Accepted | build | Dependency MSRV exceeds the pin |
| [0004](DECISIONS/ADR-0004-preserve-html-css-http.md) | Preserve standard HTML, CSS, URLs and HTTP | Accepted | reuse | Never wholesale; narrow additions at M13 |
| [0005](DECISIONS/ADR-0005-sqlite-first-data-layer.md) | SQLite first, PostgreSQL when nodes separate | Accepted | reuse | Milestone 11 |
| [0006](DECISIONS/ADR-0006-wasmtime-capability-host.md) | Wasmtime 47.0.3 capability host; effects become WIT imports | Accepted | reuse | A capability cannot be expressed in WIT |
| [0007](DECISIONS/ADR-0007-explicit-invalidation-before-inference.md) | Explicit typed invalidation events before any inference | Accepted | build | After Milestone 6, as an auditable optimization only |
| [0008](DECISIONS/ADR-0008-wasi-0.2-not-0.3.md) | Target WASI 0.2; 0.3 not reachable through the stable toolchain | Accepted | reuse | Rust ships stable `wasm32-wasip3` |
| [0009](DECISIONS/ADR-0009-parser-and-diagnostics-stack.md) | Hand-written parser + `annotate-snippets` 0.12.16 diagnostics | Accepted | build | Milestone 2 lossless-tree design |
| [0010](DECISIONS/ADR-0010-provisional-pw-extension.md) | `.pw` is the provisional source extension | **Proposed** | build | Milestone 2 task 1 owns the real decision |

## Decisions the charter asked for and where they landed

Charter §14 Milestone 0 task 5 lists eight required ADRs. All eight exist:

| charter item | ADR |
|---|---|
| Koka as temporary semantic compiler | 0001 |
| Marko as temporary renderer | 0002 |
| Rust as permanent compiler/host language | 0003 |
| standard HTML/CSS/HTTP preservation | 0004 |
| SQLite-first data layer | 0005 |
| Wasmtime capability host | 0006 |
| no browser fork before profiling | see below |
| explicit invalidation before automatic dependency tracking | 0007 |

ADRs 0008–0010 were added because Milestone 0's spikes forced decisions the
charter left open (the WASI version) or that the corpus needed immediately
(the diagnostics stack, the file extension).

**"No browser fork before profiling"** is deliberately *not* an ADR. It is a
standing constraint in the charter itself (§11.4, §14 M13) and there is no
decision to record until Milestone 13 produces profiling data. Recording an ADR
that says "we did not do the thing we were told not to do" would add
ceremony without content. It is tracked in `docs/vision/non-goals.md`.

## Decision log

Short entries for choices that shaped the repository but are too small for an ADR.

**2026-08-05 — `docs/`-scoped STATUS/DECISIONS/ASSUMPTIONS.**
The charter mandates `docs/STATUS.md` and `docs/DECISIONS/ADR-*.md`. The three
requested filenames live under `docs/` rather than at the root so there is one
source of truth. Recorded as assumption A-006.

**2026-08-05 — The original prompt file became `PROJECT_CHARTER.md` and was removed.**
Byte-identical copy verified by SHA-256 (`d37dfa2d…a90e4c`) before deleting the
duplicate, so there is exactly one constitution in the repository.

**2026-08-05 — `wasm32-wasip2` spike crates are excluded from the Cargo workspace.**
They target wasm and must not be pulled into a host-target
`cargo test --workspace`. `spikes/wasmtime-component/run.sh` builds them.

**2026-08-05 — Node 22.21.1 rather than Node 24.**
Both `vite@8.2.0` (`^20.19.0 || >=22.12.0`) and `marko@6.3.32` (`>=22`) are
satisfied. Pinning what was actually tested, per charter §3.6. Assumption A-002.

**2026-08-05 — `just ci` runs `clippy -D warnings`.**
Dead code is treated as a signal, not noise: the first `-D warnings` failure
surfaced genuinely unused model surface, which was resolved by *using* it
(`--explain`, a warning-level rule) rather than by silencing the lint.
