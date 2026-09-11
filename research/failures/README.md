# Pleris Web Failure Census

Research edition 1, 2026-09-10. Audited commit: `0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45` in `Kimeiga/perfect-web`.

**Recommendation:** retain Pleris's readable, single-semantic-application thesis. Before freezing more interfaces, make authority over time, commit outcomes, compatibility across versions, and unmanaged browser/foreign behavior precise. Finish the existing E9 type-checking repair before treating any new projection as a trusted end-to-end implementation.

The census is a research and acceptance-requirements addition, not a claim of production readiness. Targeted tooling fixes and their test evidence are recorded separately in `IMPLEMENTATION.md`. The census contains **224 invariant-level failures in 24 families**, supported by **76 external evidence anchors** and **15 commit-pinned repository anchors**. Several sources are standards or documented examples, not historical incidents. Counts describe this inventory, not the fraction of all possible failures found.

## Reading map

| Requested part | Location |
|---|---|
| 1. Executive gap analysis | This file |
| 2. Hierarchical taxonomy | `CENSUS.md` and its linked taxonomy chapters |
| 3. Full failure table | Chapters indexed by `CENSUS.md` |
| 4. Duplication census | This file |
| 5. Should-be-impossible corpus | `OBLIGATIONS.md`, CF obligations |
| 6. Runtime guarantees | `OBLIGATIONS.md`, RT obligations |
| 7. Explicit domain policy | `OBLIGATIONS.md`, policy surface |
| 8. Developer experience | `REQUIREMENTS.md` |
| 9. Performance | `REQUIREMENTS.md` |
| 10. Security and privacy | `REQUIREMENTS.md` |
| 11. Missing prior art | `REQUIREMENTS.md` |
| 12. Recommended repository changes | `REQUIREMENTS.md` |
| 13. Priority and execution order | `REQUIREMENTS.md` |
| 14. Residual uncertainty | `REQUIREMENTS.md` |

`SOURCES.md` states exactly what was learned from each source. `census.json` is an optional generated convenience export of the taxonomy chapters indexed by `CENSUS.md`, never another editable authority and not required in version control. `tools/validate.py` checks structural consistency and exports that projection. It does **not** verify the truth of historical claims, execute Pleris, or certify the proposed guarantees. `tools/test_models.py` contains small independent counterexamples, not tests of the actual Pleris runtime.

## Method and limits

A leaf names a violated invariant, not a framework-specific syntax. A repeated effect in React, a wrongly observed value in Vue, and an omitted compiler dependency are only separate leaves when their underlying obligations differ. Performance, security, AI-code generation, mobile, and interop are cross-cutting views of the same leaves, not separate piles of duplicate bugs.

The analysis ladder is A construction, B static checks, C compiler derivation, D generated runtime, E host/deployment, F browser, G deterministic tests/model checking, H explicit domain policy, I outside available knowledge. Prefer the earliest reliable enforcement boundary. A static rule can prove that a required guard exists; only the runtime authority can decide a live authorization or database race. Tests can establish witnesses and find counterexamples, not prove an arbitrary implementation universal.

A failure enters implementation work through this chain:

> checked evidence or labeled counterexample → exact invariant and assumptions → available semantic information → proposed enforcement boundary → legitimate accepted neighbor → adversarial witness → measured implementation claim.

Use stable leaf IDs in implementation tests, ADRs, risks, and benchmark reports. Do not replace an ID because a framework changes syntax. Splitting a leaf requires explaining the newly distinct invariant. Merging leaves preserves aliases in the history. A mention in a charter is not an implementation, and a successful narrow harness is not an end-to-end proof.

## Part 1. Executive gap analysis

### Existing design to preserve

The inspected charter and whitepaper already connect concepts that ordinary application code often wires independently: effects, privacy, placement, resources, commands, structured lifetime, incremental materialization, serialization, native HTML, and inspectable deployment. They already reject universal CRDTs and acknowledge offline policy, foreign boundaries, resumption compatibility, typed change events, and causal debugging. These are **not newly discovered missing areas**. [R1] [R11] [R12]

