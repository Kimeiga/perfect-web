## Q. Page lifecycle, local storage, and offline operation

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| Q01 | Background freeze breaks timer-based correctness | Chrome lifecycle model | Correctness must not require timely execution of a suspended page's timers. | Durable state; visibility-aware scheduling | specified but unproven | D+F | Recompute from authoritative time/state on resume; timers are wakeup hints, not proof that a deadline was enforced. | R+U | [S42] [S67] [R1] |
| Q02 | Restored page retains obsolete authority | bfcache heap restoration | Resumed private state must be reconciled with the current authority and version contract. | Restore hooks; generation checks | specified but unproven | D+F+H | Revalidate before new protected actions and refresh private state; do not promise control over every browser restoration frame. | R+U | [S41] [S23] [R1] |
| Q03 | Local write acknowledged as globally durable | WebKit storage limits | Device-local acceptance, durable persistence, and synchronized commitment are different outcomes. | Local-first state models | specified but unproven | A+D+H | Expose pending local versus remote-committed state and eviction/recovery policy. P0. | D+R | [S46] [S38] [R1] |
| Q04 | Quota or eviction silently loses required data | WebKit storage policy | Storage failure must be a modeled outcome, not an impossible branch. | Quota-aware adapters; export/recovery | missing | D+F+H | Handle unavailable, full, evicted, and corrupt local state; never guarantee persistence from an estimate. | R+D | [S46] |
| Q05 | Service worker update mixes incompatible versions | Service worker lifecycle examples | Worker, page, cache, and persisted state must form an admissible compatibility set. | Versioned workers; rollout negotiation | specified but unproven | C+D+E | Test old/new tabs and worker activation; do not force takeover without a safe migration or recovery path. | X+U | [S49] [R11] |
| Q06 | Multiple tabs compete as one logical owner | Model/spec: two tabs both flush the same queue | Cross-tab coordination must tolerate duplicate leaders and crash/restart. | Leases; idempotent flushes | missing | D+F | Use browser coordination as an optimization, with durable idempotency and fencing where correctness requires it. | R+X | [S74] [S13] |
| Q07 | Offline edit loses permission before reconnect | Model/spec: revoked member submits queued change | Permission at edit time and permission at commit time are distinct policies. | Reauthorization; signed offline grants | missing | D+H | Declare whether queued intents require current authority, bounded offline grants, or rejection/conflict handling. P0. | D+X | [S18] [S38] |
| Q08 | Local migration blocks behind an old tab | IndexedDB version lifecycle | Shared local storage upgrades must coordinate existing clients and preserve recoverability. | Version-change protocols | missing | D+F+H | Close or negotiate old connections, report blocked upgrades, and test interrupted migration. | R+U | [S74] [S49] |
| Q09 | Cache and offline queue retain another user's data | Model/spec: logout then second account opens same device | Device storage ownership must remain separate from transient current session identity. | Account-scoped stores; explicit retained drafts | specified but unproven | C+D+H | Partition or clear managed data according to policy; preserve intentional multi-account use through explicit authority. | E+D | [S23] [S46] [R1] |

## R. Realtime streams and media resources

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| R01 | Subscription lacks a gap/replay contract | Model/spec: SSE/WebSocket reconnect after retention expires | Reconnect must distinguish complete continuation from an irrecoverable event gap. | Cursors; snapshots; replay logs | specified but unproven | D+H | Negotiate cursor validity and snapshot recovery; connection success is not synchronization success. | R+D | [R1] [S50] |
| R02 | Producer outruns consumer indefinitely | Streams queue model | Stream pressure must be bounded end to end, including browser-facing adapters. | Backpressure; coalescing; shedding | specified but unproven | D+E+H | Select a declared bounded policy for state updates versus lossless business events. | R+D | [S50] [S33] |
| R03 | Transport chunk treated as application message | WHATWG incremental stream model | Framing and decoding must tolerate arbitrary chunk boundaries and partial records. | Incremental decoders | specified but unproven | C+D | Generate framed parsers with size limits and correct cancellation; never parse each network chunk as a complete JSON document. | X | [S50] [S71] |
| R04 | Presence treated as durable domain truth | Model/spec: stale connected indicator authorizes action | Ephemeral liveness hints must not authorize or commit durable transitions. | Leased presence; separate domain state | missing | A+D+H | Type presence as expiring advisory state and use authoritative commands for commitments. | T+D | [S42] [S24] |
| R05 | Media or socket resource outlives its purpose | Model/spec: camera stays active after closing view | Acquisition authority and lifetime must govern all underlying tracks, connections, and callbacks. | Scoped media adapters | specified but unproven | C+D+F | Own and release tracks, streams, peer connections, object URLs, and subscriptions with observable cleanup evidence. | R+X | [S51] [S52] [R1] |
| R06 | Peer negotiation fails or reorders | Model/spec: simultaneous offers or network change | Realtime connection state must expose negotiation and recovery outcomes explicitly. | WebRTC state machines | missing | D+F+H | Wrap protocol state without hiding failure; require a dedicated negotiation/NAT/device test pass before claiming coverage. | X+D | [S51] |
| R07 | Permission check assumed to guarantee media operation | Permissions model | A prior grant query cannot guarantee a later device operation succeeds. | Typed media permission/error outcomes | missing | D+F | Represent denied, changed, unavailable, and failed outcomes; do not retry permission prompts automatically. | U+X | [S52] [S51] |
| R08 | Codec or transport assumed universally available | WebRTC and gRPC-Web integration boundaries | A deployment must have an implemented compatible path for every required client capability. | Capability negotiation; fallback adapters | specified but unproven | B+D+F | Probe actual capabilities and negotiate alternatives; do not require WebTransport, native Wasm Components, or HTTP/3 for correctness. | X+U | [S51] [S73] [R1] |
| R09 | Media playback or capture silently violates user intent | Model/spec: auto-resume after user pauses or leaves | Application recovery must preserve user control over active media and privacy-sensitive capture. | Explicit media state; activation-aware controls | missing | D+H+F | Distinguish user pause/stop from buffering/failure; require renewed action when policy or browser activation requires it. | U+D | [S53] [S52] |

