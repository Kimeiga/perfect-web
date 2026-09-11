## I. Replication, ordering, and distributed policy

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| I01 | Failover creates divergent writers | GitHub October 2018 | Leadership and write authority must remain compatible with replication and application topology. | Fencing; consensus-backed leadership | specified but unproven | D+E+H | Enforce lease epochs/fencing at storage and represent unavailable consistency during partitions. | X+D | [S04] |
| I02 | Older response overwrites newer state | Model/spec: search results arrive out of order | Delivery must be checked against the relevant request/resource revision. | Generation checks; causal tokens | partially covered | C+D | Discard or separately expose stale responses; arrival time is not semantic ordering. | R | [R4] [R8] |
| I03 | Read loses the caller's acknowledged write | Model/spec: redirected read hits lagging replica | A read-your-writes contract must carry the required acknowledged revision. | Session consistency tokens | specified but unproven | D+E | Route or wait for a sufficient replica, or expose unavailable/stale according to declared policy. | R+D | [R1] [S04] |
| I04 | Causally dependent events applied backwards | Model/spec: update arrives before creation | Causal dependencies must be satisfied or explicitly buffered/reconciled. | Causal metadata; replayable logs | specified but unproven | D+H | Carry causal prerequisites where required; do not substitute wall-clock timestamps. | R+D | [S18] [S39] |
| I05 | Wall clock used as authoritative order | Automerge conflict-ordering contrast | Clock timestamps cannot silently stand in for causality or globally agreed order. | Logical clocks; trusted time services | missing | B+D+H | Distinguish instant, duration, logical revision, and deadline types; declare clock uncertainty where it affects policy. | T+D | [S39] [S67] |
| I06 | Convergence mistaken for domain correctness | Automerge conflicts | Replica convergence must not be presented as preservation of every business invariant. | Conflict states; invariant confluence | specified but unproven | B+D+H | Require per-domain merge semantics; coordinate nonconfluent invariants or represent conflict. P0. | D | [S35] [S39] [R1] |
| I07 | Partition policy hidden in automatic placement | CALM/coordination examples | Consistency, availability, and offline acceptance requirements must be mutually satisfiable under the chosen failure model. | Explicit consistency policies | specified but unproven | B+E+H | Reject incompatible plans, not legitimate policies; explain which guarantee must weaken during a partition. | D | [S35] [S36] [R11] |
| I08 | Reconnect skips an undelivered update | Repo known-versus-held cursor findings | An observed version is not necessarily a delivered or applied version. | Separate cursors; resumable logs | covered | D | Preserve the existing scoped E7 distinction; extend witnesses to reconnect, retention gaps, and multi-node delivery. | R | [R4] [R8] |
| I09 | Deletion resurrected by old replica | Model/spec: offline device reconnects after delete | Deletion and compaction must account for replicas that can still reintroduce earlier state. | Tombstones; epochs; replica retirement | missing | D+H | Define replica membership, tombstone retention, and post-retirement resync policy. | D+R | [S38] [S39] |

## J. Resource identity, caches, and materialization

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| J01 | Cache key omits a response-varying input | RFC 9111 selection model | Reuse is allowed only across requests equivalent for the response contract. | Vary; semantic keys | partially covered | C+D | Derive keys from dependencies, audience, locale, policy, and compatibility generation. | E+R | [S17] [R1] |
| J02 | HTTP freshness confused with application validity | Model/spec: fresh cached price after policy change | Age bounds and domain validity are independent conditions. | Versioned dependencies; invalidation | specified but unproven | C+D+H | Combine freshness with dependency/policy revisions; freshness alone never authorizes private reuse. | R+D | [S17] [R11] |
| J03 | Cache invalidation published before commit | Model/spec: refetch returns precommit data and stays cached | Invalidations must correspond to committed revisions and survive publisher failure. | Transactional outbox; revisioned events | specified but unproven | C+D | Publish commit-linked changes; fence refreshes by the minimum required revision. | R | [R1] [S16] |
| J04 | Stale refresh replaces newer cache entry | Model/spec: slow refresh wins after new data installed | Cache updates must respect version order and compatibility. | Compare-and-set cache writes | specified but unproven | D | Install only admissible revisions; singleflight alone does not prevent cross-generation races. | R | [R1] [R8] |
| J05 | Cache stampede on shared expiry | Model/spec: many subscribers hit one missing entry | Refresh work must be shared and admitted within bounded capacity. | Singleflight; refresh spreading | specified but unproven | D+E | Deduplicate per semantic key and partition, budget global refresh work, and allow explicit stale serving policy. | R+D | [S33] [S34] [R1] |
| J06 | Independent caches disagree on authority | Model/spec: CDN, service worker, and resource store each choose freshness | Every cache layer must implement compatible projections of one reuse policy. | Shared deployment/cache plans | specified but unproven | C+E | Derive headers, worker strategy, resource keys, and purge events from one declared contract. | E+X | [S17] [S49] [R1] |
| J07 | Negative or error cache hides recovery | Model/spec: temporary permission failure cached as permanent absence | Failure and absence have separate cacheability, audience, and lifetime contracts. | Typed cache outcomes | missing | D+H | Do not cache errors by successful-value policy; require admissible negative/error reuse semantics. | D+R | [S17] [S24] |
| J08 | Shared server mutable state leaks across requests | SvelteKit documented example | Request/session state must not enter process-global mutable storage without matching partitioning. | Scoped contexts; isolation | partially covered | B+D | Make shared state explicit and typed by its scope; enforce the same scope in generated hosts. | E+R | [S54] [R1] |
| J09 | Unbounded key cardinality exhausts cache | Model/spec: unique attacker-controlled search keys | Correct per-key behavior must still respect aggregate memory and work budgets. | Bounded caches; admission policies | specified but unproven | D+E | Budget entry count/bytes, tenant share, and refresh fanout; evictions must not masquerade as data loss. | R+X | [S33] [R1] |
| J10 | Materialization dependencies become a second authored authority | Repo explicit-events roadmap | A derived projection must not require separately maintained duplicate dependency facts when these are already known. | Typed changes; compiler dependency graphs | partially covered | C | Derive from the available semantic graph; retain explicit external change contracts where the compiler cannot observe mutations. | E+X | [R1] [R8] [R11] |