Specific strengths are the distinction between a query and a command; resource identity rather than render allocation identity; owner/subscriber separation; explicit privacy partitions; one semantic type source for outward ABI projection; separate operation identity and capability grants; scoped document-part identities; and recorded distinctions between a version noticed and a version actually held. The repository's own false-green history also already insists on causal diagnostics and valid test prerequisites. [R2] [R3] [R4] [R8]

The scoped E7 browser and E8 host work are valuable evidence. The milestone record reports browser assertions across Chromium, Firefox, and WebKit, and separate host-admission work. Those records do not establish real-device iOS behavior, all accessibility requirements, a distributed production runtime, or compiled Pleris execution through the complete host path. The E7/E8 gates were not rerun in this session; the separate browser instrumentation smoke test does not close those gates. [R4]

### Highest-risk findings

**1. The type-checker repair remains foundational, not optional cleanup.** The current status reopens E9 because nominal representation existed before ordinary call arguments and declared results were comprehensively checked. NEXT records the required dependency order and the unsupported callable-generic test syntax. Preserve that order: resolved semantic types → full argument/return checks → derived contracts → component ABI → compiled component execution → deletion of the alternate Rust command implementation. Do not use this census to skip it. A01, A02, A08, S07, S10. [R2] [R3]

**2. Authority needs temporal and contextual semantics.** A `User` privacy category, a `database.write` effect, a signed credential, and a fresh authorization decision are different facts. The design needs a precise contract for principal/tenant context, acting versus effective identity, policy revisions, revocation, and the relationship between the authorization snapshot and committed data. A cached or resumed value cannot renew its own authority. D03–D10, E02, Q02, Q07. This extends existing capabilities/privacy rather than replacing them. Zanzibar contributes the causal-policy lesson; macaroons contribute attenuation. [R1] [R11] [S18] [S19]

**3. Compatibility is a relation among live versions, not one current schema.** E7 already checks a scoped resumption boundary. Production also admits old tabs, workers, edge code, origins, stored values, queued commands, database migrations, and workflow histories. A valid rollout must describe which combinations are allowed and what happens to an excluded client with an unsaved draft. Compatibility includes meaning, not just decodability. T01–T06, Q05, M10. [R4] [R11] [S49] [S70] [S37]

**4. Optimistic restoration needs an overlapping-intent protocol.** ADR25 correctly removes developer-written inverse rollback and makes the optimistic transform pure. Its restore-the-before-state language is insufficient if multiple intents may overlap on one entry. A later successful update must survive an earlier failed update. G07 is a design counterexample, **not a reproduced bug in the current runtime**. Serialization of conflicting intents or an ordered overlay protocol can satisfy it; arbitrary noncommutative business merges still need policy. [R10] [S40]

**5. Effects need commitment and budget contracts, not just names.** Cancellation is not rollback; a lost response is not a rejected command; equal arguments are not always duplicate intent. Local singleflight does not bound global fanout, retry multiplication, reconnect work, or financial cost. Keep intent, attempt, resource, delivery, and workflow identities distinct. G01–G06, C05, K01–K05. [R1] [R11] [S13] [S14] [S33] [S34]

**6. The browser is an independent participant.** Native editing, IME, autofill, focus, history, bfcache, activation, translation, storage eviction, and mobile viewport behavior cannot be reduced to application state assignments. In particular, interaction-lazy loading can outlive transient activation. A replayed event cannot manufacture a trusted browser gesture. L06, M01, N06, Q01–Q04. This can affect the renderer and chunk planner, so it is a P0 contract question even before all browser adapters are implemented. [S43] [S48] [S53] [S46]

**7. Foreign boundaries require real confinement and honest contracts.** Decoding proves a value's shape, not its trusted origin, authorization, cleanup behavior, or absence of hidden effects. An arbitrary npm package in an ordinary page realm is not confined by a Pleris manifest. SES demonstrates useful authority restriction but explicitly does not isolate shared-agent exhaustion. The platform must distinguish safe data interop, audited privileged adapters, and genuinely isolated untrusted code. E06, W01, W05. [S20] [S29] [S58]

### Concrete research correction: layout instrumentation

The inspected thrashing probe reads `e.forcedStyleAndLayoutDuration` from a long-animation-frame entry. The phased probe search result shows the same lookup. The specification places that attribute on `PerformanceScriptTiming`, reached through the frame's `scripts` attribution records. Therefore an undefined frame-level property **does not establish that the browser lacks the measurement**. U01. [R14] [R15] [S64]