## S. Packages, builds, and compiler trust

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| S01 | Removed dependency makes build unrecoverable | npm left-pad 2016 | A release must retain the complete admitted input closure, not merely names and version constraints. | Immutable mirrors; content-addressed inputs | specified but unproven | E | Record and retain verified transitive inputs for reproducible builds and supported recovery. | X | [S08] [S31] [R1] |
| S02 | Dependency selection resolves to a different authority | SLSA dependency-confusion threat | Package identity includes expected registry/source authority, not only a package name. | Provenance expectations; scoped registries | specified but unproven | B+E | Bind package resolution to approved origin, artifact digest, and compatibility; deny silent registry fallback. | X | [S30] |
| S03 | Build-time dependency modifies trusted output | event-stream 2018 | Build tools and test dependencies must receive only the authority needed for their task. | Sandboxed builds; separated signing | specified but unproven | E | Isolate install/build/test execution, protect credentials and output finalization, and attest actual inputs. | X | [S09] [S30] |
| S04 | Signed or reproducible malware treated as safe | SLSA residual threats | Provenance proves an origin/build claim, not benign behavior or acceptable authority. | Capability review; runtime confinement | missing | B+E+H | Gate authority separately from signature verification and review source/semantic changes. | X+D | [S30] [S29] |
| S05 | Ambient environment changes the build | Bazel hermeticity examples | Every output-affecting input must participate in the build identity. | Hermetic builds | specified but unproven | C+E | Track toolchain, flags, target, environment, generated inputs, locale, and relevant configuration. | V+X | [S31] [R1] |
| S06 | Build cache serves wrong-context artifact | SLSA cache-poisoning threat | A cached result must match complete inputs and come from an admitted writer. | Authenticated content caches | missing | D+E | Bind cache keys and provenance to transitive inputs and trust context; isolate untrusted PR caches. | X+V | [S30] [S31] |
| S07 | Generated code ships forbidden capability | Repo host admission and ABI findings | Emitted artifacts must expose only the resolved admitted import/capability set. | Wasm host admission; artifact inspection | partially covered | C+E+G | Validate actual component imports independently of source manifests; compiled path still requires end-to-end evidence. P0. | E+V | [R2] [R3] [R4] |
| S08 | Compiler or optimizer preserves types but changes meaning | Model/spec: incorrect lowering or effect reordering | Well-typed and valid machine code must still preserve source semantics. | Differential evaluation; translation validation | specified but unproven | G | Compare an executable semantic oracle with generated backends, including faults, numeric boundaries, and ordering. | V | [R1] [R8] |
| S09 | Package singleton duplicated across bundle boundary | Model/spec: two copies carry incompatible runtime identity | Cross-package runtime identity must be explicit where behavior depends on it. | Stable package/type identity; linker analysis | specified but unproven | B+C+D | Detect conflicting runtime instances and derive shared ownership where valid; do not deduplicate different versions blindly. | T+X | [S58] [R1] |
| S10 | Incompatible ABI representations accepted as equivalent | Repo E10 canonical ABI findings | Semantic identity, component types, and flattened core signatures are different layers. | WIT projection; independent validators | partially covered | B+C+E | Keep one semantic authority, derive outward, and validate exact boundary shapes; flat equality is insufficient. P0. | T+V | [R2] [R3] [R8] |

## T. Compatibility, rollout, and long-lived state