## K. Capacity, networking, edge, and host composition

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| K01 | Retry amplification across layers | Model/spec: client, gateway, and service all retry | Retry attempts across the causal call tree must share an admissible work budget. | Retry budgets; admission control | specified but unproven | C+D+E | Propagate intent and retry budget across generated hops; prevent independent exponential multiplication. P0. | R+X | [S13] [S34] [R1] |
| K02 | Thundering herd after recovery | Cascading-failure model | Recovery must not admit more queued or reconnect work than dependencies can handle. | Jitter; load shedding; staged recovery | specified but unproven | D+E | Bound reconnect, refresh, and queued-command drain rates per dependency and tenant. | R+X | [S34] [S06] [R1] |
| K03 | Noisy tenant consumes shared resources | Google SRE per-customer limits | One principal's admitted work must not consume another's guaranteed share. | Quotas; fair scheduling | specified but unproven | D+E+H | Compose CPU, memory, connections, query, and egress budgets with explicit fairness policy. P0. | X+D | [S33] [R1] |
| K04 | Backpressure stops at adapter boundary | WHATWG Streams model | Every producer-consumer boundary must bound buffering or define loss/admission behavior. | Bounded streams; flow control | specified but unproven | C+D+E | Propagate demand and cancellation through generated adapters; expose drop, coalesce, buffer, or reject policy. | R+D | [S50] [R1] |
| K05 | Autoscaling overwhelms a fixed dependency | Slack opening incident account | Scaling one tier must respect downstream capacity and connection limits. | Admission-aware autoscaling; pools | missing | D+E | Include database pools, ingress capacity, and provider quotas in the deployment resource graph. | X | [S07] [S33] |
| K06 | Host-local memory mistaken for durable coordination | Model/spec: serverless instance dedupe lost on restart | Shared durable guarantees cannot depend on one ephemeral process. | Durable stores; fenced leases | specified but unproven | B+E | Reject plans claiming global dedupe, sessions, or jobs from unshared ephemeral state. | X | [R1] [S37] |
| K07 | Synchronous heavy work starves interactive tasks | Model/spec: regex or parser monopolizes worker | Scheduled work must yield, finish within a budget, or execute in an isolatable worker. | Fuel; worker isolation; cooperative scheduling | partially covered | D+E | Apply hard host limits where possible and cooperative slicing in generated browser code; FFI requires separate containment. | P+X | [S02] [S61] [R4] |
| K08 | Connection or stream resets misclassified as business failure | Model/spec: body lost after headers or commit | Transport progress is not domain outcome or whole-message validity. | Framed protocols; typed transport outcomes | specified but unproven | D | Decode complete records, expose partial transport state, and reconcile commands by intent identity. | R+X | [S50] [S13] |
| K09 | Redirect forwards authority to the wrong peer | OWASP SSRF redirect concerns | Redirects must preserve granted destination and credential scope. | Restricted clients; authenticated adapters | missing | D+E | Recheck every hop and never forward sensitive credentials solely because a library follows redirects. | X | [S22] |
| K10 | Speculative fanout exceeds time or financial budget | Model/spec: prefetch triggers many paid origin calls | Performance optimization must preserve effect, privacy, and resource policy. | Budgeted planners; profile-guided optimization | specified but unproven | B+C+D+H | Choose only admissible plans; expose cost and fanout diffs rather than silently spending more to reduce latency. | P+D | [R11] [S33] |