The targeted follow-up corrects the probe, distinguishes missing attribution from observed zero, and runs a known-thrashing positive control in installed Chromium. The original pinned environments still need remeasurement; see `IMPLEMENTATION.md`. Then annotate all summaries that inferred unavailability, including the E0 notes, risk register, known limitations, and evidence ledger. Preserve old raw observations as historical data. This finding does **not** supply replacement benchmark numbers, prove support in the earlier browser build, or invalidate unrelated wall-clock measurements. [R5] [R7] [S64]

### Underdeveloped areas, rather than entirely absent ambitions

Precise contracts are missing or thin for integrity/provenance distinct from confidentiality; purpose/consent and derived-copy retention; form drafts distinct from valid domain values; localization/message contracts and civil-time policy; actor-aware collaborative undo; ephemeral presence versus committed state; negative-read dependency tracking; whole-application resource budgets; external writers; and recovery control-plane independence. The census marks individual leaves rather than declaring all of security, offline support, forms, or deployment absent. [R1] [R11]

### Adequately directed but insufficiently tested

The resource, privacy, native-DOM, resumption, host-import, and ABI plans have useful scoped witnesses. Their composition still needs adversarial scenarios: account switch plus pending command; permission revocation plus cached result; old worker plus new schema; failed earlier optimism plus later success; canceled subscriber plus shared refresh; interrupted outbox delivery; externally mutated DOM plus fragment update. Passing each subsystem alone does not establish these combinations. [R4] [R8] [R11]

### Complexity to resist

Do not import every advanced type system into application syntax. Prefer local inference, ADTs, selective nominal IDs, small resource typestates, and explicit adapter contracts. General dependent types, unrestricted refinement solving, global information-flow inference through arbitrary JavaScript, and whole-program cost proofs would need evidence of benefit before increasing the language surface.

Do not encode product choices as optimizer guesses. Do not make all state a CRDT, all lists virtualized, all code Wasm, all placement automatic without an inspectable plan, or all foreign libraries supposedly safe after shape decoding. Do not treat one changed input as a promise of one DOM write: an input may legitimately affect many parts. Do not equate no generated layout thrashing with no browser layout work. [R1] [R11] [S35] [S60]

A shared semantic representation should derive deployment artifacts, but **not every independent oracle**. Tests of intended business behavior, independent wire validators, and runtime host enforcement are not wasteful duplicate authorities. They challenge the claim that the projections implement the authority correctly. [R3] [R8]

## Part 4. Duplication census

These are derivation opportunities and distinctions, not claims that every stack repeats every fact. Single authority is qualified by semantic identity, scope, and version. Several legitimate facts may look alike. The compiler can audit declared authority/projection relationships; it cannot generally discover that two arbitrary programs mean the same thing.

