# Requirements, priorities, and next research decisions

Parts 8–14. This document proposes acceptance criteria and changes. It does not amend accepted syntax or claim production readiness. Failure/status authority remains `CENSUS.md`; evidence scope remains `SOURCES.md`.

## Part 8. Developer-experience requirements

### Author meaning once, not mechanism repeatedly

A normal application feature should not require separately authored dependency arrays, memoization annotations, client/server DTOs, serializers, API wrappers, route-string builders, repeated validation, copied loading/error state, effect cleanup, abort controllers, retry loops, cache keys, invalidation tags, code-splitting directives, or placement directives **when those express facts already present in the supported semantic model**. Explicit external change contracts, product retry policy, and genuinely distinct draft/schema versions are not accidental duplication. [R1] [R11]

Acceptance must measure concepts and successful changes, not only line count. Use the same task, fixtures, browser behavior, accessibility requirements, failure handling, and domain policy for Pleris and the comparison implementation. Count authored semantic facts, repeated facts, manual coordination points, unsafe boundaries, required annotations, and time to diagnose/fix a seeded bug. Do not infer human usability from compilation time alone.

### Required editing and explanation behavior

| Requirement | Acceptance witness |
|---|---|
| Local inference remains readable | New contributor explains which effects, privacy, owner, and placement a small feature has using ordinary source and on-demand hints. |
| `pw explain` exposes causes | Explain why a request ran, which input changed, who owns it, why it was retried/canceled/shared, and which revision was delivered. |
| Placement is inspectable | Explain why work is at browser/edge/origin, what prevents caching, which data prevents public placement, and what alternative admissible plan exists. |
| Code shipment has provenance | Explain which reachable interaction or dependency caused every shipped runtime/chunk and show total transitive bytes. |
| Diagnostics repair the authored problem | Point to the conflicting semantic declarations and causal path; do not expose only a generated WIT/JS failure. |
| Rename is semantic | Rename a domain type or command and mechanically update derived projections without changing an unrelated same-spelled symbol. |
| IDE uses one semantic model | Hover, completion, references, semantic diffs, and compiler checks agree on resolved identities and contracts. Partial editor models never authorize deployment. |
| Incremental feedback is measured | Measure cold/warm parse, check, codegen, explain, and editor update latency after body, signature, policy, and dependency edits. Record invalidated query counts. |
| HMR preserves only valid state | Preserve compatible local drafts and UI state; visibly reset/migrate incompatible state; never repeat a committed command because code reloaded. |
| Debugging stays at source level | Trace click → intent → authorization → command → commit → outbox → invalidation → resource revision → DOM part. |
| Replay is explicit about limits | Replay supported effects in isolation, redact sensitive values, and stop at unrecorded or unsupported foreign boundaries. |
| Review shows semantic changes | Capability, privacy release, schema compatibility, placement, cost/fanout, and recovery-plan changes appear in PR/deployment diffs. |
| Unsafe integrations remain usable | One named adapter explains lost guarantees and granted authority; normal application code does not carry adapter plumbing everywhere. |
| Documentation follows semantics | Generate signatures, routes, capabilities, and boundary examples from declarations, while preserving independently authored explanations and examples. |

Do not set a fabricated universal “sub-100 ms” acceptance threshold. Establish interactive latency budgets from representative projects, hardware, edit types, and usability evidence, then report them separately from aspirational goals. The rust-analyzer architecture supplies a checked model of incremental derived semantic state, not a measured speed promise for Pleris. [S75]

### Language-design admission review

| Idea | Pleris benefit to evaluate | Complexity or unsoundness risk | Admission position |
|---|---|---|---|
| Nullability and ADTs | Make absence, partial data, failure, and unknown commitment explicit | Collapsing alternatives in erasure/interop defeats the surface types | Core, with cross-backend witnesses |
| Exhaustiveness | Evolving domain states force consumer review | Closed-world checks cannot reject legitimate unknown foreign variants | Core for closed types, explicit open boundary variants |
| Nominal versus structural types | Protect domain IDs while retaining pleasant record composition | Nominalizing every record increases ceremony; structural IDs lose meaning | Selective nominal identity, coherent resolved representation |
| Refinement types | Reuse value constraints for decoding and database checks | General solver cost and false confidence about runtime state | Bounded fragment plus checked constructors |
| Dependent types | Potentially relate dimensions or protocol indices | Ordinary authors should not need proof terms for forms and CRUD | Research only unless a concrete task becomes simpler |
| Exceptions versus typed results | Preserve expected failure and recovery choices | Silently catching foreign exceptions can manufacture success | Typed domain outcomes, explicit foreign exception conversion |
| Effect rows and row polymorphism | Compose helpers without restating every effect | Hidden effect provenance, solver growth, opaque foreign calls | Inferred locally with visible public/adaptor boundaries |
| Algebraic handlers | Centralize mechanism and test effects | Reordering/replaying a commitment can violate its protocol | Restrict handlers by commitment/lifetime semantics |
| Capabilities | Restrict the operations available to a component | Static tokens are not actual host isolation | Core admission plus live host enforcement |
| Information-flow labels | Track sensitive data to serialization and telemetry | Implicit flow, dynamic principals, metadata, and intentional release | State the supported model; infer ordinary propagation |
| Integrity/endorsement | Prevent valid-shaped untrusted claims from becoming authoritative | Too many provenance annotations can burden ordinary code | Start at security-sensitive adapters and domain facts |
| Affine/linear resources | Prevent leaks, double ownership, and invalid transfer | Browser APIs do not all obey a simple lexical lifetime | Inferred scoped ownership, small explicit transfers |
| Borrowing/uniqueness | Avoid copies at ABI boundaries where lifetime is clear | Importing a full borrow-checking burden into UI code | Prefer adapter-local use, measure byte/latency benefit |
| Typestate | Prevent use-before-open/use-after-close and invalid protocol order | State changes can occur externally or concurrently | Small library state machines plus runtime checks |
| Session types | Potentially check narrow protocol conversations | Dynamic peers, reconnects, and external protocol versions complicate proofs | Use only where a concrete adapter becomes simpler |
| Totality | Bound selected pure computations and rule evaluation | Universal termination checking rejects useful programs or needs heavy proof | Budget untrusted work; optional restricted total fragments |
| Gradual typing/contracts | Support incremental adoption | Unchecked casts can silently erase guarantees | Unknown enters only through explicit checked/audited boundaries |
| Module/package identity | Preserve nominal and capability identity across linking | Global string names and duplicate runtime instances create aliasing | Stable identity with explicit version and authority |
| Content-addressed code | Stable artifacts and reusable builds | Availability, compatibility, and trust do not follow from a hash | Derive content IDs, retain supported inputs/artifacts separately |
| Incremental/query-based compilation | Fast checks and explanations from one model | Unsound invalidation keys give stale proofs | Core engineering requirement with mutation tests |
| Deterministic builds | Inspectable reproducible projection from source | Untracked toolchain/env, compromised builder, and malicious source | Hermetic inputs plus separate provenance and trust review |
| Incremental computation | Avoid repeated resource/query/UI work | Missing negative or external dependencies produces stale results | Conservative dependency contracts first, narrower inference when proved |

