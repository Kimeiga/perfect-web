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
| P1 | "A page written in `pw` ships zero JavaScript when it needs none, and is usable with JavaScript disabled — verified in Chromium, Firefox and WebKit." | E3 | `docs/evidence/E3/spike-pw-to-marko.txt` | marko 6.3.32, vite 8.2.0, playwright 1.58.0 | **measured** | that `pw` has a renderer — this is Marko behind an adapter (ADR-0002, ADR-0017) |
| P1 | "One interactive control on an otherwise inert `pw` page costs 4,314 B of client JavaScript, and clicking it does not re-render the rest of the page." | E3 | as above | as above | **measured** | that interaction code loads lazily — RQ-1 check 4 **falsified** that for this configuration |
| P1 | "A slow region declared in `pw` streams: the shell was usable 1,201 ms before a 1,200 ms resource arrived, in two chunks, with zero client JavaScript." | E3 | `docs/evidence/E3/spike-pw-to-marko.txt` | marko 6.3.32, node 22.21.1, playwright 1.58.0 | **measured** | that `pw` implements streaming — Marko does, behind the adapter (ADR-0002, ADR-0017) |
| P1 | "Interaction code does not grow with the size of the page: a 9.6× larger document produced 1.03× the client JavaScript." | E0 | `spike-marko-stream-resume.txt` §4 | as above | **measured** | "zero hydration cost"; the shared 3,745 B runtime chunk is still downloaded |
| P1 | "The browser resumes interaction without replaying the component tree — verified by real clicks in Chrome and Safari." | E0, RQ-1 | `spike-browser-resumption.txt` | Chrome 150, Safari 26.5.2, marko 6.3.32 | **measured** | "Marko is the E7 oracle" — it is the oracle for **E7-R only**. See the oracle matrix below. |
| P1 | "Slow regions stream independently; the shell is usable at 3 ms while a 1,200 ms region is still pending." | E0 | `spike-marko-stream-resume.txt` §2 | as above | **measured** | anything about Safari *timing* — RQ-1's Safari path measures end state only |
| P0 | "Effects are inferred and machine-readable: a pure function shows an empty row, a database read shows `database.read`." | E0, RQ-2 | `spike-koka-js-interop.txt` §4 | koka 3.2.3 | **measured** | that this is `pw`'s own checker — it is Koka's, behind an adapter |
| P0 | "An effect cannot hide behind a generic helper: it propagates through unannotated higher-order code, and selective handling preserves the rest of the row." | RQ-2 | `spike-koka-row-polymorphism.txt` | koka 3.2.3 | **measured** | that it holds for `task.spawn` — Koka does not model tasks at all |
| P3 | "A component that is not granted a capability cannot instantiate, and the error names the missing capability." | E0 | `spike-wasmtime-component.txt` §3 | wasmtime 47.0.3, wit-bindgen 0.60.0 | **measured** | "the sandbox prevents ambient authority" — the *declared world* does not describe what the artifact requests (see below) |
| P3 | "A minimal component imports exactly the one capability its interface declares." | E0 | `spike-wasmtime-component.txt` §3 | as above | **measured** | that this is automatic — it required `no_std`; a `std` guest requests 15 WASI interfaces |
| P2 | "Interleaving layout reads and writes costs 264×–848× on identical work; phase scheduling removes it." | E0 | `spike-layout-phase-scheduler.txt` §1 | Chrome 150 | **measured** | that this is a universal speedup — it is workload-specific, and it is a *runtime* result, not a compiler one |
| P5 | "An unrelated state change does not recompute an expensive derived value." | E0 | `spike-bonsai-incremental-model.txt` §1 | incremental v0.16.1 | **measured** | that this is *our* implementation — it is Jane Street Incremental, a design donor |
| P0 | "An incomplete domain match is rejected even in a fallible function — the effect row cannot buy an exemption." | E1A | `docs/evidence/E1A/pw-core.txt`; `compiler/pw-core/tests/differential_vs_koka.rs` | rust 1.97.1 | **measured** | that a `.pw` *file* is rejected — there is no parser yet; the checker runs on a constructed pattern matrix |
| P0 | "An empty `Option` and an empty `List` cannot be confused at a boundary, even where the backend represents both as `null`." | E1A | as above | rust 1.97.1 | **measured** | that this holds for Koka's raw output — it holds for the canonical `pw` ABI, which generated codecs must produce |
| P0 | "A task handle cannot escape the scope that owns it, and an ordinary task cannot be detached without a durable capability — **in a real `.pw` file**, underlining the offending argument." | E2A-S, E2 | `compiler/pw-core/src/scope.rs` (10 tests); `compiler/pw-core/tests/checking_source.rs` | rust 1.97.1 | **measured** | that cancellation, cleanup ordering or leak-freedom hold *statically* — those are runtime behaviour, evidenced separately below |
| P2 | "Cancelling a scope reaches every task beneath it, cleanup runs children-first, and a result arriving after its scope died is refused." | E2A-R | `docs/evidence/E2A/e2a-r-runtime.txt`; `runtime/pw-tasks` (12 tests) | rust 1.97.1 | **measured** | that any of it is a *static* guarantee — these are behaviour tests, and RQ-4 splits E2A precisely so the two are not conflated |
| P2 | "No task is left running when its scope exits, including when a task panics." | E2A-R | as above | rust 1.97.1 | **measured** | that it holds for an async runtime — this is thread-scoped, and E8 has not chosen an execution model (ADR-0016) |
| P2 | "Fifty simultaneous readers of one resource key produce exactly one request, and a duplicated interaction produces exactly one mutation." | E4 | `docs/evidence/E4/resource-runtime.txt`; `runtime/pw-resource` (10 tests) | rust 1.97.1 | **measured** | that a `.pw` program gets this — nothing connects a declaration to the runtime yet |
| P4 | "Private state is held in a separate cache from public state, and cannot appear in public cache output." | E4 | as above | rust 1.97.1 | **measured** | that the *compiler* enforces it — that is E5, and it is not started |
| P0 | "Every policy a query or command declares is visible in `pw explain`." | E4 | `compiler/pw-cli` (asserted against HIR over the accepted corpus) | rust 1.97.1 | **measured** | that the policies are *enforced* — they are recorded and displayed; enforcement is E4's generator and E5 |
| P3 | "A build fails when the compiled component requests a capability it never declared." | E1A, RQ-5 | `compiler/pw-core/src/capability.rs` (7 tests, against E0's measured import lists) | rust 1.97.1 | **measured** | that this runs in the build — it is called from tests only so far |
| P0 | "Compiler errors name the rule, where the offending value came from, and which boundary rejected it." | E0 | `spike-compiler-diagnostic.txt` | rust 1.97.1, annotate-snippets 0.12.16 | **measured** | that the language exists — this is one rule in a toy parser |
| P0 | "An incomplete domain match is rejected **in a real `.pw` file**, naming every missing variant with its field type — `Cancelled(CancellationReason)`, not `Cancelled(_)`." | E2 | `compiler/pw-core/tests/checking_source.rs` | rust 1.97.1 | **measured** | that the corpus is broadly checked — see the row below for the current figure |
| P0 | "**All 44** rejected programs in the charter §16 corpus are compile errors in real `.pw` source, at their own spans, and **each fails for the invariant it declares** — not merely for being red. All 24 accepted programs check clean." | E2, E2B, E2C, E2D, E5, E9, E9C, E7, E6 | `docs/evidence/E2D/corpus-enforcement.txt` (`just evidence-corpus`); `docs/evidence/P0/readiness.txt` | rust 1.97.1 | **measured** | that the **language** is finished, or that these are **complete analyses** — several rules meet their fixture with less machinery than the general problem needs, and `readiness.txt` lists each. Nine fixtures were also given declarations they call; that is recorded there too |
| P0 | "A rule is checked against a negative control that differs from the rejected program in exactly the one way the rule is about — so a rule that banned its construct outright would fail." | E5, E9, E9C, E7, E6 | `examples/rules/**`; `compiler/pw-core/tests/rule_fixtures.rs` | rust 1.97.1 | **measured** | that every rule has one — the test enforces at least one clean fixture **per directory**, not per rule |
| P0 | "What the checks know about the platform — which properties the compositor can animate, which functions acquire and release resources, which event an `on:` attribute delivers, which sinks are public — is declared in `pw` source and read from the signature table, not listed in the compiler." | E2C, E2D, E5, E9, E9C | `packages/pw-platform-web/*.pw`; `compiler/pw-core/src/signatures.rs::no_checker_carries_a_table_of_library_names` | rust 1.97.1 | **measured** | that **nothing** is intrinsic — four frame-phase keywords and the frame-phase permission matrix are in the compiler, and `docs/milestones/E2D.md` says why |
| P0 | "An effect cannot hide behind a helper or a callback **in `pw`'s own source**: the row a declaration writes is checked against what its body does, through local helpers and through lambdas passed to generic functions." | E2D | `compiler/pw-core/src/effects.rs`; `compiler/pw-core/tests/checking_source.rs` | rust 1.97.1 | **measured** | that inference reaches every analysis — the placement solver and the privacy flow still read *declared* rows, so R-025 has a registered rule that cannot fire |
| P0 | "Charter §7.5A's frame phases are checked: a write during `measure`, a measurement after a layout-affecting write, an observation that triggers itself, a property the compositor cannot animate, and a painter that touches the document are each rejected — while the accepted program that differs in exactly the one way the rule is about still compiles." | E2D | `compiler/pw-core/src/layout.rs`; A-018/A-020/A-021/A-023 as negative controls | rust 1.97.1 | **measured** | that this is measured *behaviour* — E0 measured the 264×–848× cost; this is a compile-time rule about the same pattern |
| P0 | "Which properties the compositor can animate, and which writes can invalidate layout, are declared in the platform package rather than listed in the compiler — so the diagnostic's list of permitted properties comes from the signature table." | E2C, E2D | `packages/pw-platform-web/style.pw`; `compiler/pw-core/src/signatures.rs::no_checker_carries_a_table_of_library_names` | rust 1.97.1 | **measured** | that the compiler holds no built-in knowledge at all — four frame-phase *keywords* are intrinsic, and `docs/milestones/E2D.md` says why |
| P0 | "A `.pw` program compiles to Koka, and the result executes with the expected values." | E2 | `docs/evidence/E2/spike-pw-to-koka.txt` | koka 3.2.3, rust 1.97.1 | **measured** | anything about nominal types (E0 F-4: Koka erases them), or effects (only `!{}` functions lower) — see ADR-0015 |
| P0 | "Source layout is canonical and machine-checked: `pw fmt` is idempotent, preserves every token and comment, and CI fails on drift." | E2 | `compiler/pw-syntax/tests/corpus_fmt.rs`; `just ci` | rust 1.97.1, rowan 0.17.0 | **measured** | "canonical layout" — line breaking is **not** normalised, only spacing and indentation (ADR-0013 amendment) |
| P0 | "Every analysis reads compiler-internal ids and source spans, never the syntax tree — enforced by a test, not by convention." | E2 | `compiler/pw-core/src/hir.rs::only_lower_touches_syntax_nodes` | rust 1.97.1 | **measured** | that name resolution or type inference exist — neither does |

## The E7 oracle matrix

**Do not use the undifferentiated sentence "Marko is the E7 oracle."** Check 4 —
interaction code absent until needed — was a pre-registered property and it
**failed in both engines**, which directly falsifies the interaction-lazy claim.
E7 is therefore subdivided:

| E7 property | sub-milestone | Marko status |
|---|---|---|
| No client replay of inert content | E7-R | **accepted oracle**, Chrome + Safari |
| DOM node identity preserved | E7-R | **accepted oracle**, Chrome + Safari |
| Focus and local state survive | E7-R | **accepted oracle**, Chrome + Safari |
| Second interaction avoids reinitialization | E7-R | **accepted oracle**, Chrome + Safari |
| Out-of-order patch placement and replacement | E7-P | **accepted in Chrome** |
| Safari final streamed structure | E7-P | **accepted** |
| Safari streamed arrival **timing** | E7-P | **unmeasured** |
| Interaction code loads only on demand | E7-L | **FAILED — Marko is not the oracle** |
| Handler-load failure behaviour | E7-R | accepted, via the corrected capture-phase test only |

The approved sentence is:

> **Marko is the behavioural oracle for resumption, DOM preservation, and
> patch-placement semantics. It is not the oracle for interaction-lazy code
> delivery.**

The Safari autorun method is acceptable for the properties it can observe. It
**must not** be described as equivalent to the Chrome stream-timing harness.

## Not yet proven — the claims P0 actually needs

`P0` is the first public artifact and **cannot be published** until these move to
`measured`.

| proof | claim | deps | status | blocking |
|---|---|---|---|---|
| P0 | "A private cart cannot enter a public materialization." | E5 | **unstarted** | corpus example R-004 exists; no checker |
| P0 | "A secret cannot be serialized into a browser artifact." | E5 | **unstarted** | corpus R-003 exists; no checker |

| P0 | "A non-idempotent command cannot declare a retry policy." | E4 | **unstarted** | corpus R-014 exists; no checker |
| P0 | "A network request in a view does not compile." | E1, E2 | **unstarted** | corpus R-001 exists; no `pw` front end |

**68 corpus files exist and 0 of them compile.** They are a specification, not a
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
