## U. Observability, tests, and evidence integrity

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| U01 | Measurement probes the wrong object | Pleris LoAF probe | A measured claim must follow the API's actual data model. | Typed instrumentation; positive controls | partially covered | B+G | Correct frame-versus-script attribution, rerun pinned environments, and annotate unsupported historical inference. P1. | V | [R14] [R15] [S64] |
| U02 | Missing observation becomes zero cost | Model/spec: no long-frame entry labeled no layout | Absence, unsupported measurement, sampled-out events, and observed zero are distinct. | Typed measurement outcomes | missing | A+C+G | Store capability, sampling, scope, and missingness with metrics; do not convert missing values to zero. | V | [S64] [S61] |
| U03 | Benchmark compares unequal behavior | Model/spec: optimized page omits accessibility or failure path | Performance comparisons need equivalent semantic workloads and disclosed environments. | Paired workloads; full artifact accounting | partially covered | G | Check correctness first, include all bytes and stages, and report variance and device/network constraints. | V+P | [R1] [R4] [S61] |
| U04 | False-green rejection uses the wrong reason | Repo risk queue | A negative test must reach and fail the intended invariant, not an earlier unrelated error. | Causal diagnostics; accepted neighbors | partially covered | G | Record diagnostic identity/span, valid prerequisites, minimal accepted neighbor, and mutation controls. | V | [R3] [R8] |
| U05 | Generator and test share the same incorrect oracle | Model/spec: serializer roundtrip preserves its own bug | Mechanically derived tests cannot independently establish the correctness of the shared semantic source or lowering. | Independent validators; differential tests | specified but unproven | G | Use standards vectors, independent parsers, model properties, and adversarial fixtures in addition to generated examples. | V | [R3] [R8] [S71] |
| U06 | Causal trace loses the initiating intent | Model/spec: retry appears as unrelated request | Observability must preserve causal identities across retries, hops, invalidations, and UI delivery. | Causal tracing; intent IDs | specified but unproven | C+D | Derive correlation from semantic operations, distinguishing intent, attempt, command, resource, and delivery. | R+V | [R11] [S13] |
| U07 | Telemetry causes overload or sensitive retention | Model/spec: high-cardinality trace stalls requests | Instrumentation must obey resource and privacy budgets and declare loss behavior. | Bounded telemetry pipelines | specified but unproven | C+D+E+H | Separate mandatory audit from lossy diagnostics; limit queues/cardinality and redact before external dispatch. | X+D | [S25] [S33] [R11] |
| U08 | Replay reissues real external effects | Temporal replay model | Replaying recorded execution must not repeat commitments or require unrecorded nondeterminism. | Effect logs; isolated replay | specified but unproven | B+D+E | Replay recorded effect results in an isolated host; treat unsupported foreign effects as replay boundaries. | R+V | [S37] [R11] |
| U09 | Experiment assignment or exposure disagrees across tiers | Model/spec: UI variant and server pricing flag diverge | Assignment, eligibility, and exposure events must share the intended experimental unit and policy version. | Typed experiment contracts | missing | C+D+H | Derive transport and schemas from declared assignment authority; business metric meaning is not inferred from UI events. | D+V | [R1] [S25] |
| U10 | Documentation status drifts from evidence | Repo old no-compiler snapshots and later E10 progress | A claim's status must reference a precise invariant, revision, and evidence scope. | Evidence ledger; generated status views | partially covered | C+G | Preserve historical records, link current summaries to evidence, and prevent a scoped pass from upgrading broader claims. | V | [R2] [R4] [R5] [R6] [R8] |

