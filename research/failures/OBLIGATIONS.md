# Acceptance obligations, not implemented guarantees

This file supplies Parts 5–7. Every entry is **planned** until a linked implementation witness exists. It does not add accepted language syntax or claim new diagnostics already work. The leaf IDs resolve to `CENSUS.md`, which owns failure classification and current status.

## Witness contract

For each compile-fail fixture record: leaf ID; required language/adapter features; minimal rejected program or plan; intended diagnostic category and source span; accepted neighbor; mutation that removes the guard; expected failure stage; and audited compiler revision. A parser failure cannot prove an effect or type invariant. In particular, do not implement the currently unsupported callable-generic E9 example and report its parse error as type-checking success. [R3] [R8]

For each runtime fixture record: initial state; actors/principals; versions and policy; event schedule; fault injection; allowed outcomes; forbidden observation; independent oracle; source/runtime/host revisions; browser/device/storage configuration; and evidence artifact. A timeout alone is not the expected domain outcome. A passing reference model is not a passing generated implementation.

**S** below means source-level rejection. **P** means build/deployment-plan rejection. Some requirements have both a static admission check and a runtime obligation. All rows remain conditional on the stated supported semantic fragment and adapter contracts.

## Part 5. Should-be-impossible corpus

| ID | Lane | Rejection invariant | Accepted neighbor / essential qualification | Census leaves |
|---|---|---|---|---|
| CF01 | S | A call supplies one nominal domain identity where another is required. | Same identity through aliases and nested supported carriers. | A01 |
| CF02 | S | A reachable return has a type incompatible with the declared result. | Every return path inhabits the declared result. | A02 |
| CF03 | S | A closed ADT consumer omits a reachable alternative without propagation. | Exhaustive handling, or an explicit open-protocol result. | A03 |
| CF04 | S | Unvalidated external data is used as a refined domain value. | Checked constructor/decoder establishes the predicate. | A05 |
| CF05 | S | An implicit conversion loses identity, numeric precision, or required units. | Explicit checked conversion with modeled failure/rounding. | A06 O07 |
| CF06 | S | A resource known consumed/closed is reused. | Ownership transfer or an explicitly shared idempotent interface. | A09 C01 |
| CF07 | S | A blocked type/effect result is treated as evidence of validity or purity. | Editor can show partial analysis; deployment cannot. | A08 V04 |
| CF08 | S | Representation lowering conflates distinct semantic alternatives. | Distinct tags or an independently validated equivalent representation. | A04 S10 |
| CF09 | S | A domain ID is implicitly reconstructed from localized display text. | A validated stable identifier or explicit domain parser. | A10 O04 |
| CF10 | S | A mutable foreign alias is treated as an immutable validated value without a boundary contract. | Copy, immutable representation, or audited ownership transfer. | A07 W03 |
| CF11 | S | A pure render computation performs a command or other non-render effect. | Read an owned resource or invoke a command from an authorized event. | B01 B07 |
| CF12 | S | A public materialization depends on private input without authorized release. | Explicit bounded declassification establishes a public result. | E01 E05 |
| CF13 | S | A browser-placed computation requires an origin-only database or secret capability. | Typed remote operation whose host checks authority. | S07 D01 |
| CF14 | S | A privileged effect lacks an admitted capability path. | Explicitly supplied attenuated capability. | D07 D08 |
| CF15 | S | A handler captures a nonserializable server resource. | Serializable identifiers plus a separately authorized server operation. | L04 |
| CF16 | S | A resumable capture serializes a value into a broader audience than permitted. | Audience-compatible capture or explicit release. | E01 L04 |
| CF17 | S | A sensitive value reaches a log/analytics/error sink lacking its disclosure authority. | Approved redaction or purpose-limited sink. | E03 E04 |
| CF18 | S | Confidentiality labels are silently weakened by an ordinary conversion. | Capability-gated release with recorded source provenance. | E05 |
| CF19 | S | Decoding an untrusted claim silently endorses its business authority. | Trusted source verification or authoritative recomputation. | E06 |
| CF20 | S | A delegated capability broadens the issuer's declared grant. | Equal or narrower contextual authority. | D07 |
| CF21 | S | Speculative work requires a non-speculatable effect. | Pure computation or an explicitly admissible read with budget. | B07 K10 |
| CF22 | S | A known pure dependency cycle has no supported fixed-point/delay semantics. | Explicit bounded feedback or supported monotone fragment. | B05 |
| CF23 | S | An unresolved foreign call is assumed pure because its TS signature omits effects. | Audited adapter contract or explicitly unsafe boundary. | B03 W01 |
| CF24 | S | A scope-owned task escapes without durable or detached authority. | Ownership transfer or explicitly best-effort host-scoped task. | C06 |
| CF25 | S | A resource requiring release is dropped without an inferred owner or transfer. | Compiler-generated scope cleanup. | C01 |
| CF26 | S | A subscriber is granted unilateral abort authority over shared work it does not own. | Cancel its own subscription, or invoke the shared owner's policy. | C04 |
| CF27 | S | A phase-restricted resource is used across a forbidden suspension. | Complete the active operation or reopen under a supported adapter. | C08 Q08 |
| CF28 | S | A descendant silently replaces the causal deadline with a larger independent budget. | Explicit new durable workflow authority. | C07 |
| CF29 | S | Pure planning or a read phase executes an owned DOM mutation. | Mutation in the generated mutation phase. | P01 |
| CF30 | S | An owned synchronous layout read follows an owned layout write in the same prohibited batch. | Next admissible measurement phase or audited imperative boundary. | P01 P02 |
| CF31 | S | Retry is enabled for a non-idempotent effect without a stable intent and supported commitment contract. | No automatic retry, or a proven adapter/journal path. | G01 G04 |
| CF32 | S | Automatic retry constructs a new semantic interaction identity per attempt. | Attempts carry distinct trace IDs but retain intent identity. | G01 G02 |
| CF33 | S | A retriable transaction directly performs a non-replay-safe external commitment. | Transactional outbox or supported durable activity protocol. | H05 G08 |
| CF34 | S | A transport error is exhaustively handled as rejection when commit-unknown is a possible result. | Explicit unknown/reconciliation branch. | G06 |
| CF35 | S | Cancellation is used as proof that a remote command was rolled back. | Preserve the unresolved intent and reconcile the outcome. | C05 G06 |
| CF36 | S | An optimistic transform performs effects or changes a differently typed resource entry. | Pure transform of the selected entry, with runtime concurrency protocol. | G07 B07 |
| CF37 | S | A handwritten inverse claims universal rollback of shared optimistic state. | Runtime-managed intent removal under the chosen conflict policy. | G07 |
| CF38 | S | A generated public command has no authorization policy or declared public authority. | Public operations remain legitimate, explicitly scoped operations. | D01 D02 |
| CF39 | S | An operation combines tenant-indexed references from incompatible known scopes. | An explicit cross-tenant administrative capability. | D03 |
| CF40 | S | Browser/session-local state is declared the authority for a required global uniqueness or durable commitment. | Authoritative host state, or a weaker explicitly local policy. | H03 K06 |
| CF41 | S | Untrusted text is passed to an executable HTML/script sink as ordinary text data. | Safe text output or audited rich-content adapter. | F01 |
| CF42 | S | A value safe for one output grammar is reused as proof of safety for another. | Context-specific URL/CSS/HTML validation. | F02 |
| CF43 | S | A generated decoder permits arbitrary constructor/module execution from a wire tag. | Closed versioned data decoder; host dispatch remains separately authorized. | F03 |
| CF44 | S | Data is concatenated into a structured SQL/process operation's executable syntax. | Parameterized data or an explicit constrained identifier vocabulary. | F06 |
| CF45 | S | A component requests raw unrestricted egress while claiming a destination-restricted effect. | Restricted host client or explicit broader audited authority. | F07 K09 |
| CF46 | S | Ordinary decoded data constructs an unforgeable host capability. | Authenticated host-issued handle with scope/lifetime checks. | F12 D08 |
| CF47 | P | A cache plan reuses private output across incompatible bound audiences. | Same audience and admissible policy/version. | E02 J01 |
| CF48 | P | A freshness-only policy is used to claim authorization validity. | Separate current or bounded-staleness authorization contract. | D05 J02 |
| CF49 | P | A deployment claims required isolation/consistency unsupported by its adapter. | Explicitly weaker domain policy or a supporting host. | H10 I07 |
| CF50 | P | Offline globally authoritative writes are claimed under a partition policy that cannot support the invariant. | Queue, reject, merge where valid, or explicit arbitration. | I06 I07 Q07 |
| CF51 | P | A supported old/new producer-consumer combination is incompatible. | Compatible rollout phases or negotiated retirement/recovery. | T03 |
| CF52 | P | A rollback plan selects code that cannot interpret already committed state. | Verified forward-compatible reader or recovery migration. | T04 |
| CF53 | P | A retired wire identity is reassigned while old values remain supported. | Reserve it and allocate a new identity. | T02 |
| CF54 | P | A durable workflow resumes under an incompatible history interpretation. | Admitted workflow version or explicit migration. | T05 |
| CF55 | P | A supported resumable document references neither retained code nor a safe recovery path. | Retained compatible artifacts or explicit non-destructive recovery. | L05 T09 |
| CF56 | P | An optimizer's placement violates privacy, residency, consistency, or resource policy. | Any equivalent admissible plan. | E09 T08 K10 |
| CF57 | P | Generated artifacts request imports beyond the admitted resolved grant. | Exact operation/capability projection with independent artifact validation. | S07 S10 |
| CF58 | P | A dependency resolves outside its approved source/registry authority. | Explicit newly reviewed dependency authority. | S02 |
| CF59 | P | A cached build artifact lacks matching complete inputs or admitted provenance. | Verified cache entry or local rebuild. | S05 S06 |
| CF60 | P | Arbitrary same-realm npm is advertised as capability-confined without an actual isolation mode. | Explicit trusted mode or tested confined execution. | W05 |
| CF61 | S | A required control reference names no valid label/error/target in the known template. | Correct relationship or supported dynamic relationship checked at runtime. | N01 N04 |
| CF62 | S | The generated control uses incompatible semantic state, such as claiming modality while intentionally permitting background interaction. | A nonmodal control or actual modal protocol. | N03 N04 |
| CF63 | S | A form requires every intermediate edit to already inhabit its submitted domain type. | Derived draft type and checked submit transition. | M01 |
| CF64 | S | A progressively enhanced form lacks a native command/decoding/outcome path. | Explicit JS-required feature or equivalent native submission. | M03 |
| CF65 | P | A required activation-gated operation is planned solely behind delayed replay with no activation-preserving path. | Synchronously available native action or an explicit new interaction. | L06 |
| CF66 | S | A translated message violates the declared interpolation/selection contract. | Valid locale-specific message with the required typed variables. | O03 |
| CF67 | S | A local wall time is silently converted to an instant without required zone/ambiguity context. | Explicit conversion policy or unambiguous known input. | O05 |
| CF68 | S | Numeric money combines known incompatible currencies without a conversion operation. | Explicit authoritative exchange/rounding policy. | O07 |
| CF69 | S | A partial protocol result is consumed as complete without handling missing/error alternatives. | Explicit partial-state handling or validated complete result. | W02 |
| CF70 | S | A semantic projection is independently redefined while claiming the same authority identity. | Derivation, or a genuinely separate versioned contract with an explicit adapter. | J10 V01 |
| CF71 | S | A measurement requires a field not present on the resolved instrumentation record type. | Correct per-script attribute or explicit optional-support handling. | U01 |
| CF72 | P | A release claims a guarantee whose required witness/prerequisite is missing or failed. | Narrower claim with recorded evidence or completed prerequisite. | U04 U10 |

