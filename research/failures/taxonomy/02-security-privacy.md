## E. Privacy, integrity, and data lifecycle

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| E01 | Private data enters public materialization | SvelteKit server-global example | Public output must not depend on private values without authorized release. | Information-flow checks; partitioned caches | partially covered | B+D | Check explicit and control-flow dependencies and enforce runtime partitions on generated artifacts. | E | [S54] [S20] [R1] |
| E02 | Different private principals share a partition | Model/spec: all User<T> values share one cache | Privacy category is insufficient without the actual principal or audience identity. | Principal-indexed labels | partially covered | B+D | Derive partition identity from the bound audience, not the textual privacy label alone. | E+R | [R1] [R8] [S20] |
| E03 | Secret escapes through errors or diagnostics | Model/spec: token in exception or trace | Diagnostic paths are disclosure sinks with their own audience and retention. | Typed/redacted logging | specified but unproven | B+C+D | Propagate sensitivity into errors, source previews, traces, replay, and support exports. | E+X | [S25] [R11] |
| E04 | Analytics receives data without allowed purpose | Model/spec: session field in vendor event | Permission to compute with data does not imply permission to export it for any purpose. | Purpose-scoped adapters | missing | B+D+H | Bind sink, purpose, consent state, and allowed fields at collection and dispatch; do not infer consent. P0. | E+D | [S25] [R11] |
| E05 | Declassification lacks accountable authority | Jif selective downgrading | Releasing confidential information requires explicit, bounded release authority. | Information-flow declassification | specified but unproven | B+D+H | Make release sites capability-gated and visible in semantic diffs; aggregation is not automatically anonymization. | E+D | [S20] [R11] |
| E06 | Untrusted data gains trusted meaning after shape decode | Model/spec: decoded client price treated as authoritative | Structural validity does not establish provenance or authority over a business fact. | Integrity labels; authoritative lookups | missing | B+D | Separate confidentiality from integrity; require trusted endorsements or authoritative recomputation. P0. | E+X | [S20] [S24] |
| E07 | Metadata leaks protected information | Model/spec: private existence exposed in cache key or error variant | Observable metadata must obey the declared disclosure policy, not only payload fields. | Information-flow analysis; sink control | specified but unproven | B+D+H | Include URLs, existence errors, resource identifiers, timing instrumentation, and cardinality in threat review; do not claim all side channels solved. | E+D | [S20] [S25] [R11] |
| E08 | Retention ignores derived copies | Model/spec: deleted record remains in logs and materializations | Declared deletion and retention must apply to managed dependent representations within stated guarantees. | Lineage and retention jobs | missing | C+D+E+H | Track managed copies, tombstones, backups, exports, and expiry; external recipients require a separate contract. P0. | X+D | [S25] [S38] |
| E09 | Cross-region placement violates data policy | Model/spec: private trace replicated to another region | Placement constraints apply transitively to data, metadata, backups, and support access. | Residency-constrained deployment plans | specified but unproven | B+E+H | Derive admissible plans from declared policy and verify actual host/storage placement. | X+D | [R11] [S25] |
| E10 | Compromised endpoint or user defeats secrecy | Model/spec: recipient copies already displayed content | Delivered plaintext cannot be made unknowable to its recipient by a compiler. | Browser/OS security; end-to-end encryption where applicable | intentionally outside scope | F+I | State endpoint and authorized-recipient limits explicitly; minimize delivered data and contain reachable authority. | X | [S20] [S29] |

