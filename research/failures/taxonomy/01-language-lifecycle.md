## A. Value meaning and language soundness

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| A01 | Nominal identity erased at calls | Repo E9 reopening | A value must retain its domain identity through every argument position. | Pleris resolved types; nominal types | partially covered | B | Complete argument inference and unification, including nested carriers; never accept because names or lowered shapes match. P0. | T | [R2] [R3] [S58] |
| A02 | Declared result not checked | Repo E9 reopening | Every returned value must inhabit the declared result, on every reachable return path. | Typed languages; Pleris E9 plan | partially covered | B | Check explicit and implicit returns against the single resolved type representation. P0. | T | [R2] [R3] |
| A03 | Missing business-state alternative | Model/spec: newly added order state | A closed-state consumer must handle every reachable alternative or explicitly propagate the unmatched result. | Elm-style ADTs; Pleris corpus | partially covered | B | Exhaustiveness over resolved constructors, not string spelling; test nested patterns and foreign/open variants separately. | T | [R1] [R8] [R12] |
| A04 | Absence confused with empty or failure | Model/spec: empty list versus missing result | Distinct semantic outcomes must not collapse through representations or decoding. | Option/Result; Pleris erasure experiments | partially covered | A+B | Preserve tagged alternatives across JS, JSON, database nulls, and ABI projections. | T | [R5] [S71] |
| A05 | Unvalidated refinement | Model/spec: negative quantity from HTTP | A refined domain value may only enter through a constructor that establishes its predicate. | Decoders; refinement constructors | specified but unproven | B+D | Derive boundary validation; prove decidable predicates statically when possible and validate remaining predicates at runtime. | T | [R1] [S71] |
| A06 | Implicit precision or unit loss | Model/spec: integer identifier rounded by JSON consumer | Conversion must preserve meaning or expose a checked conversion outcome. | Typed numeric APIs; JSON constraints | specified but unproven | B+D+H | Specify widths, overflow, decimal/money units, and rounding policy; do not silently coerce through a browser number. | T+D | [R1] [S71] |
| A07 | Mutable alias invalidates a checked value | Model/spec: foreign object changes after decode | Evidence about a value must remain valid for its lifetime. | Ownership; immutable decoded data | specified but unproven | B+D | Copy or freeze boundary values, or use audited ownership transfer; validation of a mutable alias is not durable proof. | T+X | [R1] [S29] |
| A08 | Unknown analysis treated as safe | Repo false-green history | Unresolved type/effect/placement evidence is not an empty effect row or permission grant. | Pleris blocked resolved types | partially covered | A+B | Propagate blocked evidence and stop deployable code generation; IDE recovery must remain separate. P0. | E | [R2] [R3] [R8] |
| A09 | Protocol state used out of order | Model/spec: transaction used after close | Operations must be valid for the resource's current protocol state. | Typestate; scoped resources | specified but unproven | B+D | Use small library typestates and runtime checks at foreign boundaries, not general session-type annotations everywhere. | T+R | [R1] [S74] |
| A10 | Logical key confused with display text | Model/spec: translated label used as identifier | Stable entity identity must not depend on presentation or incidental object allocation. | Nominal IDs; keyed collections | partially covered | A+B | Derive stable domain keys and require explicit identity at genuinely dynamic boundaries. | T | [R4] [R8] [S65] |