A valid program may still fail at runtime because permissions changed, a unique constraint conflicted, a device disappeared, a payment outcome is unknown, or storage was evicted. Such programs are not rejected merely for encountering legitimate uncertainty.

## Part 6. Runtime and host must guarantee

The following are protocol obligations under declared host/adapter assumptions. They are not claims that compilation establishes the live fact. Implement tests against the generated runtime and actual enforcement boundary, not only a hand-written simulation.

| ID | Guarantee and adversarial witness | Census leaves |
|---|---|---|
| RT01 | A late completion for an obsolete owner generation cannot mutate its replacement. Complete after unmount and remount. | C03 M06 |
| RT02 | Releasing one subscriber does not abort still-owned shared work. Cancel one of two subscribers during refresh. | C04 |
| RT03 | Live owned handles receive the specified cleanup attempt at most once per ownership termination; cleanup failure is observable where relevant. Process death is a separate guarantee. | C01 C10 |
| RT04 | Duplicate mount/reconnect does not create unintended logical subscriptions. Preserve intentional multiple owners. | C02 |
| RT05 | Child work does not outlive its owner without transferred/durable authority. Crash and cancel at every await boundary. | C06 |
| RT06 | Causal deadlines shrink across retries and hops; queue time counts under the chosen contract. | C07 |
| RT07 | Cancellation changes observation/lifetime, not an unobserved remote commit outcome. | C05 G06 |
| RT08 | A deferred operation cannot silently switch principal/tenant context while retaining the old intent's authority. | B06 D03 |
| RT09 | All protected commands enforce live permission at the declared commit/read consistency boundary. Race revocation with execution. | D01 D04 |
| RT10 | Every data access, join, batch, cache, and subscription uses the actual bound tenant/audience. Supply foreign-scope IDs. | D03 E02 |
| RT11 | Session/account changes retire incompatible cached grants, pending attachment state, and local partitions according to policy. | D05 D06 Q09 |
| RT12 | Delegation never exceeds the effective issuer grant, and actor/subject remain distinguishable. | D07 D10 |
| RT13 | Identity adapters verify the expected issuer/audience/transaction and report protocol failure without creating a session. | D09 |
| RT14 | Sensitive values remain absent from unauthorized wire, error, telemetry, cache, and replay sinks. Include failed requests and generated diagnostics. | E01 E03 E07 |
| RT15 | Purpose/consent checks apply at delayed dispatch as required by policy, not merely at SDK initialization. | E04 |
| RT16 | Managed deletion/retention tracks required derived copies and reports unfinished external obligations. Backups obey the declared retention/recovery policy. | E08 |
| RT17 | Decoders reject ambiguous/unsupported representations without executing payload-directed behavior or exhausting admitted resources. | F03 F04 F11 |
| RT18 | Foreign data cannot introduce executable getters/prototypes into an inert-data boundary. | F05 |
| RT19 | SSRF restrictions apply to actual resolved destinations and redirects; restricted credentials stay within scope. | F07 K09 |
| RT20 | Generated ingress strips or authenticates internal-only metadata and rejects ambiguous request framing. Test the deployed chain. | D02 F08 |
| RT21 | Credentialed command protection remains effective on direct native requests and bypassed UI paths. | F09 M02 |
| RT22 | Deployed security headers match admitted scripts/resources/embedding policy; unsupported browser enforcement is not represented as enabled. | F10 |
| RT23 | Same intent plus same authorized payload returns or reconciles one commitment outcome; concurrent duplicates cannot both create the protected change. | G01 G04 |
| RT24 | Same identity with different bound intent is rejected; different legitimate identities with equal payload remain separate. | G02 G03 |
| RT25 | Expired dedupe entries cannot silently authorize unsafe automatic replay. Test delayed arrivals beyond retention. | G05 |
| RT26 | Lost acknowledgments preserve an unknown state until resolved, rather than synthesizing rejection or success. | G06 |
| RT27 | Removing failed optimism preserves later confirmed state and other pending intents. Test both completion orders and conflicting/nonconflicting operations. | G07 |
| RT28 | Commit-linked outbox obligations survive crash before/after publication; consumers tolerate duplicate delivery. | G08 |
| RT29 | Compensation failure becomes a domain outcome with an auditable unresolved obligation. | G09 |
| RT30 | Verified duplicate or out-of-order webhook delivery cannot force an illegal obsolete transition. | G10 |
| RT31 | Declared uniqueness, version checks, and multi-row invariants are enforced at the commit authority. | H01 H02 H03 |
| RT32 | Transaction retries re-execute only replay-safe effects and preserve the command's intent. | H05 |
| RT33 | Snapshot/causal/read-your-writes requirements are honored or explicitly fail/degrade according to policy. | H04 I03 I07 |
| RT34 | Query invalidation includes range membership, empty results, and external change contracts. Insert into an initially empty query. | H06 X05 |
| RT35 | Cursors bind ordering, audience, and compatibility; data movement cannot silently violate the chosen paging semantics. | H08 O09 |
| RT36 | Leadership changes fence obsolete writers under the claimed storage protocol. | I01 |
| RT37 | Older replies cannot overwrite an inadmissibly newer resource state. | I02 J04 |
| RT38 | Causal gaps and expired replay cursors produce explicit recovery, not false synchronization success. | I04 I08 R01 |
| RT39 | Tombstone/replica-retirement policy prevents unintended resurrection after compaction. | I09 |
| RT40 | Cache selection includes all known response-varying inputs, with compatible freshness and authorization revisions. | J01 J02 J06 |
| RT41 | Invalidations correspond to committed revisions and cannot be overwritten by stale refreshes. | J03 J04 |
| RT42 | Refresh sharing, retry, and reconnect remain within per-key and aggregate budgets. | J05 K01 K02 |
| RT43 | Error/negative caching never inherits a success policy without explicit permission. | J07 |
| RT44 | Cache key cardinality, stream queues, pools, queries, and telemetry are bounded by admitted resource policy. | J09 K03 K04 U07 |
| RT45 | Scaling and recovery respect fixed downstream capacity; overload does not turn into an unbounded queue. | K02 K05 |
| RT46 | Ephemeral hosts do not falsely report durable jobs, global dedupe, or globally shared state. | K06 C06 |
| RT47 | Stream framing survives arbitrary chunk boundaries, truncation, reset, and oversized inputs. | K08 R03 |
| RT48 | Part/entity identity preserves native state through keyed movement, repeated instances, and structural updates. | L01 L02 L09 |
| RT49 | Attach/resume verifies actual document, handler, captured state, and compatibility before privileged interaction. | L03 L04 L05 |
| RT50 | Gesture-gated operations either run with valid browser activation or report/request renewed interaction. | L06 |
| RT51 | Foreign DOM mutation produces bounded recovery rather than wrong-target mutation or disabling translation/copy. | L07 |
| RT52 | Portals, fragments, and shadow-boundary integrations preserve logical owner cleanup and tested event/a11y relationships. | L08 |
| RT53 | Native and enhanced form submission share the same authoritative operation/decoder/policy and intent semantics. | M02 M03 M04 |
| RT54 | A stale validation response cannot replace newer draft text. Composition, autocorrect, selection, undo, and autofill remain coherent. | M05 N06 |
| RT55 | History, URL, focus, and scroll restore according to the navigation contract, not a generic remount heuristic. | M07 M08 |
| RT56 | Compatibility recovery never silently discards protected unsaved work. Test storage failure during draft preservation. | M10 |
| RT57 | Generated native controls expose actual state and operable keyboard/pointer behavior; modal scopes release correctly. | N01 N02 N03 N04 |
| RT58 | Announcement and motion policies reflect user intent and preferences without announcing every internal state transition. | N05 N10 |
| RT59 | Text formatting/segmentation/bidi and monetary/time conversions preserve their declared units and policies. | O01 O02 O04 O05 O06 O07 O08 |
| RT60 | Layout scheduling avoids generated interleaving without claiming external invalidations cannot occur. Observer feedback remains bounded. | P01 P02 P03 |
| RT61 | Freeze/discard/resume does not corrupt authority, queued work, or deadlines; no final unload callback is required for durability. | Q01 Q02 C10 |
| RT62 | Local acceptance, persisted state, synchronization, quota failure, and eviction remain distinguishable outcomes. | Q03 Q04 |
| RT63 | Worker/page/local-schema upgrades coordinate supported old clients and expose blocked or interrupted migration. | Q05 Q08 |
| RT64 | Duplicate tab leaders cannot duplicate protected commitments; local coordination failure remains recoverable. | Q06 |
| RT65 | Queued offline intents are reauthorized or validated against their admitted offline grant policy at commit. | Q07 |
| RT66 | Presence expires as advisory state; media and connections release under their ownership contract and expose revoked permission/device failure. | R04 R05 R07 |
| RT67 | Realtime negotiation, transport fallback, and media recovery preserve user stop/pause and expose unsupported modes. | R06 R08 R09 |
| RT68 | Artifact admission checks actual imports, provenance, and grants, not only declared manifests. | S03 S06 S07 W05 |
| RT69 | Every reachable rollout/rollback/workflow state uses an admitted compatibility interpretation. | T01 T03 T04 T05 |
| RT70 | Replay is isolated from production commitments and obeys trace privacy/retention. | U08 E03 |
| RT71 | Instrumentation distinguishes absent, unsupported, sampled, and zero values and preserves causal identity. | U01 U02 U06 |
| RT72 | Recovery tooling and restore drills operate without the failed dependency and report the actual recoverable state. | X02 X03 |