| ID | Failure class | Concrete historical example(s) or model/spec | Root invariant | Existing systems addressing it | Pleris status | Strongest prevention layer | Proposed Pleris treatment | DX cost | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| T01 | Wire-compatible change alters domain meaning | Protobuf evolution contract | Compatibility must preserve admitted meaning, not just successful decoding. | Versioned semantic contracts | specified but unproven | B+D+E+H | Compare domain units, defaults, variants, policies, and operation behavior as well as wire layouts. P0. | T+X | [S70] [R11] |
| T02 | Removed field/tag reused for another meaning | Protobuf reserved-field guidance | Retired wire identities must not be reassigned while old data or clients remain admissible. | Reserved identities; schema history | missing | B+E | Retain field/variant identity history and reject unsafe reuse across supported generations. | T | [S70] |
| T03 | Rolling deployment admits incompatible old/new pairs | Model/spec: old browser sends to new origin | Every reachable producer-consumer pairing in a rollout must satisfy its compatibility contract. | Expand/contract rollout; version routing | specified but unproven | B+E+G | Validate browser, edge, origin, persisted state, worker, and job versions together. P0. | X+V | [R11] [S49] [S70] |
| T04 | Rollback restores code but not compatible data | Model/spec: migrated data cannot be read by rollback version | Recovery plans must account for state evolution already committed. | Reversible or forward-compatible migrations | missing | E+G+H | Admit rollback only with compatible state or an explicit recovery/migration plan. P0. | X+D | [S70] [S03] |
| T05 | Durable workflow replay uses incompatible code | Temporal history model | A resumed workflow must interpret prior history under an admitted program version. | Workflow versioning; deterministic replay | missing | B+D+E | Bind workflow history and handlers to compatible code and declared migration; do not silently restart commitments. | X+R | [S37] |
| T06 | Default or nullability change corrupts old meaning | Model/spec: missing field acquires new business default | Old absence and new explicit values must retain distinguishable meanings when required. | Versioned defaults; open variants | specified but unproven | B+C+D | Generate migrations and readers from admitted schema history; require explicit semantic conversion. | T+D | [S70] [S71] [R11] |
| T07 | Feature flag combinations violate assumptions | Fastly valid-config trigger; model of cross-tier flags | Every admitted configuration combination must satisfy the same semantic constraints as source. | Typed configuration; config model tests | missing | B+D+E+G | Check reachable flag/config states, rollout order, and capability changes; do not require enumerating an unbounded flag space. | E+V | [S05] [S07] |
| T08 | Optimization silently changes placement or cost | Model/spec: compiler upgrade moves work to origin | Deployment-affecting changes must remain inside declared privacy, consistency, and budget constraints. | Inspectable placement plans | specified but unproven | B+C+E+H | Produce semantic plan diffs and optional policy locks; optimize only within equivalent admissible plans. | P+D | [R11] [S33] |
| T09 | Artifact identity exists but old assets disappear | Model/spec: retained HTML's content-addressed chunk is deleted | Content addressing needs a retention and revocation policy to ensure usable references. | Artifact retention; recovery negotiation | specified but unproven | E+H | Retain supported generations and publish explicit retirement behavior; hashes do not guarantee availability. | X+D | [S08] [S57] [R11] |


[S03]: ../SOURCES.md#s03
[S05]: ../SOURCES.md#s05
[S07]: ../SOURCES.md#s07
[S08]: ../SOURCES.md#s08
[S09]: ../SOURCES.md#s09
[S13]: ../SOURCES.md#s13
[S18]: ../SOURCES.md#s18
[S23]: ../SOURCES.md#s23
[S24]: ../SOURCES.md#s24
[S29]: ../SOURCES.md#s29
[S30]: ../SOURCES.md#s30
[S31]: ../SOURCES.md#s31
[S33]: ../SOURCES.md#s33
[S37]: ../SOURCES.md#s37
[S38]: ../SOURCES.md#s38
[S41]: ../SOURCES.md#s41
[S42]: ../SOURCES.md#s42
[S46]: ../SOURCES.md#s46
[S49]: ../SOURCES.md#s49
[S50]: ../SOURCES.md#s50
[S51]: ../SOURCES.md#s51
[S52]: ../SOURCES.md#s52
[S53]: ../SOURCES.md#s53
[S57]: ../SOURCES.md#s57
[S58]: ../SOURCES.md#s58
[S67]: ../SOURCES.md#s67
[S70]: ../SOURCES.md#s70
[S71]: ../SOURCES.md#s71
[S73]: ../SOURCES.md#s73
[S74]: ../SOURCES.md#s74
[R1]: ../SOURCES.md#r1
[R2]: ../SOURCES.md#r2
[R3]: ../SOURCES.md#r3
[R4]: ../SOURCES.md#r4
[R8]: ../SOURCES.md#r8
[R11]: ../SOURCES.md#r11