## F. Injection, decoding, and hostile boundaries

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| F01 | Text becomes executable markup | OWASP XSS examples | Untrusted text must not acquire executable syntax authority. | Safe text sinks; typed HTML | specified but unproven | A+B+C+D | Generate context-correct output and require an audited capability for sanitized rich HTML. | E+X | [S21] [S27] [R1] |
| F02 | Unsafe URL or CSS context bypasses HTML escaping | OWASP context-specific guidance | Safety is specific to a sink grammar and allowed behavior. | Typed URLs; context-aware encoders | specified but unproven | B+D | Validate schemes and structure for each sink; do not use one universal SafeString type. | T+X | [S21] |
| F03 | Decoder executes attacker-specified behavior | React RSC CVE-2025-55182 | Wire data must not select arbitrary constructors, module loading, or privileged execution paths. | Closed decoders; host admission | specified but unproven | A+D+E | Generate non-executable tagged decoders with bounded work and independently fuzz the emitted endpoint. P0. | X+V | [S11] [R1] |
| F04 | Ambiguous duplicate keys or canonicalization | RFC 8259 duplicate-name behavior | Security checks and consumers must interpret exactly the same value. | Strict parsers; canonical representations | missing | D+E | Reject ambiguous input before normalization; sign or hash one specified representation. | X | [S71] [S12] |
| F05 | Prototype pollution or hostile accessors through JS | Model/spec: decoded object inherits attacker behavior | Data crossing into trusted code must not carry executable property behavior or ambient prototypes. | Inert data copies; SES | specified but unproven | D+E | Use audited inert representations and safe key handling; do not validate by invoking arbitrary getters. | X | [S29] [S71] [R1] |
| F06 | SQL or command injection | Model/spec: user input concatenated into executable query | Data must not change an operation's syntactic authority. | Parameterized queries; structured process APIs | specified but unproven | A+B+D | Emit structured queries and arguments; dynamic identifiers require an explicit constrained vocabulary. | E+X | [R1] [S21] |
| F07 | SSRF through destination changes | OWASP SSRF examples | The actual destination and every redirect must remain inside the granted network authority. | Egress controls; restricted HTTP clients | specified but unproven | D+E | Enforce scheme, resolved address, redirect, and credential-forwarding policy in the host adapter. | X+D | [S22] |
| F08 | Request smuggling across intermediaries | PortSwigger desync research | Every hop must agree on request boundaries and routing meaning. | Strict ingress normalization; protocol testing | missing | E+G | Reject ambiguous framing and test deployed proxy chains; a generated client alone cannot enforce this. | X+V | [S12] |
| F09 | CSRF on credentialed commands | Model/spec: unrelated site causes ambient-cookie action | A state-changing request must carry the intended origin/session interaction authority. | Origin checks; CSRF protocols; cookie policy | specified but unproven | D+E | Derive protection by authentication mode and form route; SameSite and CORS are not substitutes for command authorization. | X | [S23] [S24] [R1] |
| F10 | Security headers disagree with emitted behavior | CSP/Trusted Types contracts | Security policy and actual executable/resource capabilities must describe the same artifact. | CSP; Trusted Types; Permissions Policy | missing | C+E+F+H | Generate supported policies from approved assets and integrations; embedding and cross-origin choices remain explicit. | X+D | [S26] [S27] [S52] |
| F11 | Unbounded parsing or decompression | Model/spec: small input expands into expensive structure | Untrusted input must consume a bounded share of memory, CPU, and nested work. | Bounded decoders; host fuel and memory limits | partially covered | D+E | Limit depth, count, decoded size, compression expansion, and CPU before privileged work. | X | [S02] [S33] [R4] |
| F12 | Foreign callback or message forges authority | Model/spec: postMessage payload claims administrator | A decoded message needs authenticated source, channel, and action context, not merely a matching shape. | Capability channels; origin checks | specified but unproven | B+D+E | Bind channels to expected peers and narrow operations; never deserialize a host capability from an ordinary identifier. | E+X | [S29] [S24] [R1] |

## G. Commands, payments, and external commitments

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| G01 | Retry creates a second business action | AWS idempotency examples | Repeated attempts of one intent must not create additional commitments. | Idempotency keys; command journals | specified but unproven | B+D+E | Derive stable interaction identity, not a fresh identifier per attempt; require provider or transactional enforcement. P0. | R+X | [S13] [S14] [R1] |
| G02 | Equal payloads collapse distinct intentions | AWS identical-request discussion | Same arguments do not necessarily mean the same user action. | Caller-supplied intent identifiers | specified but unproven | A+C+D | Separate intent identity from payload hash; preserve legitimate repeated purchases. | R+D | [S13] |
| G03 | Same intent identity accepts changed payload | AWS idempotency contract | A committed identity must bind to one defined intent and principal scope. | Idempotency stores | missing | D+E | Atomically bind identity, canonical intent, and outcome; reject mismatched reuse. | R+X | [S13] |
| G04 | Deduplication record races with the effect | AWS atomic idempotency example | Deduplication and the protected state change must share a reliable commit protocol. | Transactional idempotency records | specified but unproven | D+E | Use a database transaction where possible; otherwise expose an adapter reconciliation protocol, not a local-memory promise. | R+X | [S13] [S16] |
| G05 | Deduplication expires before replay risk | AWS late-arriving requests | A retry contract must account for delayed requests and retention limits. | Retained intent journals; provider contracts | missing | D+E+H | Match permitted retries to dedupe lifetime; expired identities require reconciliation or a new explicit intent. | R+D | [S13] [S14] |
| G06 | Unknown commit represented as failure | Model/spec: response lost after charge | Transport failure does not decide the commitment outcome. | Pending/unknown/reconcile states | specified but unproven | A+D+H | Expose not-started, committed, rejected, and unknown as appropriate; retain identity for status reconciliation. P0. | R+D | [S13] [S14] [R11] |
| G07 | Older optimistic failure erases newer success | ADR25 restoration counterexample | Removing one speculative intent must preserve later confirmed state and unrelated pending intents. | Intent overlays; serialized mutation lanes; selective undo | partially covered | D+G+H | Specify ordered pending intents over confirmed state or serialize conflicting operations; do not blindly restore an old entry snapshot. P0. | R+D | [R10] [S40] |
| G08 | Database update and event publication split | Model/spec: commit succeeds before process crashes | A committed state change and its promised notification need a durable shared obligation. | Transactional outbox | specified but unproven | C+D+E | Generate outbox records in the same local commit and idempotent dispatch; delivery remains at least once unless a stronger adapter proves otherwise. | R+X | [R1] [S37] [S15] |
| G09 | Compensation treated as atomic rollback | Model/spec: refunded payment but shipment cannot be recalled | Cross-system recovery must model irreversible or partially reversible outcomes. | Sagas; durable workflow state | specified but unproven | D+H | Require domain compensation or reconciliation states; do not invent the business remedy. | D | [R11] [S37] |
| G10 | Webhook drives duplicate or obsolete transition | Stripe delivery contract | Verified delivery identity and domain transition relevance must both be checked. | Inbox dedupe; authoritative state reconciliation | missing | D+E | Verify source, dedupe delivery, then check current state/version; delivery IDs alone may not identify duplicate business events. | R+X | [S15] |