This is an adoption filter, not a proposal to incorporate every named system. The existing matrix already contains Koka, Effekt, Elm, Gleam, Rust, Links, Ur/Web, Unison, Roc, Skip, Bonsai/Incremental, and other donors. Their presence is not a reason to add another visible language construct. [R13]

### AI-written application evaluation

Do not create duplicate AI-only failure leaves for ordinary effect, schema, auth, resource, or UI bugs. Use the same corpus:

| Coding-agent failure | Existing invariant reduction | Remaining limit |
|---|---|---|
| Forgotten cleanup, stale callback | C01–C06 | An opaque SDK's undocumented behavior still needs an adapter witness. |
| Wrong library API or hallucinated field | A01, A02, A08, W01, W02 | A fabricated adapter contract is not verified merely because its declaration compiles. |
| Duplicate schemas or wrappers | J10, V01, duplication census | Compiler cannot infer arbitrary semantic equivalence between independently invented models. |
| Missing auth or wrong tenant | D01–D05, E06 | The policy's business intent must still be supplied and reviewed. |
| Inappropriate retry or rollback | G01–G07, H05 | Provider contracts and compensation policy remain external/domain inputs. |
| Dependency arrays/unnecessary effects | B01–B07 | Deliberate feedback and unobservable external dependencies remain explicit. |
| Unsafe cast or missing variant | A03, A05, A08, V08 | Explicit unsafe authority and open foreign variants require review. |
| Wrong placement or cache policy | E01, J01–J10, T08 | Real freshness/consistency/cost preferences are not guessed. |
| Silent error swallowing | B08, G06, W02 | Intentional best-effort behavior is a valid explicitly weaker policy. |
| Inaccessible or slow UI | N01–N10, P01–P09, performance suite | Human accessibility/usability and actual devices remain necessary. |
| Prompt material becomes deploy authority | X06, S03, V08 | Agent tool permissions and trusted review are separate from source-language safety. |

Use repeated fixed tasks, pinned agent/model configurations, independent domain oracles, identical available information, and separate compilation, functional, security, accessibility, performance, and repair outcomes. Record sample sizes and uncertainty; do not use an LLM judge as the sole correctness oracle. The checked older AI-assistant study motivates testing confidence as well as code, but establishes no current model ranking or error rate. [S76] [R1]

## Part 9. Performance requirements and benchmark suite

### Three kinds of claim

**Invariant:** a behavior or bound the implementation promises under stated assumptions, such as no application JavaScript emitted for a truly static page. **Measurement:** observed bytes, time, work, memory, or correctness under a disclosed workload. **Aspiration:** match or outperform the strongest measured semantically equivalent implementation on a named workload. No aspiration below is reported as achieved.

“Fastest for every program and every browser” is not a demonstrated property. Preserve performance as a first-class requirement by comparing admissible execution plans and reporting the full tradeoff vector. Do not hide more origin work, bandwidth, privacy exposure, or first-interaction latency behind reduced hydration. [R1] [R11]

