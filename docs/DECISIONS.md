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
| [0011](DECISIONS/ADR-0011-koka-is-an-effects-only-oracle.md) | Koka is an effects-only oracle; `pw` owns value semantics | Accepted | tape | E9 checker reaches parity |
| [0012](DECISIONS/ADR-0012-adopt-rowan-before-body-parsing.md) | Adopt `rowan` 0.17.0 before body parsing; analyses consume HIR, not syntax nodes | Accepted | reuse | rowan cannot express a needed property |
| [0013](DECISIONS/ADR-0013-formatter-design.md) | Canonical formatting rules; implementation deferred until ADR-0012 lands | Accepted (design) | build | canonical examples drafted |
| [0014](DECISIONS/ADR-0014-hir-representation.md) | HIR is id-indexed arenas per body, with a span on every node; only `lower.rs` sees syntax | Accepted | build | name resolution needs to interleave with lowering |
| [0015](DECISIONS/ADR-0015-koka-backend-scope.md) | The Koka backend lowers a pure subset only, and states what that does not prove | Accepted | tape | the `pw` checker reaches corpus parity (ADR-0001) |
| [0016](DECISIONS/ADR-0016-e2a-r-runtime-shape.md) | E2A-R is a thread-scoped runtime on `std::thread::scope`; its results are behaviour, not guarantees | Accepted | build | E8 selects the host execution model |
| [0017](DECISIONS/ADR-0017-marko-adapter-boundary.md) | The Marko adapter is a one-way lowering from HIR; generated files are build output, never authored | Accepted | tape | E7-R passes the golden suite (ADR-0002) |
| [0018](DECISIONS/ADR-0018-manifest-is-the-compiler-runtime-boundary.md) | The resource manifest is a data artifact; neither compiler nor runtime depends on the other | Accepted | build | E8 selects the host execution model |
| [0019](DECISIONS/ADR-0019-materializer-store-and-outbox.md) | The materializer's state and its outbox share one SQLite database, so a command writes both in one transaction | Accepted | build | E8 selects the host execution model |
| [0020](DECISIONS/ADR-0020-component-contract-is-the-compiler-host-boundary.md) | The compiler hands the host a six-field `ComponentContract`, one per declaration, and actual Wasm imports must be a SUBSET of what it allows | Accepted | build | E9 lowers capabilities and effects into it |

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

**2026-08-05 — one diagnostic code per invariant, not per detector.**
`PW2004` ("a resource cannot outlive the scope that owns it") is emitted by both
the declaration rule and the scope graph, distinguished by a `reason` and
`detector` field. `PW0326` is a deprecated alias resolving to it. Two permanent
codes for one invariant would be wrong; aliasing during migration is fine.

**2026-08-05 — semantic rules may not live in the CLI.**
`pw-syntax` owns syntax, `pw-core` owns meaning and emits one `Diagnostic` type,
`pw-cli` renders. This is what lets a language server, test harness, build
system, playground, AI loop and PR analyser share one checker.

**2026-08-05 — E7 is subdivided.**
E7-R (resumption/DOM), E7-P (patch semantics), E7-L (lazy loading). Marko is the
accepted oracle for E7-R only; it **fails** E7-L. The undifferentiated sentence
"Marko is the E7 oracle" is forbidden.

**2026-08-05 — four small syntax decisions forced by the body grammar.**
None is large enough for an ADR; all four are load-bearing for the tree shape.
*Compound comparisons* (`==` `!=` `<=` `>=`) lex as one token, while `<` and
`>` stay separate because they also delimit type arguments — the joined forms
are unambiguous since a type argument list is never followed directly by `=`.
*An infix operator that can also begin an expression* (`<`, `-`, `!`) may not
begin a line; without the rule, `let s = f(id)` followed by `<main>` parses as
one comparison. *Dotted paths are not absorbed* in expression position: the
parser cannot distinguish `Stores.get` from `store.name` and must not pretend
to, so every `.ident` is a field access and name resolution folds the segments
that turn out to be a module path. *Triple-quoted strings* carry wrapped
`because "..."` justifications, leaving the single-quoted form ending at the
newline, which is the error-recovery property worth keeping.

**2026-08-05 — silent parser recovery is a defect, not a convenience.**
Three closers were consumed with a bare `eat` whose `false` was discarded.
Turning them into diagnostics moved coverage from 56/68 corpus files to 68/68,
because the silence was hiding four real defects — each of which reported its
error one line *after* the cause. Recovery must continue parsing; it must not
continue quietly.

**2026-08-05 — `just ci` runs `clippy -D warnings`.**
Dead code is treated as a signal, not noise: the first `-D warnings` failure
surfaced genuinely unused model surface, which was resolved by *using* it
(`--explain`, a warning-level rule) rather than by silencing the lint.