## V. Developer tooling and maintainable semantics

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| V01 | Semantic rename changes only some representations | Model/spec: renamed operation leaves manual route/schema behind | References to one semantic entity must resolve through one identity. | Semantic refactoring; derived contracts | partially covered | B+C+G | Drive rename, serializers, documentation, route bindings, and deployment projections from the same resolved entity. | T+V | [R3] [S75] |
| V02 | Inference hides the cause of a restriction | Model/spec: opaque cannot-cache error | Inferred facts must retain source-level provenance sufficient to explain and repair a conflict. | Constraint provenance; semantic IDEs | specified but unproven | C | Explain the shortest relevant dependency/capability/privacy path with source spans and a minimal safe repair. | E+V | [R11] [S75] |
| V03 | Whole-program checking destroys edit feedback | rust-analyzer incremental architecture | Small edits must invalidate only their semantic dependency closure where the model permits. | Incremental query systems | specified but unproven | C+G | Use stable interfaces and cached queries; measure invalidation sets and cold/warm feedback, not just total compile time. | V | [S75] [R11] |
| V04 | Incomplete editor code accepted for deployment | rust-analyzer error-tolerant syntax contrast | Editor recovery and executable validity require different acceptance boundaries. | Error-tolerant AST; strict build gate | partially covered | A+B | Retain partial IDE models but block executable projection of unresolved or invalid semantic facts. | T+V | [S75] [R3] |
| V05 | HMR preserves incompatible state or repeats effects | Model/spec: live edit changes state type or command body | Development reload must preserve only state/effects compatible with the new code. | Version-aware HMR | specified but unproven | C+D+H | Migrate compatible local state, dispose owned resources, and explicitly reset unsupported state without replaying committed actions. | U+V | [R11] [S37] |
| V06 | Generated stack obscures the authored cause | Model/spec: decoder stack has no source operation | Every generated operation must retain a usable mapping to its semantic source and causal context. | Source maps; operation-aware traces | specified but unproven | C+D | Map errors, async boundaries, ABI calls, and placement back to Pleris constructs; generated code remains inspectable but not required reading. | V | [R11] [S75] |
| V07 | Safety requires repeated annotations everywhere | Model/spec: every helper restates privacy and placement | Inferable mechanism must not become another authored contract that can drift. | Local inference; public interface contracts | specified but unproven | B+C+G | Infer within modules and expose boundaries; validate comprehension and error repair, not just line count. | E+V | [R1] [R11] |
| V08 | Unsafe escape becomes invisible in review | Model/spec: cast or plugin silently broadens authority | A safety escape must declare exactly which guarantee and authority it bypasses. | Capability-gated unsafe boundaries | specified but unproven | B+C+E | Require narrow audited adapters and semantic diffs for added effects, privacy release, placement, and host access. | E+X | [R1] [S29] [S30] |

## W. Foreign protocols and incremental adoption

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| W01 | Foreign signature omits relevant behavior | Model/spec: typed SDK function hides network or resource lifetime | An imported shape alone is not an effect, privacy, lifecycle, or failure contract. | Audited capability adapters | specified but unproven | B+D+E | Decode values and declare minimum relevant foreign behavior at one adapter boundary; ordinary code consumes the safe surface. P0. | X | [S58] [S29] [R11] |
| W02 | Partial API success treated as complete success | GraphQL documented partial data | Protocol-level partial results must remain distinguishable from successful completion. | Typed protocol result variants | missing | A+C+D | Preserve data-plus-errors and operation outcomes; do not flatten all successful HTTP responses into a full value. | T+X | [S72] |
| W03 | Foreign ABI ownership or memory lifetime mismatches | Model/spec: borrowed buffer retained after host call | The boundary must agree on value representation and ownership duration. | Component ABI adapters; ownership contracts | partially covered | B+C+D+G | Derive transfers and cleanup from actual ABI contracts and validate with independent consumers. | X+V | [R2] [R3] |
| W04 | Two frameworks independently own the same application state | Model/spec: migration island and host both mutate session/router | Incremental adoption must assign authority and lifecycle ownership at the integration boundary. | Typed island inputs/events | specified but unproven | B+C+D | Keep one owner for each state domain and route transition; bridge explicit values/events rather than mirror stores bidirectionally. | U+X | [R11] [S54] |
| W05 | Interop declaration claims a sandbox that does not exist | SES ambient-realm and shared-agent limitations | Static restrictions must correspond to actual execution isolation and granted capabilities. | Host sandbox; SES; workers or isolated frames | specified but unproven | B+E+G | Audit each isolation mode, including same-realm DOM access and CPU limits; capability metadata alone cannot confine arbitrary npm. P0. | X+V | [S29] [R1] |