| Benchmark | Behavioral invariant | Measurements | Optimization responsibility / aspiration |
|---|---|---|---|
| B01 Static document | No app/runtime JS for content with no required scripted behavior | All HTML/CSS/JS/Wasm and third-party bytes; requests; parse/execute work | Compiler; zero application JS is an output assertion, not an invented timing target |
| B02 Cold first interaction | Action remains correct with uncached lazy code, including failure and activation needs | Input delay, module fetch/parse, handler work, next paint, outcome latency | Compiler/runtime/browser; compare lazy and eager admissible plans |
| B03 One changed dependency | Only semantically dependent work is invalidated; identity is preserved | Recomputations, DOM writes, style/layout, affected part fanout | Compiler/runtime; not a universal promise of one mutation |
| B04 Repeated keyed updates | State follows entities through insertion, reorder, deletion, and duplicate instances | Mutations, identity/selection/focus checks, CPU, memory | Compiler/runtime; compare with independent native baseline and relevant framework |
| B05 Request DAG | No serial dependency introduced solely by incidental rendering | Initiation graph, required versus avoidable waterfalls, duplicate work, end-to-end time | Compiler where dependence known; domain-dependent otherwise |
| B06 Query graph | Batching preserves auth, snapshots, order, limits, and errors | Database round trips, N+1 count, rows scanned/returned, plans, payload bytes | Compiler + adapter + profiles; do not batch incompatible operations |
| B07 Payload/bundle economy | No unused reachable mechanism required by the chosen feature set | Transitive dependencies, duplicate packages, overfetch, serialization and parsing cost | Compiler/linker; browser cannot skip parsing bytes already sent by magic |
| B08 Resource sharing | Concurrent subscribers share only equivalent authorized work | Calls, subscriber lifetimes, aborts, cache entries, server work | Runtime; test high-cardinality keys as well as one hot key |
| B09 Cache policy | Correct audience/freshness/version reuse at every layer | Hit/miss by reason, staleness, refresh calls, invalidation fanout, origin work | Compiler/runtime/deployment; hit rate alone is not correctness |
| B10 Fault/retry recovery | Bounded attempts and queue growth with explicit outcomes | Retry tree, offered/admitted work, backlog, shed work, tail latency, recovery time | Runtime/host; measure under partial failure and recovery spikes |
| B11 Tenant isolation | One workload cannot exceed the declared shared-resource policy | Per-tenant CPU/memory/connections/queries/egress and latency | Host/runtime; fairness policy is explicit |
| B12 Layout phases | No prohibited generated read/write interleaving | Trace layout/style events, script attribution, phase trace, DOM size | Compiler/runtime/browser; rerun corrected LoAF probe with positive controls |
| B13 Animation and observers | Interaction and user motion preferences remain correct; feedback is bounded | Frame work, observer iterations, layout/paint/compositing cost, long tasks | Runtime/profiles; browser limitations and legitimate visual requirements remain |
| B14 Large documents | Required find/copy/print/accessibility behavior survives rendering strategy | DOM size, render/style/layout time, memory, task completion | Domain/profile-guided choice; no mandatory universal virtualization |
| B15 Assets/fonts | Layout policy reserves or deliberately handles asynchronous content | LCP subparts, asset bytes, font behavior, initial and post-load CLS | Compiler for known assets; domain/browser for unknown content |
| B16 Memory endurance | Released owners have no unintended retained resources under the model | Reachability, active handles/listeners, steady-state heap trends, detached DOM | Runtime; GC timing is not a deterministic zero-memory assertion |
| B17 Navigation/restore | Drafts, scroll, selection, focus, history, and authority survive correctly | Navigation/restore timing, resumed work, requests, state assertions | Runtime/browser; test bfcache, cold load, back, and interrupted navigation |
| B18 Mobile lifecycle | Backgrounding, keyboard, orientation, eviction, and slow CPU preserve outcomes | Real-device interaction, CPU, memory, energy proxies if available, storage failures | Browser/runtime/domain; automation alone is insufficient |
| B19 Edge/origin startup | Host plan meets semantics during cold start, overload, and restart | Startup latency, compile/instantiate time, connection reuse, origin calls, CPU | Deployment/profiles; no fixed universal cold-start budget invented |
| B20 Long-lived versions | Old/new clients, workers, origins, and data remain admitted or recover safely | Compatibility outcomes, artifact retention, recovery time, draft preservation | Compiler/runtime/deployment |
| B21 Streams/media | Bounded queues and correct cancellation/reconnect/permission behavior | Throughput, queue bytes, dropped/coalesced messages, replay gaps, media resource lifetimes | Runtime/browser/adapter; measure each supported protocol |
| B22 Compiler/IDE | Small changes preserve unaffected cached semantic facts | Cold/warm parse/check/codegen, invalidated queries, memory, explain/LSP response | Compiler; establish representative budgets empirically |
| B23 Replay/debugging | Instrumentation does not alter commitments or leak protected values | Capture/replay fidelity, overhead, trace bytes, redaction, missing event detection | Runtime/tooling; sampled traces cannot claim complete replay |
| B24 Full feature migration | Equivalent Kiokun feature has no duplicated mechanism and preserves required behavior | Authored facts, bytes, CPU, latency, origin work, accessibility, debugging/repair effort | End-to-end aspiration, not a prerequisite to fixing E9 |

**Measurement controls:** pin code, browser builds, operating system, hardware, power state where relevant, network/storage conditions, workload, warm/cold state, and correctness oracle. Include multiple runs and distributions rather than only the best sample. Record unsupported and sampled metrics. Validate a probe with a positive control and an independent signal before interpreting its absence. Attribute `forcedStyleAndLayoutDuration` to script records; do not treat a lack of long-animation-frame entries as proof of zero forced layout. [S64] [S61]