## L. DOM identity, rendering, and resumption

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| L01 | Logical part aliases another instance | Repo E7 part-identity work | A rendered part identity includes its instance context, not only its template-local name. | Instance-scoped part IDs | covered | C+D | Retain E7 identity tuple and test repeated/nested instances across moves and resumption. | U | [R4] [R8] |
| L02 | Key reuse transfers state to another entity | Model/spec: list index reused after insertion | UI state must follow the intended entity identity across structural edits. | Stable keyed reconciliation | partially covered | C+D+H | Infer keys from domain identity; require a key policy when multiple valid identities exist. | U+D | [R1] [R4] |
| L03 | Server and browser attach to incompatible structure | Angular hydration constraints | Attachment must target the structure and state that were actually emitted. | Hydration validation; resumable manifests | partially covered | C+D | Validate boundary identity and compatibility; isolate mismatch recovery instead of silently attaching to the wrong tree. | U | [S56] [R4] |
| L04 | Capture crosses a serialization boundary unsafely | Qwik lexical captures; Pleris corpus | A resumable handler may capture only data supported by its privacy and serialization contract. | QRLs; typed closure serialization | partially covered | B+C+D | Reject server resources and secret captures; bind decoded captures to build/schema/authority context. | E+U | [S57] [R1] |
| L05 | Old HTML references removed executable code | Model/spec: delayed click after deployment | A resumable document must retain a valid executable/compatibility path for its admitted lifetime. | Content-addressed chunks; version recovery | partially covered | C+E+H | Retain compatible artifacts or negotiate recovery without losing user drafts; current E7 scope is not full rollout proof. | U+X | [S57] [R4] [R11] |
| L06 | Lazy handler loses user activation | HTML transient activation model | Delayed loading cannot assume a gesture survives until a gated browser operation runs. | Activation-aware native actions | missing | C+D+F | Keep necessary gesture-bound work synchronously available, or request a new interaction; synthetic replay cannot mint trust. P0. | U+X | [S53] [S57] |
| L07 | Foreign DOM mutation invalidates renderer assumptions | React translation issue 11538 | The renderer must detect or tolerate changes outside its ownership model. | Island boundaries; guarded DOM repair | specified but unproven | D+G | Recover bounded owned regions while preserving translation, selection, and native controls; do not disable user tools as a fix. | U+X | [S47] [R11] |
| L08 | Fragment or portal loses logical ownership | Model/spec: teleported dialog outlives source scope | Physical DOM position, logical owner, and event/a11y relationships are distinct. | Scoped portals; part ranges | specified but unproven | C+D | Track owned ranges and logical cleanup across portals, shadow roots, and moves; test event retargeting. | U+X | [R1] [R11] [S45] |
| L09 | Optimization mutates unaffected DOM | Model/spec: unrelated subtree recreated for one value | Generated updates must preserve semantically unaffected identity and browser state. | Fine-grained DOM parts | partially covered | C+G | Measure affected part writes and identity, not merely component callback count; allow necessary structural work. | U+P | [R4] [S56] |


[S02]: ../SOURCES.md#s02
[S04]: ../SOURCES.md#s04
[S06]: ../SOURCES.md#s06
[S07]: ../SOURCES.md#s07
[S13]: ../SOURCES.md#s13
[S16]: ../SOURCES.md#s16
[S17]: ../SOURCES.md#s17
[S18]: ../SOURCES.md#s18
[S22]: ../SOURCES.md#s22
[S24]: ../SOURCES.md#s24
[S33]: ../SOURCES.md#s33
[S34]: ../SOURCES.md#s34
[S35]: ../SOURCES.md#s35
[S36]: ../SOURCES.md#s36
[S37]: ../SOURCES.md#s37
[S38]: ../SOURCES.md#s38
[S39]: ../SOURCES.md#s39
[S45]: ../SOURCES.md#s45
[S47]: ../SOURCES.md#s47
[S49]: ../SOURCES.md#s49
[S50]: ../SOURCES.md#s50
[S53]: ../SOURCES.md#s53
[S54]: ../SOURCES.md#s54
[S56]: ../SOURCES.md#s56
[S57]: ../SOURCES.md#s57
[S61]: ../SOURCES.md#s61
[S67]: ../SOURCES.md#s67
[R1]: ../SOURCES.md#r1
[R4]: ../SOURCES.md#r4
[R8]: ../SOURCES.md#r8
[R11]: ../SOURCES.md#r11
