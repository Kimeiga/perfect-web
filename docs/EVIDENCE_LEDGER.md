# Evidence ledger

Every claim the project may make in public, what backs it, and — importantly —
**the stronger wording it may not use yet**.

The public site and whitepaper (`web-recompiled/`) must derive their badges and
sentences from this table rather than maintaining separate prose. That is the
mechanism preventing the communication kit from getting ahead of the evidence.

**Status values:** `unstarted` · `partial` · `measured` · `invalidated` · `closed`

`measured` means an artifact in `docs/evidence/` produced by a recorded command
supports the claim **under stated pins**. It does not mean the implementation
milestone is closed.

All measurements below: Apple M2 Pro, 12 cores, 16 GiB, macOS 26.5.2 arm64.
Absolute numbers are **not** comparable to the M3 Max the charter assumes.

---

## Measured

| proof | claim (public wording it may use) | deps | evidence | pins | status | may **not** say |
|---|---|---|---|---|---|---|
| P1 | "A fully static route ships **no** JavaScript at all — no script tag, nothing downloaded." | E0 | `spike-marko-stream-resume.txt` §1 | marko 6.3.32, vite 8.2.0, node 22.21.1 | **measured** | that this holds for *streamed* routes — those carry 849 B of inline patch shim |
| P1 | "Interaction code does not grow with the size of the page: a 9.6× larger document produced 1.03× the client JavaScript." | E0 | `spike-marko-stream-resume.txt` §4 | as above | **measured** | "zero hydration cost"; the shared 3,745 B runtime chunk is still downloaded |
| P1 | "The browser resumes interaction without replaying the component tree — verified by real clicks in Chrome and Safari." | E0, RQ-1 | `spike-browser-resumption.txt` | Chrome 150, Safari 26.5.2, marko 6.3.32 | **measured** | "interaction code loads on demand" — it does **not**; it loads eagerly at 40–55 ms |
| P1 | "Slow regions stream independently; the shell is usable at 3 ms while a 1,200 ms region is still pending." | E0 | `spike-marko-stream-resume.txt` §2 | as above | **measured** | anything about Safari *timing* — RQ-1's Safari path measures end state only |
| P0 | "Effects are inferred and machine-readable: a pure function shows an empty row, a database read shows `database.read`." | E0, RQ-2 | `spike-koka-js-interop.txt` §4 | koka 3.2.3 | **measured** | that this is `pw`'s own checker — it is Koka's, behind an adapter |
| P0 | "An effect cannot hide behind a generic helper: it propagates through unannotated higher-order code, and selective handling preserves the rest of the row." | RQ-2 | `spike-koka-row-polymorphism.txt` | koka 3.2.3 | **measured** | that it holds for `task.spawn` — Koka does not model tasks at all |
| P3 | "A component that is not granted a capability cannot instantiate, and the error names the missing capability." | E0 | `spike-wasmtime-component.txt` §3 | wasmtime 47.0.3, wit-bindgen 0.60.0 | **measured** | "the sandbox prevents ambient authority" — the *declared world* does not describe what the artifact requests (see below) |
| P3 | "A minimal component imports exactly the one capability its interface declares." | E0 | `spike-wasmtime-component.txt` §3 | as above | **measured** | that this is automatic — it required `no_std`; a `std` guest requests 15 WASI interfaces |
| P2 | "Interleaving layout reads and writes costs 264×–848× on identical work; phase scheduling removes it." | E0 | `spike-layout-phase-scheduler.txt` §1 | Chrome 150 | **measured** | that this is a universal speedup — it is workload-specific, and it is a *runtime* result, not a compiler one |
| P5 | "An unrelated state change does not recompute an expensive derived value." | E0 | `spike-bonsai-incremental-model.txt` §1 | incremental v0.16.1 | **measured** | that this is *our* implementation — it is Jane Street Incremental, a design donor |
| P0 | "Compiler errors name the rule, where the offending value came from, and which boundary rejected it." | E0 | `spike-compiler-diagnostic.txt` | rust 1.97.1, annotate-snippets 0.12.16 | **measured** | that the language exists — this is one rule in a toy parser |

## Not yet proven — the claims P0 actually needs

`P0` is the first public artifact and **cannot be published** until these move to
`measured`.

| proof | claim | deps | status | blocking |
|---|---|---|---|---|
| P0 | "An incomplete domain match is rejected even in a fallible function." | E1A | **unstarted** | RQ-3. Koka provably does **not** do this; `pw` must. |
| P0 | "A private cart cannot enter a public materialization." | E5 | **unstarted** | corpus example R-004 exists; no checker |
| P0 | "A secret cannot be serialized into a browser artifact." | E5 | **unstarted** | corpus R-003 exists; no checker |
| P0 | "A task handle cannot escape its component scope." | E2A-S | **unstarted** | RQ-4. Runtime detection cannot support a compile-time claim. |
| P0 | "A non-idempotent command cannot declare a retry policy." | E4 | **unstarted** | corpus R-014 exists; no checker |
| P0 | "A network request in a view does not compile." | E1, E2 | **unstarted** | corpus R-001 exists; no `pw` front end |

**45 corpus files exist and 0 of them compile.** They are a specification, not a
demonstration. Any public wording implying otherwise is forbidden until E2.

## Invalidated — claims the charter made that measurement removed

| claim as originally written | what measurement showed | where |
|---|---|---|
| Koka can serve as the oracle for ADTs, opaque types, and exhaustiveness | exhaustiveness is enforced only for `exn`-free functions; single-field value structs are erased; `Nothing` and `Nil` are both `null` | ADR-0011 |
| `forcedStyleAndLayoutDuration` is the forced-layout signal | not exposed in Chrome 150; long-frame count + `blockingDuration` is the proxy | layout spike F-3 |
| WASI 0.3 is the target | not reachable through stable Rust; `wasm32-wasip2` emits `@0.2.9` | ADR-0008 |
| CSS containment is a safe automatic optimization where legal | 4.18× faster for initial subtree layout, marginally **slower** for a forced layout after an unrelated mutation | layout spike F-4 |
| Bonsai's incrementality extends to rendering | rendering ends at `Patch.create ~previous ~current`, a whole-tree diff | bonsai spike F-2 |

---

## Revalidation triggers

| trigger | re-run | why |
|---|---|---|
| Koka upgrade | `just spike-koka`, `just rq-row-polymorphism` | `.kki` is an internal format; the reader throws on any version but 3.2.3 |
| Rust or Wasmtime upgrade | `just spike-wasmtime` | the emitted WASI version may move off `@0.2.9` |
| Marko / Vite upgrade | `just spike-marko`, `just rq-resumption` | byte budgets and resumption behaviour are the dependency's, not ours |
| Chrome or Safari update | `just rq-resumption`, `just spike-layout` | resumption behaviour and the availability of LoAF fields |
| Replacing Koka (E9) or Marko (E7) | everything above | the oracle changes |

## House rules

1. A claim moves to `measured` only with a named evidence file produced by a
   recorded command.
2. Every `measured` row carries its **forbidden stronger wording**. That column
   is the point of this table.
3. Numbers ship with their pins. A benchmark without a host string and a version
   set is an anecdote.
4. `measured` never means "milestone closed" (`docs/MILESTONES.md`).
5. When a measurement contradicts a claim, the claim moves to `invalidated` and
   stays visible. Deleting it hides the most useful information the project has.