Compare current framework implementations only after building semantically equivalent fixtures with the same functionality. This census did not execute or rank current React, Vue, Angular, Svelte, Solid, Qwik, Marko, Astro, Next, Nuxt, Remix, Ember, Lit, or htmx releases. Existing donor mechanisms suggest candidate baselines, not benchmark winners. [R13]

### Performance pathology routing

Excess JS, hydration work, unused code, bundle duplication, and serialization are compiler/linker opportunities inside the known graph. Unnecessary reactive work and DOM mutations require coherent dependency/identity semantics. Waterfalls, N+1, overfetching, and underfetching need query/effect knowledge; arbitrary provider behavior remains opaque. Long tasks, layout, animations, DOM size, memory retention, cold starts, and network latency require runtime, host, and profile evidence. Images, fonts, cache partitioning, invalidation, startup, INP, navigation, and mobile constraints cross these boundaries. The benchmark table deliberately measures both removed work and costs moved elsewhere. [R1] [S60] [S61] [S62] [S63]

## Part 10. Threat-oriented security and privacy requirements

The trusted computing base includes the compiler/lowering pipeline, admitted runtime, actual host adapters, cryptographic/session implementations, database semantics, build service, and browser/OS. A threat model must name which of these may be malicious, compromised, stale, or merely faulty. “Compiled” is not itself a security boundary.

| Threat | Static / derived | Runtime / deployment enforcement | External dependence and remaining policy |
|---|---|---|---|
| XSS and unsafe rich content | Context-specific sink types, generated escaping, explicit HTML authority | Correct sanitizer and browser sinks; supported CSP/Trusted Types policies | Rich-content rules and browser implementation. Trusted Types policy does not certify sanitization. [S21] [S27] |
| CSRF/session confusion | Command/authentication-mode contract and generated native/enhanced paths | Origin/interaction checks, appropriate cookie/session lifecycle, token binding | Identity provider, browser cookie behavior, explicitly supported cross-origin flows. [S23] [S28] |
| CORS confused with authorization | Separate approved consumer origins from command permission | Serve policy at ingress; authorize the command regardless of headers | Cross-origin integrations are domain/deployment choices, not inferred from arbitrary outgoing fetches. |
| Clickjacking and unauthorized features | Declared embedding and powerful-feature needs | Generate supported frame/permission policy; validate deployed headers | Legitimate embeds, OAuth/payment popups, browser support. Do not blindly impose isolation that breaks required integrations. [S26] [S52] |
| Tenant leaks and confused deputy | Scoped references, inferred authority, no ambient host clients | Enforce tenant/resource/action on reads, writes, joins, caches, and subscriptions | Current principal/ACL state and the declared consistency contract. [S18] [S24] |
| Revocation/TOCTOU | Require a live-authority protocol in the plan | Atomic checks, revision predicates, causal constraints, bounded grants as applicable | Chosen revocation behavior during partitions; static typing cannot know the live ACL. [S18] |
| Delegation/session fixation/impersonation | Narrow grants and distinct actor/subject | Authenticate context, rotate/rebind sessions as required, preserve actor chain | Audited credential/identity adapters and explicit delegation policy. [S19] [S23] |
| SSRF and credential forwarding | Restricted network capabilities | Check actual destination/redirect and enforce egress isolation | DNS/network/proxy behavior and allowed destination policy. [S22] |
| SQL/command injection | Structured query/argument APIs | Parameterized driver/process boundary and restricted identifiers | Trusted adapter/driver; arbitrary unsafe raw syntax is separately authorized. |
| Malicious serialized payload | Closed versioned data schema; no arbitrary constructors | Bounded decoder, host dispatch checks, fuzzing of emitted endpoints | Runtime implementation remains part of TCB. [S11] [S71] |
| Prototype pollution / malicious JS | Inert data boundary, explicit effects, named isolation mode | Safe representations, real compartment/worker/frame/host boundary | Same-realm trusted mode explicitly weakens containment; SES alone does not prevent shared-agent exhaustion. [S29] |
| HTTP smuggling/cache poisoning | One normalized routing/cache contract | Strict ingress and compatible intermediary framing; deployed-chain tests | Third-party proxy/CDN implementations not compiler-controlled. [S12] [S17] |
| Secret serialization or diagnostic leakage | Dataflow to captures/errors/logs/URLs and metadata | Runtime principal partitions, redaction before export, access control on traces | Recipient endpoint, metadata/timing channels beyond the stated IFC model. [S20] [S25] |
| Wrong trusted provenance | Separate integrity endorsement from decoding and secrecy | Verified provider/source state or authoritative recomputation | Trust in designated authorities; shape validation is insufficient. [S20] |
| Replay/idempotency/commerce race | Replay-safe command admission and intent identity derivation | Atomic journals, provider contract, reconciliation, live state-transition guards | Provider idempotency window and irreversible business effects. [S13] [S14] [S15] |
| Resource-exhaustion attack | Require bounded boundary/host plans | CPU, memory, parse depth, queues, query, connection, and tenant budgets | Physical capacity and browser preemption limits. [S02] [S33] |
| Supply-chain compromise | Expected package authority and admitted capability diffs | Hermetic isolated builds, provenance verification, actual artifact admission | Malicious approved source remains possible; signature is not safety. [S09] [S30] [S31] |
| Privacy purpose/retention/residency | Declared sink/data lifecycle/location constraints | Dispatch checks, managed lineage deletion, host location verification | Product/legal requirements and unmanaged recipients are not compiler-inferred. [S25] |
| Control-plane compromise or recovery failure | Separate destructive capabilities and plan scope | Staging, independent recovery paths, restore drills, protected signing | Operator/host trust and recovery objectives remain explicit. [S03] [S06] |