## Part 7. What must remain explicit domain policy

These are conceptual policy fields, **not proposed accepted Pleris grammar**. Applications should author only choices relevant to their domain; reusable domain modules and adapters can supply explicit, inspectable defaults. The compiler derives implementation mechanics after the policy is known.

| Decision | Smallest useful policy surface | What must not be inferred magically |
|---|---|---|
| Consistency under partition | Required read/write consistency plus `unavailable`, `stale`, or `queue` outcome policy | Whether stale inventory, balance, or permission is acceptable |
| Offline acceptance | `read_only`, `queue_then_reauthorize`, or a named bounded offline-grant policy | Permission to commit indefinitely after revocation |
| Conflict resolution | Named merge operation, compare-and-set conflict, server arbitration, or user resolution | A universal correct merge for all records |
| Concurrent optimism | Serial lane or named intent-overlay/conflict contract | Commutativity of arbitrary transformations |
| Commitment uncertainty | Reconcile by authoritative identity; user confirmation or manual review when unresolved | Whether a lost payment response means failure |
| Compensation | Domain transition and unresolved-compensation state | That every external effect can be reversed |
| Authority freshness | Required policy revision/causal relation or a declared staleness allowance | Instantaneous global revocation during partitions |
| Delegation/impersonation | Allowed action/resource scope, audience, expiry, actor visibility | The user's business reason for granting authority |
| Freshness and errors | Value freshness plus separate stale/error/negative reuse choices | That a cached denial should persist as long as a public catalog |
| Retention, purpose, consent | Named purpose, recipients, retention/deletion policy, and relevant consent requirement | Applicable legal basis or whether all copies can be recalled |
| Data location | Allowed regions/hosts and external recipient constraints | Unstated residency obligations |
| Work and cost | Named budget class, admission priority, and overflow behavior | Infinite provider spend in exchange for lower latency |
| Stream pressure | `lossless_bounded`, `coalesce_latest`, `drop`, or `reject`, as appropriate | Whether dropping a location update is equivalent to dropping a payment event |
| Time | Civil or elapsed recurrence, zone, gap/fold behavior | User intent at an ambiguous daylight-saving transition |
| Money | Currency, precision, authoritative quote, rounding stage | Tax or exchange policy from a generic decimal type |
| Text identity | Domain normalization/case policy and display preservation | A universal rule for account names, file paths, and human names |
| Locale | Supported locales, fallback, typed messages, formatting context | Translation quality or culturally appropriate copy |
| Navigation | New-navigation versus restoration behavior and unsaved-work recovery | A single correct focus or scroll target for every interaction |
| Accessibility feedback | Meaningful labels, status urgency, and necessary alternatives | Whether an automatically generated description is useful |
| Media | Desired permission/capture/playback state and recovery interaction | That a device should restart after a user intentionally stopped it |
| Compatibility retirement | Supported client/data/workflow generations and retire/migrate/export policy | Permission to reload away a draft |
| Disaster recovery | Recovery objectives and admissible loss/unavailability | That any available replica is an acceptable backup |
| Experimentation | Unit, assignment authority, eligibility, exposure, metric meaning | That render or click counts establish business success |
| Trusted interop | Trusted mode versus a named tested isolation boundary | That a package is harmless because it has types or a signature |

A compact conceptual example is: a note editor declares user-scoped data, queued offline edits that reauthorize on reconnect, and a named conflict-resolution policy. It should not additionally require hand-authored fetch wrappers, abort plumbing, cache keys, event schemas, server/client validators, or deployment placement for those same facts. A payment flow declares authoritative commitment and reconciliation; it must not inherit the note editor's merge policy merely because both use resources.

## Source links

[R3]: SOURCES.md#r3
[R8]: SOURCES.md#r8