| Semantic fact | Where it may be repeated today | Disagreement failure | Single-authority treatment | Proposed authority |
|---|---|---|---|---|
| Domain identifier identity | Frontend/backend types, route params, database wrappers | Store ID accepted as cart ID | Yes; preserve identity through every projection | Resolved nominal domain declaration |
| Value shape | TS interface, JSON schema, API DTO, decoder | Accepted payload differs across tiers | Yes within one contract; external schema may be authoritative instead | Declared owned type or versioned imported schema |
| Refinement predicate | Client checks, server validators, database constraints | Browser accepts what commit rejects unexpectedly | Derive equivalent checks where backend can represent them; runtime-only checks remain separate | Domain constructor/predicate |
| Required versus optional value | Form, wire schema, database default | Missing becomes a misleading empty/default value | Yes, with explicitly distinct draft and persisted states | Domain and boundary presence contract |
| Form draft representation | Input state, errors, pending display | Temporary invalid text lost | Derive where possible; draft is not identical to valid value | Typed form/draft adapter over domain input |
| Operation signature | Server route, client wrapper, RPC, WIT, docs | Different arguments/returns or ABI meanings | Yes; import authority can reverse at foreign boundary | Resolved operation declaration |
| Operation identity | Route name, capability string, telemetry name | Wrong implementation bound to a valid permission | Keep operation identity distinct from capability; derive names | Stable semantic operation identity |
| Capability requirement | Code checks, manifest, host config | Deployed component receives excess power | Derive requested set; host grant is an independent authority | Inferred effects plus explicit deployment grants |
| Permission rule | UI visibility, endpoint middleware, resolver, SQL | Hidden control but callable unauthorized action | Derive advisory affordances and mandatory gates; live decision stays authoritative | Domain authorization policy and current authority state |
| Principal/tenant scope | Session context, cache key, SQL predicate, trace context | Cross-user data/command leakage | Yes for managed scope; verify at host boundary | Authenticated scoped execution context |
| Confidentiality label | Serializers, logs, cache config, client/server directives | Secret exposed by a forgotten sink | Derive propagation and admissible sinks | Value information-flow policy |
| Integrity/provenance | Caller assertions, decoded types, business checks | Client price accepted as authoritative price | Keep separate from shape and secrecy | Verified source/endorsement authority |
| Release of private data | Analytics transform, export API, logs | Implicit unauthorized declassification | One explicit release policy per purpose, not repeated sanitizers | Capability-gated release operation |
| Consent/purpose | SDK initialization, dispatch filters, retention settings | Export after purpose or consent ceases | Managed derivations possible; user/legal policy not inferable | Versioned collection/use policy |
| Retention/deletion | DB cleanup, caches, search, backups, logs | Derived copies survive promised deletion | Derive managed lineage jobs; state external limits | Data lifecycle policy and lineage |
| Residency | Database region, edge config, logs, backups | Secondary copy violates location policy | Yes for managed resources; runtime attestations still needed | Deployment data-location constraints |
| Resource identity | Hook key, API URL, cache tag, invalidation key | Missing dependency or accidental aliasing | Yes for known semantic dependencies | Resource declaration plus bound inputs/audience/generation |
| Freshness allowance | CDN TTL, server cache, browser store, worker | Layers serve data under conflicting ages | Derive each cache projection; age is not authorization validity | Resource freshness policy |
| Change dependency | Query code, invalidation lists, materialized view | Insert into previously empty query never invalidates | Derive observable dependencies; retain external event contracts | Query graph plus authoritative change domains |
| Intent identity | Form state, retry wrapper, payment key, trace ID | Duplicate commitment or collapsed separate purchase | Yes; retain attempts and provider identifiers separately | Stable interaction/command journal identity |
| Replay safety | Client retry, proxy retry, job retry | Multiplicative attempts or duplicate effects | Derive allowable retries from command and adapter contract | Commitment/replay contract |
| Deadline | UI timeout, fetch timeout, service timeout | Work survives the user's budget | Derive remaining budgets; workflow deadline is a distinct fact | Causal operation deadline |
| Optimistic projection | UI mutation and handwritten rollback | Inverse erases another update | Derive intent overlay lifecycle, not arbitrary business merge | Pure optimistic transform plus conflict policy |
| State transition | UI switches, command guards, database update | Impossible status or race across transition | Derive dispatch and static cases; runtime state/commit checks remain | Domain state machine and authoritative revision |
| Commit-linked notification | Mutation code, event publisher, invalidation job | Commit without notification or vice versa | Yes within a local transactional boundary | Transition's durable outbox obligation |
| Serialization | Frontend codec, backend codec, WIT adapter | Same type maps differently | Derive from boundary contract; independently validate projection | Semantic type and versioned boundary representation |
| URL/route codec | Link builder, router, server parser, docs | Double decoding or wrong resource identity | Yes | Typed route definition |
| Accessible control relationships | IDs, labels, errors, ARIA state | Incorrect tree or unusable error association | Derive structure; semantic label quality remains authored | Native control/form declaration |
| Stable UI entity identity | List key, component store, resumable part ID | State moves to another entity | Derive projections, preserving instance versus entity distinctions | Domain key and renderer instance identity |
| Message interpolation | Source text, catalogs, formatters | Missing placeholder or locale case | Derive checks; translations remain independent authored content | Typed message signature |
| Monetary arithmetic | UI total, checkout service, provider amount | Charged amount differs from agreed amount | Derive views from server quote; tax/provider rules may be separate authorities | Versioned authoritative quote and currency policy |
| Compatibility | API version, worker version, database migration, chunk manifest | Live combinations cannot interact safely | Derive compatibility graph and rollout checks | Versioned semantic contracts with supported-history policy |
| Build inputs | Lockfile, CI env, local shell, artifact labels | Irreproducible or poisoned outputs | Derive provenance from actual admitted input closure | Hermetic build plan |
| Instrumentation identity | API logs, frontend events, trace spans | Lost causal chain and duplicate logical counts | Derive technical identities, not product metric definitions | Semantic operation/intent graph |
| Experiment assignment | Browser flag, server flag, analytics exposure | Users see one variant while measured as another | Derive delivery from one assignment authority | Experiment unit, assignment, eligibility, and exposure policy |
| Deployment placement | Source directives, infra files, routing/CDN rules | Private or strongly consistent work placed incorrectly | Derive only admissible plans; product constraints remain explicit | Semantic constraints plus approved deployment plan |
| Security policy | CSP, script imports, Permissions Policy, embed allowlists | Policy either breaks app or permits too much | Derive supported enforcement from declared integrations and artifact | Approved execution/resource/embedding contract |
| Test oracle | Generated tests, expected snapshots, business assertions | Same wrong premise passes both generator and test | **Not one authority for all tests** | Independent domain examples and standards validators |
| Documentation status | Charter, status, milestone summaries, research notes | Planned feature presented as proven | Derive current summaries from scoped evidence, preserve history | Claim/evidence ledger with revision and assumptions |