Add adversarial tests for internal-header spoofing, forged handles, duplicate/malformed wire tags, cache audience aliases, permission change between read and write, account switch with pending work, foreign callback reentrancy, redacted error paths, hostile package authority, and rolling-version replay. These test goals do not require publishing exploit payloads.

## Part 11. Prior art missing from the inspected technology matrix

“Missing” here means absent from the checked `docs/research/technology-matrix.md`, not necessarily absent from every conversation or file. Borrow the stated lesson, not an entire implementation.

| Addition | Exact lesson | Proposed use |
|---|---|---|
| Zanzibar | ACL/content causal consistency is part of authorization correctness. [S18] | Specify temporal authority before API/cache schemas freeze. |
| Macaroons | Delegated authority can be attenuated by context. [S19] | Model scoped delegation without requiring a particular credential format. |
| Jif and its referenced IFC lineage | Confidentiality and integrity are separate; release/endorsement are explicit. [S20] | Prototype boundary-focused IFC with readable diagnostics. |
| CALM | A restricted monotone fragment can justify coordination avoidance. [S36] | Optional proven optimization, not universal inference. |
| Invariant confluence | Coordinate based on declared invariant preservation, not a blanket storage slogan. [S35] | Analyze selected replicated operations and their nonconfluent counterexamples. |
| FoundationDB simulation | Deterministic fault schedules make distributed counterexamples reproducible. [S32] | Expand E11 with recorded schedules and independent state invariants. |
| Temporal | Durable execution binds histories to replay-compatible commands/code. [S37] | Model workflow versioning and commitment boundaries. |
| Automerge | Convergence can retain conflicts instead of proving product correctness. [S39] | Explicit replicated-value conflict surfaces. |
| Yjs selective undo | Undo is associated with operation origins and user context. [S40] | Evaluate optimistic/collaborative intent removal instead of whole-snapshot restoration. |
| Local-first research | Agency, offline capability, and preservation are distinct goals. [S38] | Define local versus remote durability and export/recovery outcomes. |
| SES/Hardened JavaScript | Authority restriction needs actual compartments/endowments; availability isolation is separate. [S29] | Specify trusted/isolated JS interop modes honestly. |
| SLSA | Provenance, source review, builder integrity, package selection, and runtime trust are separate. [S30] | Derive build evidence without treating signatures as semantic safety. |
| Bazel hermeticity | Actual build input closure determines cache correctness. [S31] | Compiler/deployment reproducibility and remote-cache admission. |
| rust-analyzer | One incremental derived semantic model supports editor operations over incomplete source. [S75] | `pw explain`, LSP, refactoring, and strict build/lenient editor split. |
| Unicode UAX 9/29 and W3C bidi/writing modes | Text units and direction are not ASCII string operations. [S65] [S66] [S68] [S69] | First-class text boundary and layout conformance fixtures. |
| Temporal date/time concepts | Civil time and exact time have different conversion and recurrence rules. [S67] | Domain time types and explicit ambiguity policy, without claiming current browser support. |
| Protobuf schema-evolution lessons | Retired identities and wire/name compatibility require history. [S70] | Generalize compatibility beyond current type equality. |
| GraphQL partial outcomes | Data and errors may coexist. [S72] | Safe external protocol adapters that do not flatten meaningful states. |
| HTML activation, Input Events, IndexedDB, lifecycle standards | Browser-owned protocols can invalidate an otherwise valid application operation. [S43] [S53] [S74] | Activation-aware chunking, editing drafts, transaction lifetime, and restore tests. |
| Concrete incidents S01–S12 | Triggers, amplification, control-plane recovery, and decoding boundaries differ. | Add bounded witnesses without claiming Pleris eliminates every outage at those companies. |

### Candidates requiring another source-reading pass

TLA+/PlusCal, Alloy, P, CompCert, Alive2, Liquid Haskell, LIO/Viaduct, Ur/Web's formal guarantees in detail, TUF, reproducible-build bootstrapping, relational lenses/bidirectional schema evolution, Flink-style watermarks, CRDT rich-text/move semantics, and older web-framework failure retrospectives deserve focused evaluation. They are **not recommendations supported by a full paper/implementation audit in this pass**. Do not add language features merely from these names.

## Part 12. Recommended changes to Perfect Web

This research addition is deliberately separate from accepted ADRs. The following edits should be implemented through the existing design and milestone process, with leaf IDs as traceability anchors.