## X. Operations, recoverability, and limits of automation

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| X01 | Destructive administration reaches the wrong environment | GitLab deletion incident | Administrative authority must bind target, environment, scope, and permitted destructive action. | Environment-scoped capabilities; staged operations | missing | B+D+E+H | Separate production deletion authority from routine application access; expose a semantic destructive-plan diff. | X+D | [S03] |
| X02 | Recovery depends on failed control-plane services | Meta October 2021 | Recovery authority and access must survive the failure domains they are intended to repair. | Out-of-band recovery paths | missing | E+G | Test independent access, rollback artifacts, credentials, DNS, and recovery tooling; do not promise compiler-only prevention. | X+V | [S06] |
| X03 | Backup exists but cannot be restored | GitLab recovery failures | A backup claim needs successful restoration evidence for the retained data and toolchain. | Restore drills; independent backup checks | missing | E+G+H | Record restore tests and recovery policy; replication and file existence are not sufficient backup evidence. | V+X | [S03] |
| X04 | Configuration blast radius is unbounded | Cloudflare 2019; Fastly 2021 | An admitted configuration change must not automatically acquire unrestricted fleet-wide failure scope. | Staged rollout; isolation; rollback | missing | D+E+G+H | Derive affected regions/services and stage risky changes; verify rollback through independent controls. | X+D | [S02] [S05] |
| X05 | Critical invariant depends on unobserved external mutation | Model/spec: DBA or external service changes tracked data | A guarantee must declare every writer or explicitly state its external assumptions. | Change capture; authority inventories | specified but unproven | B+D+E+H | Register external writers and adapter change feeds or invalidate conservatively; never assume compiler ownership of unmanaged systems. | X | [R11] [S16] |
| X06 | Code-generation agent gains deployment authority from content | Model/spec: untrusted issue text instructs credential export | Untrusted task material must not become authority to execute, disclose, or deploy. | Scoped build/agent capabilities | missing | E+H | Give coding agents task-bounded filesystem/network/deploy grants; review semantic capability changes independently of generated prose. | X+D | [S30] [S29] |
| X07 | Valid generated program implements the wrong product | AI-assistant study; synthetic wrong refund policy | A compiler cannot infer unstated intent or prove a deliberately incorrect specification correct. | Independent domain acceptance tests | intentionally outside scope | G+H+I | Evaluate typed semantic tasks with independent expected outcomes; do not claim elimination of all AI-written bugs. | D+V | [S76] [R1] |
| X08 | Browser/OS/network/provider defect escapes managed boundary | Model/spec: engine crash or provider outage | Guarantees end at explicit trusted-computing-base and external-service assumptions. | Isolation; fallback; operational resilience | intentionally outside scope | E+F+I | Minimize and document trusted dependencies, expose failures, and retain graceful degradation or recovery policy. | X+D | [S42] [S51] [R1] |

## Evidence links


[S02]: ../SOURCES.md#s02
[S03]: ../SOURCES.md#s03
[S05]: ../SOURCES.md#s05
[S06]: ../SOURCES.md#s06
[S13]: ../SOURCES.md#s13
[S16]: ../SOURCES.md#s16
[S25]: ../SOURCES.md#s25
[S29]: ../SOURCES.md#s29
[S30]: ../SOURCES.md#s30
[S33]: ../SOURCES.md#s33
[S37]: ../SOURCES.md#s37
[S42]: ../SOURCES.md#s42
[S51]: ../SOURCES.md#s51
[S54]: ../SOURCES.md#s54
[S58]: ../SOURCES.md#s58
[S61]: ../SOURCES.md#s61
[S64]: ../SOURCES.md#s64
[S71]: ../SOURCES.md#s71
[S72]: ../SOURCES.md#s72
[S75]: ../SOURCES.md#s75
[S76]: ../SOURCES.md#s76
[R1]: ../SOURCES.md#r1
[R2]: ../SOURCES.md#r2
[R3]: ../SOURCES.md#r3
[R4]: ../SOURCES.md#r4
[R5]: ../SOURCES.md#r5
[R6]: ../SOURCES.md#r6
[R8]: ../SOURCES.md#r8
[R11]: ../SOURCES.md#r11
[R14]: ../SOURCES.md#r14
[R15]: ../SOURCES.md#r15