## B. Effects, dependencies, and reactive causality

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| B01 | Incidental identity restarts work | Cloudflare dashboard 2025 | Request identity must follow semantic inputs, not allocation identity during rendering. | Resources; tracked reactive dependencies | partially covered | C+D | Derive resource keys and sharing from meaning; make render-owned imperative request creation unavailable. | E+R | [S01] [S59] [R1] |
| B02 | Derived state becomes another authority | React documented redundant-state example | A derivable value must not acquire an independent update path unless it is intentionally a snapshot or draft. | Computed values; signals | specified but unproven | C | Represent live derivations, snapshots, and editable drafts distinctly; eliminate synchronization effects for live derivations. | E | [S59] [R1] |
| B03 | Dependency omitted through indirection | Vue watcher source example; repo member fallback | Dependency analysis must resolve the computation actually called. | Tracked getters; resolved call graphs | partially covered | B+C+D | Follow resolved symbols and dynamic reads; require adapter dependency contracts where analysis cannot observe reads. | E+X | [S55] [R8] |
| B04 | Reactive glitch exposes mixed revisions | Model/spec: total updated before its inputs agree | Observers requiring a coherent snapshot must not see an intermediate inconsistent graph state. | Transactional reactive batches | specified but unproven | C+D | Define batch boundaries and deterministic propagation; preserve explicit streaming partial-state semantics separately. | E+R | [R1] [S50] |
| B05 | Reactive cycle creates unbounded propagation | Model/spec: mutually updating watchers | Feedback requires an explicit convergence, delay, or bounded-work contract. | Effect separation; bounded schedulers | specified but unproven | B+D+H | Reject known pure dependency cycles; bound dynamic feedback and expose nonconvergence instead of forbidding all feedback. | E+D | [S55] [R1] |
| B06 | Callback silently captures stale context | Model/spec: session changes before deferred callback | A deferred operation must declare whether it uses captured context or current context. | Structured resources; context binding | partially covered | C+D+H | Bind principal, resource generation, and intent at the correct boundary; invalidate incompatible captures on context change. P0. | R+D | [R1] [R4] |
| B07 | Speculation executes visible side effects | Model/spec: render or prefetch charges a customer | Speculative evaluation may perform only effects authorized for speculation. | Pure views; command separation | partially covered | B+C | Treat render, optimizer speculation, prefetch, and optimistic transforms as distinct execution contexts. | E | [R1] [R10] [S13] |
| B08 | Error disappears through effect handling | Model/spec: catch returns successful empty state | Recovery must not misrepresent an unresolved required outcome as success. | Result variants; explicit recovery | specified but unproven | B+D+H | Require typed recovery results; permit intentional best-effort work with an explicit contract rather than universal error logging. | E+D | [R1] [S72] |

## C. Ownership, cancellation, and resource lifecycle

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| C01 | Imperative resource leaks | Model/spec: listener or observer survives owner | Every acquired resource needs an owner and a release or transfer path. | Affine scopes; structured concurrency | partially covered | B+C+D | Infer owner from lexical scope, generate cleanup, and test actual asynchronous foreign resources. | R | [R1] [R4] |
| C02 | Duplicate subscription survives remount | Model/spec: two active handlers for one owner/key | Logical subscription multiplicity must match declared ownership. | Keyed subscription registries | partially covered | C+D | Deduplicate by owner and semantic subscription identity; distinguish intentional multiple subscribers. | R | [R1] [R8] |
| C03 | Completion mutates a destroyed owner | Model/spec: late request changes new page | Only a live matching owner generation can receive a completion. | Cancellation tokens; generations | partially covered | C+D | Gate delivery by owner generation even when the underlying operation ignores cancellation. | R | [R1] [R4] |
| C04 | One subscriber aborts shared work | Model/spec: one view leaves while another awaits | Shared work lives until its owning policy permits termination, not until any individual subscriber leaves. | Reference-counted singleflight | specified but unproven | C+D | Separate operation ownership from subscriber attachment and cancellation. | R | [R1] [R11] |
| C05 | Cancellation presented as rollback | Model/spec: charge committed after client abort | Stopping observation does not establish that a remote effect was not committed. | Explicit unknown outcomes | specified but unproven | D+H | Retain interaction identity and reconciliation route after local cancellation. P0. | R+D | [S13] [S14] [R11] |
| C06 | Detached task loses lifetime authority | Model/spec: response returns before important job persists | Work outliving its request must be durable or explicitly best effort under a host lifetime. | Durable jobs; structured tasks | specified but unproven | B+D+E | Require a durable/detached capability; disallow silently discarded promises as business commitments. | R+X | [R1] [S37] |
| C07 | Timeout budget expands across calls | Model/spec: each nested service restarts full timeout | Descendants must respect the remaining end-to-end deadline unless an explicit new workflow begins. | Deadline propagation | specified but unproven | C+D | Propagate monotonic remaining budgets; distinguish queue, connection, service, and workflow timeouts. | R | [S33] [S34] [R1] |
| C08 | Resource held across incompatible suspension | IndexedDB transaction lifecycle | A language reference cannot extend an external resource's valid activity window. | Typestate adapters; scoped transactions | missing | B+D | Model suspension restrictions per adapter; split preparation from the bounded active transaction. | R+X | [S74] |
| C09 | Reentrant callback observes half-transition | Model/spec: foreign callback runs during state mutation | Public callbacks must see an established state or an explicitly reentrant protocol. | Transaction boundaries; queued callbacks | missing | C+D | Define callback delivery boundaries and prevent partially initialized state escaping through interop. | R+X | [S29] [S74] |
| C10 | Finalizer assumed on process death | Browser discard; server crash | Durability and remote cleanup cannot depend on a final callback being delivered. | Leases; checkpoints; durable workflows | specified but unproven | D+E | Use leases or durable records for remote resources; treat local destructor guarantees as process-lifetime bounded. | R+X | [S42] [S37] [R1] |