| Repository area | Specific change | Acceptance / non-goal |
|---|---|---|
| `PROJECT_CHARTER.md` | Clarify “one fact, one authority” as one governing semantic fact per identity/scope/version, with derived projections and independently enforced boundaries. | Do not collapse drafts into domain values or tests into generated self-oracles. |
| `PROJECT_CHARTER.md` | Add explicit temporal authority, commitment outcome, compatibility-set, budget-composition, and browser-owned interaction contracts. | Do not introduce new syntax before readable accepted examples and counterexamples. |
| `PROJECT_CHARTER.md` | Replace any universal interpretation of virtualization, zero forced layout, or best performance with semantically qualified invariants and named workloads. | Preserve performance ambition without asserting unsupported dominance. |
| Accepted/rejected corpus | Add CF obligations as supported fixtures, each with accepted neighbor, valid prerequisites, diagnostic/span, and mutation control. | Do not count this prose as executable `pw check` tests. |
| Runtime corpus | Add RT schedules against real generated runtime/host, starting with optimism, auth revision, compatibility, and cancellation/commit. | Passing a small Python model is only design evidence. |
| `docs/RISK_REGISTER.md` | Link the P0/P1 register below; add specific LoAF measurement correction and composition risks. | Preserve old evidence and narrowly revise its interpretation. |
| `docs/RISK_QUEUE.md` | Record LoAF scope counterexample and the required positive control; add overlapping-intent restoration witness. | Do not call the optimism scenario a reproduced implementation bug. |
| `docs/NEXT.md` | Keep E9 repair and E10 compiled command execution first. Link census decision dependencies before further protocol/interface freeze. | No interruption of the current resolved-type migration branch. |
| `docs/MILESTONES.md` | Add explicit compatibility/authority/budget/browser gates around E10/E11; move contract decisions forward from broad E15 hardening. | Keep E13 browser-native work optional and later. |
| E11 network lab | Add bounded retry/reconnect, principal changes, stale replicas, ambiguous commits, outbox interruption, old/new versions, and recovery control plane. | Include both transport faults and domain invariants. |
| E14 tooling | Add semantic authority/projection audit, source-level explainability, compatible HMR, causal replay boundaries, and AI task evaluation. | No probabilistic correctness judge standing in for witnesses. |
| Benchmark suite | Add B01–B24; fix both layout probes and remeasure before correcting claims. | No invented replacement numbers. |
| `docs/research/technology-matrix.md` | Add checked donors above, their narrow lesson, implementation cost, and a reject/borrow/evaluate decision. | Research source presence is not automatic feature admission. |
| `docs/SEMANTICS.md` and `docs/ARCHITECTURE.md` | Label old no-compiler snapshots historically or derive a current front matter from the evidence ledger. | Do not rewrite historical observations to look current. |
| `docs/STATUS.md` and evidence ledger | Tie current claims to exact revision, prerequisite, witness, host/browser scope, and remaining limitations. | “Covered” cannot silently expand from core Wasm to full component execution. |
| Kiokun pilot | Select one real authenticated note/editor flow with draft, cache, version, and offline policy after core gates pass. | Demonstrate less authored machinery and equivalent behavior, not merely rendering a button. |

### Concrete proposed amendments to preserve for ADR review

**Authority law:** Every managed semantic fact has one declared governing authority for its identity, scope, and version. Dependent representations are mechanically derived. A distinct business policy, draft state, historical version, or independent verification oracle is not duplicate merely because its shape resembles another.

**Commitment law:** Effects that can commit outside the caller's observation must distinguish intent from attempt, cancellation from rollback, and uncertain from rejected outcome. Automatic retries require a supported commitment contract at the authority that performs the effect.

**Compatibility law:** Deployment admission considers every supported live producer, consumer, persisted state, queued intent, and workflow version. Excluding a version requires an explicit non-destructive recovery or retirement policy.

**Browser boundary law:** Generated code preserves supported native editing, navigation, activation, accessibility, lifecycle, and user-content affordances. It cannot manufacture browser authority or rely on guaranteed final callbacks.

**Evidence law:** A guarantee is published only with its exact assumptions, scope, revision, and witness. Missing measurement is not zero; an earlier parser failure is not proof of a later semantic check; a valid artifact is not proof of correct lowering.

These are proposed wording for review, not silently accepted replacements for the charter or ADR25.

## Part 13. Priority

P0 means a foundational omission or incomplete contract that can make later architecture expensive. It does **not** mean implement every P0 feature immediately or abandon the current type repair. P1 is required before a production pilot; P2 before broad adoption; P3 later/native standards work.

| Priority | Risk / decision | Leaf anchors | Next evidence |
|---|---|---|---|
| P0, existing blocker | Complete semantic type checking and one-way contract/ABI projection | A01 A02 A08 S07 S10 | Valid argument/result witnesses, independent ABI validation, compiled `add_to_cart` through real host |
| P0 | Temporal authorization, scoped principals, integrity versus shape | D03 D04 D05 E02 E06 Q07 | Account switch/revocation/TOCTOU schedules and one readable domain example |
| P0 | Commitment and overlapping optimism | C05 G01 G04 G06 G07 H05 | Lost response, duplicated attempt, older failure after newer success, external side-effect replay |
| P0 | Cross-version compatibility and recoverable drafts | T01 T03 T04 T05 M10 Q05 | Old/new browser-worker-origin-data pairings and interrupted migration |
| P0 | Browser editing and activation semantics | L06 M01 N06 Q03 | Real IME/draft lifecycle and delayed activation-gated first interaction |
| P0 | Honest foreign trust and host confinement | F03 W01 W05 S07 | Malicious or contract-violating adapter, direct wire request, actual import/capability admission |
| P0 | Whole-call-tree budgets and invariant-preserving replication | K01 K03 H02 H06 I06 I07 | Retry/fanout saturation and negative-read/partition counterexamples |
| P0 contract, P1 implementation | Data use, retention, external writers, residency | E04 E08 E09 X05 | Managed data lineage and explicit unmanaged boundary |
| P1 | Correct measurement and evidence attribution | U01 U02 U04 U05 U10 | Correct LoAF probe, positive control, pinned remeasurement; independent oracles |
| P1 | Security adapters and browser security policy | D06 D09 F01–F12 | Threat-driven native/direct request, session, egress, decoder, and header tests |
| P1 | Native forms, navigation, accessibility | M01–M10 N01–N10 | Keyboard/screen-reader/zoom/IME/mobile task completion |
| P1 | Cache, outage recovery, and restore behavior | J01–J10 X01–X04 | Stampede/queue limits, restored backup, independent control-plane recovery |
| P1 | Minimal safe deployment/build chain | S01–S10 T01–T09 | Retained inputs/artifacts, provenance, actual host admission, rollout/rollback witnesses |
| P1 | Domain time, money, text basics | O01–O08 | Unicode, quote/rounding, zone ambiguity, locale message fixtures |
| P2 | Rich media, multi-tab, complex offline collaboration | R01–R09 Q06 Q08 I09 | Protocol-specific devices, collaborative undo/move, retirement/resync studies |
| P2 | Broad ecosystem migration and advanced tooling | W01–W05 V01–V08 U08 U09 | Maps/editor/chart/media adapters, version-aware HMR, causal replay, experiments |
| P2 | Advanced semantics-preserving optimization | P01–P09 H07 K10 T08 | Equivalent plans with measured full-cost tradeoffs |
| P3 | General verification and browser-native experiments | Optional donor candidates; E13 | Demonstrated benefit over existing-browser implementation before standards proposals |