## Rule-design and developer-cost review

No row in the census authorizes adding syntax by itself. Each proposed feature must pass a readable accepted example and a diagnostic repair exercise. The following profiles are the minimum review for every associated leaf; combinations inherit both reviews.

| Profile | Soundness, information available, false-positive risk | Inference and authored surface | Diagnostics and incremental cost | Runtime and code-size cost | Browser/interop impact and escape hatch |
|---|---|---|---|---|---|
| T: types and values | Sound within resolved operations and validated boundaries; arbitrary predicates or mutable foreign aliases need runtime proof. Excessive nominalization forbids useful structural records. | Infer local types; declare domain distinctions and public/imported contracts. Restrict refinement solving to a bounded decidable fragment. | Show expected/actual semantic identities and provenance, not flattened ABI types. Cache resolved signatures; measure recursive/generic worst cases. | Boundary decoders and checked conversions add work/bytes. Specialize and share them without erasing meaning. | Emit ordinary JS/Wasm/HTTP representations. Low-level conversions require checked or explicitly unsafe adapters. |
| E: effects/authority/privacy | Sound only for tracked calls/control flow and honest adapters. Unknown must stay unknown; implicit flow and delegation require a stated model. | Infer local propagation; annotate public authority, declassification, and unobservable foreign behavior. Do not restate placement everywhere. | Show the shortest causal chain. Bound row/constraint inference and separate body changes from interface invalidation. | Static checks can erase; host grants, sink guards, and dynamic principals still cost. Measure metadata growth. | Ordinary same-realm JS can bypass wrappers. Grant narrow host interfaces or isolate code; unsafe effects must remain visible. |
| R: runtime protocols | Static shape proves protocol admission, not live order, commit, or release after death. Tests must explore schedule/fault combinations. | Infer owner, subscriber, intent, and dependency plumbing. Expose only policy differences the system cannot choose. | Explain why work ran, who owns it, and why a completion was dropped. Compile protocol templates once per needed feature. | Journals, generation checks, queues, overlays, and retries consume memory/CPU/storage. Enforce budgets and measure cardinality. | Adapter cancellation can be advisory. Model loss, failure, and unknown outcomes; durable work needs a real host service. |
| U: UI/native document | Structural checks are useful but cannot prove semantic label quality or all assistive-technology behavior. Native/browser state is not fully compiler-owned. | Prefer native elements and derived relationships. Explicit keys/focus/announcement policies only when multiple meanings are legitimate. | Report affected control/part and user impact. Incrementally check affected template and style dependencies. | Resumption, event plumbing, focus scopes, and compatibility shims cost bytes. Do not ship unsupported or unused widgets. | Test actual target browsers, mobile devices, IMEs, and screen readers. Audited imperative islands preserve ownership and native agency. |
| X: foreign/host boundary | Guarantees depend on actual host isolation, adapter contracts, and live service behavior. Rejecting every unknown library would prevent adoption. | One audited adapter per integration, with decoded values and narrowly relevant behavior; no fabricated purity from TS declarations. | Explain unmet host capability/contract and responsible adapter. Cache adapter interface checks; do not repeatedly whole-analyze opaque code. | Sandboxes, codecs, cross-realm messages, signatures, and resource limits have real cost. Measure by integration. | Support trusted same-realm mode with explicitly weaker guarantees, and isolated modes where feasible. No silent fallback between them. |
| D: domain policy | Compiler lacks the product's intent. A universal choice can be incorrect even if mechanically safe. | Require the smallest relevant choice: conflict, freshness, offline, expiry, rounding, recovery, purpose, or budget. Infer mechanisms afterward. | Explain incompatible requirements and concrete outcomes, not abstract solver jargon. Policy checking must be bounded. | Chosen consistency or durability may require coordination and storage. Show the cost consequence before deployment. | Some policies are impossible on a chosen browser/host; reject the plan or request an explicit weaker policy, never silently weaken. |
| P: optimization | Only optimize among semantically equivalent, policy-admissible plans. Profiles estimate cost, not correctness. | No routine memoization, dependency arrays, manual chunk boundaries, or placement directives. Optional explainable constraints for requirements. | Report why a plan changed and which measured bottleneck motivated it. Incremental planning keys include target/profile/config. | Track HTML/CSS/JS/Wasm, parsing, CPU, DOM, memory, network, origin work, and cost. One faster metric can hide another regression. | Browser engines differ. Retain a correct baseline and target-specific optional plans; avoid a universal performance promise. |
| V: evidence/tooling | Tests prove only their observed scope; generated oracles can agree on a bug. Missing observations cannot become passes. | No production annotations merely to satisfy test tooling. Fixtures can state assumptions and independent expected outcomes. | Pin diagnostics, source spans, prerequisites, revision, environment, and witness ownership. Measure edit invalidation separately from full builds. | Testing and trace overhead must not silently enter production bundles. Sample only when the measurement contract allows it. | Cross-browser automation is not real-device or assistive-tech proof. Keep unsupported measurements and untested adapters explicit. |