## D. Principals, sessions, and authorization over time

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| D01 | Authentication mistaken for action permission | OWASP authorization examples | Principal authentication does not authorize an arbitrary action on an arbitrary object. | Object authorization; capabilities | specified but unproven | B+D | Generated commands require an authorization policy evaluated by the serving authority. | E+D | [S24] [R1] |
| D02 | Middleware becomes sole authority | Next.js CVE-2025-29927 | Caller-controlled transport metadata must not bypass authoritative authorization. | Endpoint enforcement; trusted ingress | specified but unproven | D+E | Authenticate internal context at trust boundaries and enforce command/data permission independently of routing middleware. | X | [S10] [R1] |
| D03 | Cross-tenant reference accepted | Model/spec: foreign tenant ID in valid request | Every tenant-bound reference must be valid in the operation's tenant context. | Scoped capabilities; tenant predicates | specified but unproven | B+D | Brand scoped references where useful and enforce scope in host/database access, including joins and bulk operations. P0. | T+X | [S24] [R1] |
| D04 | Authorization checked before a conflicting change | Zanzibar causal ACL/content problem | Protected reads or writes must honor the required relation between policy revision and data revision. | Zanzibar-style consistency tokens | missing | D+E+H | Specify authorization consistency and enforce atomic checks, version predicates, or causal minimum revisions. P0. | X+D | [S18] [R11] |
| D05 | Revocation leaves reusable cached authority | Model/spec: revoked session uses resumed resource | Cached decisions and resumable state must not outlive their authority contract. | Revisioned grants; revocation checks | partially covered | D+E+H | Bind cached grants to principal and policy generation; revalidate according to explicit revocation requirements. P0. | R+D | [S18] [S23] [R4] |
| D06 | Session fixation or account-switch confusion | OWASP session lifecycle | Session establishment and privilege changes must not preserve an attacker-controlled or obsolete binding. | Audited session adapters | missing | D+E | Generate rotation and context invalidation through the adapter; partition pending commands and local data across account changes. | X | [S23] |
| D07 | Delegation increases authority | Macaroons attenuation model | Delegated authority must be no broader than its issuer's effective authority. | Attenuated capabilities | specified but unproven | B+D | Preserve resource, action, audience, expiry, and delegation restrictions at every hop. | E+X | [S19] [R1] |
| D08 | Confused deputy spends its own authority | Model/spec: server fetches arbitrary private target for caller | A callee's ambient power must not substitute for the caller's delegated authority. | Object capabilities; explicit service grants | specified but unproven | B+D+E | Bind effect grants to the operation and principal context; do not expose unrestricted host clients to components. | E+X | [S19] [S22] [R1] |
| D09 | Federated identity bound to wrong issuer | OAuth dynamic trust model | A valid token must belong to the expected issuer, audience, transaction, and resource context. | OAuth/OIDC adapters | missing | D+E | Make identity-provider protocols audited adapters with typed outcomes and tested callback binding. | X | [S28] |
| D10 | Impersonation loses actor identity | Model/spec: support action logged only as customer | Effective subject and acting principal must remain distinguishable when delegation or impersonation occurs. | Delegation chains; audit context | missing | B+D+H | Carry actor and subject separately; restrict impersonation capability and preserve both in purpose-limited audit events. | E+D | [S19] [S25] |


[S01]: ../SOURCES.md#s01
[S10]: ../SOURCES.md#s10
[S13]: ../SOURCES.md#s13
[S14]: ../SOURCES.md#s14
[S18]: ../SOURCES.md#s18
[S19]: ../SOURCES.md#s19
[S22]: ../SOURCES.md#s22
[S23]: ../SOURCES.md#s23
[S24]: ../SOURCES.md#s24
[S25]: ../SOURCES.md#s25
[S28]: ../SOURCES.md#s28
[S29]: ../SOURCES.md#s29
[S33]: ../SOURCES.md#s33
[S34]: ../SOURCES.md#s34
[S37]: ../SOURCES.md#s37
[S42]: ../SOURCES.md#s42
[S50]: ../SOURCES.md#s50
[S55]: ../SOURCES.md#s55
[S58]: ../SOURCES.md#s58
[S59]: ../SOURCES.md#s59
[S65]: ../SOURCES.md#s65
[S71]: ../SOURCES.md#s71
[S72]: ../SOURCES.md#s72
[S74]: ../SOURCES.md#s74
[R1]: ../SOURCES.md#r1
[R2]: ../SOURCES.md#r2
[R3]: ../SOURCES.md#r3
[R4]: ../SOURCES.md#r4
[R5]: ../SOURCES.md#r5
[R8]: ../SOURCES.md#r8
[R10]: ../SOURCES.md#r10
[R11]: ../SOURCES.md#r11
[R12]: ../SOURCES.md#r12