### Execution order

1. Preserve and finish the current E9/E10 sequence. This is the first implementation dependency, not an invitation to parallel rewrites. [R3]
2. Review the P0 contracts with minimal accepted examples and counterexamples while keeping the implementation milestone focused. Resolve what interfaces must carry before expanding them.
3. Add independent model/schedule witnesses, then run them against the generated path. Correct measurement tools before updating performance conclusions.
4. Extend E11 into a hostile full-path app: multi-principal, multi-version, multi-node, bounded load, partial failure, browser lifecycle, and uncertain commitments.
5. Migrate one meaningful Kiokun subsystem only when it demonstrates the single-authority benefit with equivalent security, accessibility, performance, recovery, and debugging behavior.
6. Expand adapters and tooling, then evaluate optional browser-native extensions. Existing browsers remain sufficient for correctness.

## Part 14. Residual uncertainty

### What was searched and inspected

The repository review covered the requested charter, status, next steps, milestones, semantics, architecture, risk register/queue, decisions, whitepaper, proof roadmap, and technology matrix, plus the optimistic-update ADR and layout-probe code. The charter and whitepaper were read broadly through their full ranges; long status/risk histories and most implementation code were sampled. The audit is pinned to the stated commit and does not include later branch work. [R1]–[R15]

External coverage includes checked primary outage/advisory accounts from Cloudflare, GitLab, GitHub, npm, Fastly, Meta, Slack, Next.js/Vercel, React, and PortSwigger; AWS/Stripe/PostgreSQL contracts; HTTP/HTML/Streams/IndexedDB/security/Unicode standards; browser lifecycle and performance guidance; selected framework documentation and two historical React browser issues; and selected language/distributed/testing/tooling research.

There are 76 external anchors, including incident accounts, standards, documentation, research abstracts, and engineering explanations. **This is broad taxonomy coverage, not an exhaustive search of all trackers or the entire literature.** A checked standard supports an invariant example, not a claim that a named production outage occurred. No percentage of “all web failures” is defensible.

### Coverage quality

| Area | This pass | What could change the architecture |
|---|---|---|
| Current Pleris intent and milestone constraints | Relatively strong document-level coverage; selected concrete code checks | Full checker/codegen/runtime audit may reveal additional unsound boundaries |
| Type/ABI evidence, LoAF probe | Strong for the stated narrow findings | Actual compiler/browser reruns could change current implementation status and measurement interpretation |
| Authorization/commit/compatibility design | Strong invariant rationale, incomplete proposed protocol verification | Model-checking cross-principal/version schedules could change context and manifest representation |
| Distributed/cache/overload | Several primary mechanisms and incidents; no live lab | Database adapter guarantees, topology, and resource accounting may require new host interfaces |
| Browser forms/lifecycle/accessibility | Standards and selected historical issues; no real-device/screen-reader testing | Native editing and activation behavior can change chunking, draft, and event architecture |
| Security/privacy/supply chain | Broad threat boundaries; limited exploit and implementation audit | IFC implicit flows, JS isolation, crypto/session protocols, and metadata leakage may alter trust boundaries |
| Framework history | Selected React/Vue/Angular/Svelte/Qwik evidence, existing donor matrix | Deeper Solid/Marko/Astro/Nuxt/Remix/Ember/Lit/htmx and abandoned-design studies remain open |
| Media/realtime | High-level protocol/lifetime coverage | WebRTC negotiation, codec/device quirks, stream congestion and collaborative editor semantics remain undersearched |
| Internationalization | Core text/time/direction contracts | ICU/message-format/plural rules, collations, calendars, and normalization/confusables need deeper normative work |
| AI code generation | One older study abstract plus invariant mapping | Controlled current-agent evaluations may change DX priorities, not remove the need for independent oracles |
| Formal PL and compiler verification | Selected donors and design analysis | Detailed papers on effects, IFC, session/refinement types, translation validation, and schema lenses remain underread |