## H. Database invariants and query semantics

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| H01 | Lost update | PostgreSQL concurrency model | A write derived from a prior version must not silently overwrite an incompatible concurrent write. | Version predicates; atomic updates | specified but unproven | D+H | Generate compare-and-set or transactional operations from declared transition semantics; return conflict when required. | R+D | [S16] [R1] |
| H02 | Write skew violates a multi-row invariant | PostgreSQL isolation examples | Individually valid concurrent transitions must preserve the declared joint invariant. | Serializable transactions; coordination | specified but unproven | D+E+H | Select sufficient isolation or explicit coordination; a per-record version check is not enough. P0. | D+X | [S16] [S35] |
| H03 | Uniqueness checked before competing insert | Model/spec: two signups pass availability check | Uniqueness must be enforced at the commit authority. | Database unique constraints | specified but unproven | C+D | Derive the authoritative constraint and expose conflict as a normal outcome; frontend validation is advisory. | T+R | [S16] [R1] |
| H04 | Multi-step read observes incompatible snapshots | Model/spec: total and line items from different revisions | Related reads must satisfy the application's snapshot or causality requirement. | Snapshot transactions; revision tokens | specified but unproven | D+E+H | Carry read-consistency requirements through query planning and reject hosts that cannot meet them. | D+X | [S16] [R1] |
| H05 | Retry replays nontransactional side effect | Model/spec: email sent inside retried transaction | Transaction retry may repeat only effects with a compatible replay contract. | Effect-restricted transactions; outbox | specified but unproven | B+D | Keep external commitments outside retriable transaction bodies or route them through a durable adapter. | E+X | [S16] [S13] [R1] |
| H06 | Query dependency misses nonexistence or range | Model/spec: empty search cached after matching row inserted | Invalidations must include predicates, negative reads, and membership changes, not only returned row IDs. | Predicate tracking; explicit change domains | specified but unproven | C+D | Use conservative typed change events first; prove narrower dependency inference before relying on it. P0. | E+R | [R11] [S16] |
| H07 | N+1 query graph | Model/spec: one remote lookup per list row | Logical data demand should not impose unnecessary serial or per-row round trips. | Batched queries; query planners | specified but unproven | C+G | Batch only with equivalent auth, consistency, limits, and failure semantics; report unbatchable causes. | P+X | [R1] [S33] |
| H08 | Unstable pagination duplicates or skips data | Model/spec: offset page while rows are inserted | A pagination cursor must identify an ordering and consistency context. | Keyset pagination; snapshot cursors | missing | C+D+H | Derive stable tie-breakers and cursor scope; expose live-versus-snapshot paging as policy. | D+R | [S16] |
| H09 | Aggregate or join amplifies resource use | Model/spec: small API request causes a large join | Data access must satisfy runtime work and result-size budgets. | Query limits; execution monitoring | specified but unproven | D+E | Enforce row, time, memory, and concurrency budgets; profile query plans rather than promise static complexity inference. | P+X | [S33] [R1] |
| H10 | Database adapter overstates a guarantee | Model/spec: weak backend advertised as serializable | Required semantics must be supported by the actual selected database and deployment. | Capability negotiation; conformance suites | specified but unproven | B+E+G | Require adapter evidence for transactions, durability, ordering, and cancellation; reject unsupported deployment plans. | X+V | [S16] [R1] |


[S02]: ../SOURCES.md#s02
[S11]: ../SOURCES.md#s11
[S12]: ../SOURCES.md#s12
[S13]: ../SOURCES.md#s13
[S14]: ../SOURCES.md#s14
[S15]: ../SOURCES.md#s15
[S16]: ../SOURCES.md#s16
[S20]: ../SOURCES.md#s20
[S21]: ../SOURCES.md#s21
[S22]: ../SOURCES.md#s22
[S23]: ../SOURCES.md#s23
[S24]: ../SOURCES.md#s24
[S25]: ../SOURCES.md#s25
[S26]: ../SOURCES.md#s26
[S27]: ../SOURCES.md#s27
[S29]: ../SOURCES.md#s29
[S33]: ../SOURCES.md#s33
[S35]: ../SOURCES.md#s35
[S37]: ../SOURCES.md#s37
[S38]: ../SOURCES.md#s38
[S40]: ../SOURCES.md#s40
[S52]: ../SOURCES.md#s52
[S54]: ../SOURCES.md#s54
[S71]: ../SOURCES.md#s71
[R1]: ../SOURCES.md#r1
[R4]: ../SOURCES.md#r4
[R8]: ../SOURCES.md#r8
[R10]: ../SOURCES.md#r10
[R11]: ../SOURCES.md#r11