### Legitimate programs that must remain expressible

Live search may intentionally run on each committed input; a second purchase with identical fields is a new intent; a draft may be invalid while editing; a feedback loop may be a deliberate animation; public aggregate release may be explicitly authorized; an editor may own an imperative island; an offline conflict may require a human; a game may require JavaScript; a trusted library may need DOM access; a migration may intentionally reset incompatible development state. Every static restriction must include an accepted neighbor for such cases rather than treating them as unsafe by default.

## Conclusion

Starting again today, the change would be **a smaller explicit semantic core with sharper boundaries**, not a larger annotation language: resolved value meaning; effects and delegated authority; owned work; versioned state and compatibility; commitment outcomes; browser-owned interaction; and explicit product policy. Derive the machinery from those facts, then make the evidence for each guarantee as precise as the guarantee itself.

## Source links

[R1]: SOURCES.md#r1
[R2]: SOURCES.md#r2
[R3]: SOURCES.md#r3
[R4]: SOURCES.md#r4
[R5]: SOURCES.md#r5
[R7]: SOURCES.md#r7
[R8]: SOURCES.md#r8
[R10]: SOURCES.md#r10
[R11]: SOURCES.md#r11
[R12]: SOURCES.md#r12
[R14]: SOURCES.md#r14
[R15]: SOURCES.md#r15
[S13]: SOURCES.md#s13
[S14]: SOURCES.md#s14
[S18]: SOURCES.md#s18
[S19]: SOURCES.md#s19
[S20]: SOURCES.md#s20
[S29]: SOURCES.md#s29
[S33]: SOURCES.md#s33
[S34]: SOURCES.md#s34
[S35]: SOURCES.md#s35
[S37]: SOURCES.md#s37
[S40]: SOURCES.md#s40
[S43]: SOURCES.md#s43
[S46]: SOURCES.md#s46
[S48]: SOURCES.md#s48
[S49]: SOURCES.md#s49
[S53]: SOURCES.md#s53
[S58]: SOURCES.md#s58
[S60]: SOURCES.md#s60
[S64]: SOURCES.md#s64
[S70]: SOURCES.md#s70