### Explicitly not performed

No compiler/cargo, full benchmark, database fault, screen-reader, or production suite was rerun locally. A subsequent targeted instrumentation fix has 20 Node tests, 20 Python validator/countermodel tests, and a local-script Chromium smoke test recorded in `IMPLEMENTATION.md`; it is not an E7/E8 rerun. No current framework performance ranking was established. No systematic CVE database enumeration, complete Chromium/WebKit/Firefox tracker sweep, current npm dependency vulnerability scan, or full source-to-host security audit was performed. An access-denied WebKit issue and failed legacy/PDF links were not counted as evidence. Paper abstracts were not silently treated as complete formal proofs.

The census scripts check structure and small reference countermodels; the separately identified instrumentation tests exercise the targeted helper and harness startup cleanup. They do not turn a proposed rule into an implemented guarantee. Every corpus obligation remains open until an implementation witness is linked.

### Strong versus tentative conclusions

Strong: E9 remains a blocking prerequisite in the audited status; the LoAF lookup is at the wrong record level; source type validity and actual host security are different boundaries; transient browser activation cannot be assumed to survive arbitrary deferred work; equal arguments do not identify equal business intent; wire compatibility does not establish semantic compatibility. [R2] [R3] [R14] [S64] [S53] [S13] [S70]

Strong as a design counterexample, not a runtime bug report: unconstrained snapshot restoration is insufficient for overlapping optimistic intents. A valid implementation may instead serialize operations or preserve confirmed state through a tested overlay protocol. [R10]

Tentative: the best concrete representation for temporal authorization evidence, the smallest ergonomic integrity-label system, which compatibility facts belong in public syntax versus inferred metadata, and which replication/cost analyses can be inferred without unacceptable compile time. These require prototypes and accepted-program usability tests.

The additional pass most likely to change architecture is a **cross-boundary state-machine study**: authorization revision × data revision × command intent/attempt × client/worker/runtime version × browser lifecycle. Follow it with an explicit JS-interoperability threat model and real mobile editing/activation tests. Searching more independent rendering anecdotes is less likely to change the core than finding a counterexample at these intersections.

## Answer to the restart question

Starting Perfect Web again, I would keep the readable declarative application, effects, resources, native document, and one-way semantic projection. I would change the **order and precision of the contracts**:

- Define authority with principal, scope, provenance, and temporal validity before treating privacy categories as sufficient.
- Define commitment and reconciliation before making retries or optimistic UI automatic.
- Define compatibility across live versions before treating one generated schema as the whole distributed contract.
- Define editable/native/browser-owned state before freezing the renderer and lazy-handler protocol.
- Define actual foreign/host trust and composed resource budgets before promising global safety or efficiency.
- Make every safety/performance claim carry bounded evidence from the start.

These changes should make application code shorter conceptually, not more annotated. The compiler can infer and derive more only after the semantic boundaries distinguish the facts it must not conflate. That is the design work to resolve before additional APIs and generated machinery make the choices expensive.

## Source links

[R1]: SOURCES.md#r1
[R2]: SOURCES.md#r2
[R3]: SOURCES.md#r3
[R10]: SOURCES.md#r10
[R11]: SOURCES.md#r11
[R13]: SOURCES.md#r13
[R14]: SOURCES.md#r14
[R15]: SOURCES.md#r15
[S02]: SOURCES.md#s02
[S03]: SOURCES.md#s03
[S06]: SOURCES.md#s06
[S09]: SOURCES.md#s09
[S11]: SOURCES.md#s11
[S12]: SOURCES.md#s12
[S13]: SOURCES.md#s13
[S14]: SOURCES.md#s14
[S15]: SOURCES.md#s15
[S17]: SOURCES.md#s17
[S18]: SOURCES.md#s18
[S19]: SOURCES.md#s19
[S20]: SOURCES.md#s20
[S21]: SOURCES.md#s21
[S22]: SOURCES.md#s22
[S23]: SOURCES.md#s23
[S24]: SOURCES.md#s24
[S25]: SOURCES.md#s25
[S26]: SOURCES.md#s26
[S27]: SOURCES.md#s27
[S28]: SOURCES.md#s28
[S29]: SOURCES.md#s29
[S30]: SOURCES.md#s30
[S31]: SOURCES.md#s31
[S32]: SOURCES.md#s32
[S33]: SOURCES.md#s33
[S35]: SOURCES.md#s35
[S36]: SOURCES.md#s36
[S37]: SOURCES.md#s37
[S38]: SOURCES.md#s38
[S39]: SOURCES.md#s39
[S40]: SOURCES.md#s40
[S43]: SOURCES.md#s43
[S52]: SOURCES.md#s52
[S53]: SOURCES.md#s53
[S60]: SOURCES.md#s60
[S61]: SOURCES.md#s61
[S62]: SOURCES.md#s62
[S63]: SOURCES.md#s63
[S64]: SOURCES.md#s64
[S65]: SOURCES.md#s65
[S66]: SOURCES.md#s66
[S67]: SOURCES.md#s67
[S68]: SOURCES.md#s68
[S69]: SOURCES.md#s69
[S70]: SOURCES.md#s70
[S71]: SOURCES.md#s71
[S72]: SOURCES.md#s72
[S74]: SOURCES.md#s74
[S75]: SOURCES.md#s75
[S76]: SOURCES.md#s76
