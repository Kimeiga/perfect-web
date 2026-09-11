# Evidence registry

Reviewed 2026-09-10. These are evidence anchors, not a claim that every linked document was read end to end. Incident accounts, normative/specification examples, implementation documentation, and paper abstracts are distinguished below. A source supports only the bounded takeaway stated here. Proposed Pleris treatments are analysis, not claims made by the source. No exploit code is needed for this census.

The corpus uses synthetic counterexamples when there is no checked historical incident for the exact invariant. Those entries are labeled **model/spec** rather than made-up incidents. Current product patches and complete browser compatibility matrices are not asserted.

<a id="s01"></a>

## S01: Cloudflare dashboard/API incident, September 12, 2025

**Kind:** incident. **Source:** [Cloudflare dashboard/API incident, September 12, 2025](https://blog.cloudflare.com/deep-dive-into-cloudflares-sept-12-dashboard-and-api-outage/).

A dashboard dependency-identity bug triggered unnecessary calls during a Tenant Service deployment. Service overload and recovery problems amplified the impact. A pure UI dependency model addresses the trigger, not every amplification mechanism.

<a id="s02"></a>

## S02: Cloudflare outage, July 2, 2019

**Kind:** incident. **Source:** [Cloudflare outage, July 2, 2019](https://blog.cloudflare.com/details-of-the-cloudflare-outage-on-july-2-2019/).

A WAF regular expression exhausted CPU. Global configuration distribution increased the blast radius. Bound computation and stage rollout independently.

<a id="s03"></a>

## S03: GitLab database outage, January 31, 2017

**Kind:** incident. **Source:** [GitLab database outage, January 31, 2017](https://about.gitlab.com/blog/postmortem-of-database-outage-of-january-31/).

Accidental primary data removal became prolonged unavailability and data loss amid ineffective recovery mechanisms. Backup execution, alert delivery, and restoration are distinct obligations.

<a id="s04"></a>

## S04: GitHub October 21, 2018 post-incident analysis

**Kind:** incident. **Source:** [GitHub October 21, 2018 post-incident analysis](https://github.blog/news-insights/company-news/oct21-post-incident-analysis/).

A brief network partition caused database failover and divergent writes. An orchestration-supported topology was not necessarily compatible with application assumptions.

<a id="s05"></a>

## S05: Fastly June 8, 2021 outage

**Kind:** incident. **Source:** [Fastly June 8, 2021 outage](https://www.fastly.com/blog/summary-of-june-8-outage).

A valid customer configuration triggered a previously undiscovered software bug. The public summary does not establish a more specific internal defect.

<a id="s06"></a>

## S06: Meta October 4, 2021 outage details

**Kind:** incident. **Source:** [Meta October 4, 2021 outage details](https://engineering.fb.com/2021/10/05/networking-traffic/outage-details/).

A backbone command and a failed audit safeguard disconnected sites. DNS withdrawal and unavailable response tools complicated recovery. Independent recovery authority matters.

<a id="s07"></a>

## S07: Slack May 12, 2020 incident

**Kind:** incident. **Source:** [Slack May 12, 2020 incident](https://slack.engineering/a-terrible-horrible-no-good-very-bad-day-at-slack/).

A configuration-triggered database performance problem increased web-worker utilization and fleet size. The postmortem describes the separate load-balancer and service-discovery integration. This review read its opening technical account, not every later causal detail.

<a id="s08"></a>

## S08: npm left-pad incident, March 2016

**Kind:** incident. **Source:** [npm left-pad incident, March 2016](https://blog.npmjs.org/post/141577284765/kik-left-pad-and-npm).

Unpublishing a transitive dependency broke builds requesting the removed version. Stable identity alone does not ensure future artifact availability.

<a id="s09"></a>

## S09: npm event-stream incident, 2018

**Kind:** incident. **Source:** [npm event-stream incident, 2018](https://blog.npmjs.org/post/180565383195/details-about-the-event-stream-incident).

A malicious dependency introduced through maintainer transfer modified targeted release builds. Build-time and runtime authority must both be considered.

<a id="s10"></a>

## S10: Next.js middleware authorization bypass postmortem, 2025

**Kind:** advisory. **Source:** [Next.js middleware authorization bypass postmortem, 2025](https://nextjs.org/blog/cve-2025-29927).

CVE-2025-29927 involved an internal middleware header crossing a trust boundary. Impact depended on deployment; the postmortem distinguishes self-hosted deployments from unaffected hosted providers. This is not current patch guidance.

<a id="s11"></a>

## S11: React Server Components decoding vulnerability, December 2025

**Kind:** advisory. **Source:** [React Server Components decoding vulnerability, December 2025](https://react.dev/blog/2025/12/03/critical-security-vulnerability-in-react-server-components).

CVE-2025-55182 enabled unauthenticated remote code execution through a payload-decoding flaw. A generated server endpoint is an attack surface even when ordinary source code appears typed. Historical advisory, not a complete current advisory inventory.

<a id="s12"></a>

## S12: HTTP desync/request-smuggling research

**Kind:** security research. **Source:** [HTTP desync/request-smuggling research](https://portswigger.net/research/http-desync-attacks-request-smuggling-reborn).

Primary research on inconsistent HTTP request interpretation across intermediaries. Pleris can constrain generated infrastructure, not prove arbitrary third-party proxy parsers correct.

<a id="s13"></a>

## S13: AWS: making retries safe with idempotent APIs

**Kind:** engineering. **Source:** [AWS: making retries safe with idempotent APIs](https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/).

Caller intent identifiers differ from hashes of equal arguments. Idempotency requires a defined scope, atomic recording, late-request handling, and treatment of the same identifier with different intent.

<a id="s14"></a>

## S14: Stripe idempotent requests

**Kind:** provider contract. **Source:** [Stripe idempotent requests](https://docs.stripe.com/api/idempotent_requests).

Documents provider-side idempotency behavior. Adapters must bind to a tested provider contract rather than claim arbitrary external effects are exactly once.

<a id="s15"></a>

## S15: Stripe webhooks

**Kind:** provider contract. **Source:** [Stripe webhooks](https://docs.stripe.com/webhooks).

Delivery may be duplicated or out of order; event versions and verification matter. A provider callback is not proof of the current state until the application validates its contract and relevance.

<a id="s16"></a>

## S16: PostgreSQL transaction isolation

**Kind:** database contract. **Source:** [PostgreSQL transaction isolation](https://www.postgresql.org/docs/current/transaction-iso.html).

Isolation levels permit different anomalies. Serializable execution can abort and require retry. A local transaction does not roll back an external payment or message already sent.

<a id="s17"></a>

## S17: RFC 9111: HTTP caching

**Kind:** standard. **Source:** [RFC 9111: HTTP caching](https://www.rfc-editor.org/rfc/rfc9111.html).

Cache selection, freshness, validation, and authorization restrictions are separate concerns. Correct reuse depends on every response-varying input, not merely the URL.

<a id="s18"></a>

## S18: Zanzibar, 2019

**Kind:** paper abstract. **Source:** [Zanzibar, 2019](https://research.google/pubs/zanzibar-googles-consistent-global-authorization-system/).

Authorization decisions respect causal ordering of ACL and content changes. The lesson is authority freshness and consistency, not importing a global ACL service wholesale.

<a id="s19"></a>

## S19: Macaroons, 2014

**Kind:** paper abstract. **Source:** [Macaroons, 2014](https://research.google/pubs/macaroons-cookies-with-contextual-caveats-for-decentralized-authorization-in-the-cloud/).

Delegation can attenuate credentials with contextual caveats. This motivates narrowing authority without requiring this exact credential format.

<a id="s20"></a>

## S20: Jif information-flow language

**Kind:** language documentation. **Source:** [Jif information-flow language](https://www.cs.cornell.edu/jif/).

Tracks confidentiality and integrity policies, with static and runtime mechanisms and selective downgrading. Shape validation, confidentiality, and trusted provenance are different properties.

<a id="s21"></a>

## S21: OWASP XSS prevention

**Kind:** security guidance. **Source:** [OWASP XSS prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html).

Output contexts require different treatment. Safe text sinks, validated URLs, and audited HTML sanitization are not interchangeable.

<a id="s22"></a>

## S22: OWASP SSRF prevention

**Kind:** security guidance. **Source:** [OWASP SSRF prevention](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html).

URL parsing, schemes, redirects, and network destinations require layered controls. User-selected arbitrary destinations and fixed service allowlists are legitimate but different products.

<a id="s23"></a>

## S23: OWASP session management

**Kind:** security guidance. **Source:** [OWASP session management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).

Session identity connects authentication to requests and access control. Fixation, credential exposure, and lifecycle mistakes remain distinct from type correctness.

<a id="s24"></a>

## S24: OWASP authorization

**Kind:** security guidance. **Source:** [OWASP authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html).

Authentication does not imply authorization. Resource, action, and principal scope must be enforced at the authority that serves or changes data.

<a id="s25"></a>

## S25: OWASP logging

**Kind:** security guidance. **Source:** [OWASP logging](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html).

Logs have purposes, trust boundaries, and disclosure risks. Security audit records and product analytics need not have identical contents or retention.

<a id="s26"></a>

## S26: CSP Level 3

**Kind:** editors draft. **Source:** [CSP Level 3](https://w3c.github.io/webappsec-csp/).

CSP constrains document execution and resource loading as defense in depth. Generated policies must match actual artifacts and supported browser behavior; this draft is not universal implementation evidence.

<a id="s27"></a>

## S27: Trusted Types

**Kind:** specification. **Source:** [Trusted Types](https://www.w3.org/TR/trusted-types/).

Constrains values entering injection sinks through explicit policies. A policy is not automatically a correct sanitizer, and compatibility must be tested on target browsers.

<a id="s28"></a>

## S28: OAuth security BCP, RFC 9700

**Kind:** standard. **Source:** [OAuth security BCP, RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html).

The threat model includes dynamic relationships among clients, authorization servers, and resource servers. Protocol identity binding belongs in audited authentication adapters.

<a id="s29"></a>

## S29: SES / Hardened JavaScript

**Kind:** implementation documentation. **Source:** [SES / Hardened JavaScript](https://github.com/endojs/endo/tree/master/packages/ses).

Compartments with controlled endowments and frozen intrinsics constrain authority. The initial realm remains powerful; shared-agent CPU and memory exhaustion are explicitly outside compartment isolation.

<a id="s30"></a>

## S30: SLSA threats and mitigations, v1.1

**Kind:** supply-chain threat model. **Source:** [SLSA threats and mitigations, v1.1](https://slsa.dev/spec/v1.1/threats).

Provenance, input closure, cache poisoning, dependency selection, and review context are separate threats. Valid provenance does not prove benign intent or availability.

<a id="s31"></a>

## S31: Bazel hermeticity

**Kind:** build documentation. **Source:** [Bazel hermeticity](https://bazel.build/basics/hermeticity).

Toolchains, environment, and declared inputs determine reproducibility. Untracked host state breaks cache correctness and repeatable builds.

<a id="s32"></a>

## S32: FoundationDB simulation and testing

**Kind:** engineering. **Source:** [FoundationDB simulation and testing](https://apple.github.io/foundationdb/testing.html).

Deterministic simulation explores cluster failures reproducibly and complements live performance and hardware tests. Simulation is not a substitute for every browser or operating-system behavior.

<a id="s33"></a>

## S33: Google SRE: handling overload

**Kind:** engineering. **Source:** [Google SRE: handling overload](https://sre.google/sre-book/handling-overload/).

Request count is not a complete measure of resource cost. Capacity, per-customer limits, degraded responses, and resource exhaustion need explicit handling.

<a id="s34"></a>

## S34: Google SRE: cascading failures

**Kind:** engineering. **Source:** [Google SRE: cascading failures](https://sre.google/sre-book/addressing-cascading-failures/).

Overload and failover can form positive-feedback loops. Isolated retry or autoscaling decisions do not establish whole-system stability.

<a id="s35"></a>

## S35: Invariant confluence / coordination avoidance

**Kind:** paper. **Source:** [Invariant confluence / coordination avoidance](https://arxiv.org/abs/1402.2237).

Under the paper's model, invariant confluence characterizes coordination-free preservation of declared invariants. It does not decide arbitrary business predicates or eliminate partitions.

<a id="s36"></a>

## S36: Keeping CALM

**Kind:** paper. **Source:** [Keeping CALM](https://arxiv.org/abs/1901.01930).

Monotonicity gives a constructive coordination-avoidance criterion under a formal model. Restrict any compiler analysis to a supported fragment.

<a id="s37"></a>

## S37: Temporal Workflow Execution

**Kind:** runtime documentation. **Source:** [Temporal Workflow Execution](https://docs.temporal.io/workflow-execution).

Workflow replay checks emitted commands against durable history. External activities, workflow code compatibility, and recovery remain explicit boundaries.

<a id="s38"></a>

## S38: Local-first software, 2019

**Kind:** research essay. **Source:** [Local-first software, 2019](https://www.inkandswitch.com/essay/local-first/).

Offline operation, collaboration, user ownership, and preservation are related goals, not one guarantee automatically provided by a replicated data type.

<a id="s39"></a>

## S39: Automerge conflicts

**Kind:** runtime documentation. **Source:** [Automerge conflicts](https://automerge.org/docs/reference/documents/conflicts/).

Replicas select deterministic visible values while retaining conflicting assignments. Convergence alone does not choose the product's intended resolution.

<a id="s40"></a>

## S40: Yjs UndoManager

**Kind:** runtime documentation. **Source:** [Yjs UndoManager](https://docs.yjs.dev/api/undo-manager).

Selective undo can track transaction origins and associated cursor metadata. This supplies prior art for undoing one actor's intent without restoring an entire shared document.

<a id="s41"></a>

## S41: Back/forward cache

**Kind:** browser engineering. **Source:** [Back/forward cache](https://web.dev/articles/bfcache).

A restored page can retain its heap and paused work. HTTP freshness and bfcache lifecycle are distinct. Browser-specific eligibility should be measured, not inferred from a framework.

<a id="s42"></a>

## S42: Chrome Page Lifecycle API

**Kind:** browser engineering. **Source:** [Chrome Page Lifecycle API](https://developer.chrome.com/docs/web-platform/page-lifecycle-api).

Pages can freeze or be discarded, including termination without a final callback. Chromium-specific hooks are not the cross-browser baseline.

<a id="s43"></a>

## S43: Input Events Level 2

**Kind:** draft standard. **Source:** [Input Events Level 2](https://w3c.github.io/input-events/).

Editing intentions, composition, paste, replacement, and undo have distinct event semantics. Some composition-related input cannot be canceled. Draft algorithms do not prove cross-browser conformance.

<a id="s44"></a>

## S44: WCAG 2.2

**Kind:** accessibility standard. **Source:** [WCAG 2.2](https://www.w3.org/TR/WCAG22/).

Accessibility combines structural and behavioral criteria with human evaluation. Automated checks cannot establish every user need or whole-application conformance.

<a id="s45"></a>

## S45: WAI-ARIA modal dialog pattern

**Kind:** accessibility pattern. **Source:** [WAI-ARIA modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/).

Modal behavior, focus placement, inert background, and meaningful return focus must agree. Appropriate initial focus depends on the interaction and content.

<a id="s46"></a>

## S46: WebKit storage policy

**Kind:** browser engineering. **Source:** [WebKit storage policy](https://webkit.org/blog/14403/updates-to-storage-policy/).

Best-effort website storage can be evicted and writes can fail. Quota estimates are not guaranteed capacity; local acceptance is not remote durability.

<a id="s47"></a>

## S47: React issue 11538: Google Translate DOM mutations

**Kind:** historical issue. **Source:** [React issue 11538: Google Translate DOM mutations](https://github.com/react/react/issues/11538).

A 2017 report, reproduced in the thread, describes removeChild failures after translation changed DOM structure. This is evidence of a failure class, not a claim that every current version reproduces it.

<a id="s48"></a>

## S48: React issue 8683: IME and controlled inputs

**Kind:** historical issue. **Source:** [React issue 8683: IME and controlled inputs](https://github.com/react/react/issues/8683).

A 2017 report describes composition/change ordering and controlled-input problems across environments. Do not characterize a closed historical issue as a current unfixed regression.

<a id="s49"></a>

## S49: Service worker lifecycle

**Kind:** browser engineering. **Source:** [Service worker lifecycle](https://web.dev/articles/service-worker-lifecycle).

Installation, activation, control, and coexisting clients introduce version coordination obligations. A service worker is not an immortal background process.

<a id="s50"></a>

## S50: WHATWG Streams

**Kind:** living standard. **Source:** [WHATWG Streams](https://streams.spec.whatwg.org/).

Streams model incremental data, queues, and backpressure. A transport chunk is not an application record; a bounded queue needs an overflow or producer-control strategy.

<a id="s51"></a>

## S51: WebRTC specification

**Kind:** standard. **Source:** [WebRTC specification](https://www.w3.org/TR/webrtc/).

Peer connections integrate negotiation and NAT traversal with media/data channels. The overview was reviewed; deep codec, congestion, and device-specific behavior remain undersearched.

<a id="s52"></a>

## S52: Permissions

**Kind:** editors draft. **Source:** [Permissions](https://w3c.github.io/permissions/).

Powerful-feature permission reflects a user-agent decision that can change. An application capability declaration cannot grant browser permission.

<a id="s53"></a>

## S53: HTML user interaction / activation

**Kind:** living standard. **Source:** [HTML user interaction / activation](https://html.spec.whatwg.org/multipage/interaction.html#tracking-user-activation).

Transient activation expires and may be consumed. An asynchronous or replayed handler cannot be assumed to retain a user gesture. Focus, inertness, and find-in-page are also browser-owned behavior.

<a id="s54"></a>

## S54: SvelteKit state management

**Kind:** framework documentation. **Source:** [SvelteKit state management](https://svelte.dev/docs/kit/state-management).

The documented server-global example leaks one user's data to another and loses state across restart. Request scope and durable state are distinct.

<a id="s55"></a>

## S55: Vue watchers

**Kind:** framework documentation. **Source:** [Vue watchers](https://vuejs.org/guide/essentials/watchers.html).

Watch sources and deep/shallow observation differ. Capturing a property value is not the same as observing the property. This review did not audit every watcher implementation.

<a id="s56"></a>

## S56: Angular hydration

**Kind:** framework documentation. **Source:** [Angular hydration](https://angular.dev/guide/hydration).

DOM reuse depends on compatible structure; direct DOM manipulation can interfere. Hydration constraints do not disappear merely because the source language is typed.

<a id="s57"></a>

## S57: Qwik QRL

**Kind:** framework documentation. **Source:** [Qwik QRL](https://qwik.dev/docs/advanced/qrl/).

Lazy handler references combine chunk location, symbol identity, and captured scope. This is prior art for resumption, not evidence of universal speed superiority.

<a id="s58"></a>

## S58: TypeScript type compatibility

**Kind:** language documentation. **Source:** [TypeScript type compatibility](https://www.typescriptlang.org/docs/handbook/type-compatibility.html).

Structural compatibility and deliberate unsound allowances serve JavaScript interop. Pleris should preserve nominal domain identity without forcing every ordinary record to be nominal.

<a id="s59"></a>

## S59: React: You Might Not Need an Effect

**Kind:** framework documentation. **Source:** [React: You Might Not Need an Effect](https://react.dev/learn/you-might-not-need-an-effect).

Derived render values and user-action effects have different causes. Copying a derived value into independently updated state creates avoidable synchronization work.

<a id="s60"></a>

## S60: Layout and layout thrashing

**Kind:** performance engineering. **Source:** [Layout and layout thrashing](https://web.dev/articles/avoid-large-complex-layouts-and-layout-thrashing).

Geometry-affecting writes and subsequent reads can force layout. Batching controls generated work, not every invalidation caused by fonts, external code, or the browser.

<a id="s61"></a>

## S61: Optimize INP

**Kind:** performance engineering. **Source:** [Optimize INP](https://web.dev/articles/optimize-inp).

Interaction latency includes input delay, event processing, and presentation delay. Field and lab observations answer different questions.

<a id="s62"></a>

## S62: Optimize LCP

**Kind:** performance engineering. **Source:** [Optimize LCP](https://web.dev/articles/optimize-lcp).

Discovery, loading, and rendering contribute to largest-contentful-paint timing. The metric does not represent completion of every application task.

<a id="s63"></a>

## S63: Optimize CLS

**Kind:** performance engineering. **Source:** [Optimize CLS](https://web.dev/articles/optimize-cls).

Images, fonts, embeds, and later updates can move visible content. Initial-load-only measurements miss later instability.

<a id="s64"></a>

## S64: Long Animation Frames specification

**Kind:** draft specification. **Source:** [Long Animation Frames specification](https://w3c.github.io/long-animation-frames/).

forcedStyleAndLayoutDuration is a PerformanceScriptTiming attribute, accessed through a frame's scripts. Missing frame-level properties cannot establish lack of implementation support.

<a id="s65"></a>

## S65: Unicode UAX 9

**Kind:** standard. **Source:** [Unicode UAX 9](https://unicode.org/reports/tr9/).

Bidirectional ordering is not equivalent to reversing strings. Text direction and application layout direction require distinct handling.

<a id="s66"></a>

## S66: Unicode UAX 29

**Kind:** standard. **Source:** [Unicode UAX 29](https://unicode.org/reports/tr29/).

Grapheme clusters, code points, and encoded units differ. Text operations must state which unit they use.

<a id="s67"></a>

## S67: Temporal: time zones and ambiguity

**Kind:** conceptual documentation. **Source:** [Temporal: time zones and ambiguity](https://tc39.es/proposal-temporal/docs/ambiguity.html).

Exact time differs from wall-clock time; zone transitions can produce ambiguous or nonexistent local times. The retrieved page had an old proposal-status banner; no current support claim is made.

<a id="s68"></a>

## S68: W3C inline bidirectional markup

**Kind:** internationalization guidance. **Source:** [W3C inline bidirectional markup](https://www.w3.org/International/articles/inline-bidi-markup/).

Unknown-direction embedded text benefits from isolation and appropriate direction handling. This is distinct from selecting an RTL page layout.

<a id="s69"></a>

## S69: CSS Writing Modes Level 4

**Kind:** specification. **Source:** [CSS Writing Modes Level 4](https://www.w3.org/TR/css-writing-modes-4/).

Horizontal, vertical, and bidirectional flows distinguish logical axes from physical coordinates. Retrieved publication status is not a browser-support assertion.

<a id="s70"></a>

## S70: Protocol Buffers evolution

**Kind:** protocol documentation. **Source:** [Protocol Buffers evolution](https://protobuf.dev/programming-guides/proto3/).

Removed field numbers must not be repurposed; JSON/name and binary/tag compatibility differ. Wire compatibility does not establish unchanged business meaning.

<a id="s71"></a>

## S71: RFC 8259: JSON

**Kind:** standard. **Source:** [RFC 8259: JSON](https://www.rfc-editor.org/rfc/rfc8259.html).

Duplicate object names can be interpreted differently by consumers. Numeric and structural decoding must preserve the application's declared meaning.

<a id="s72"></a>

## S72: GraphQL response semantics

**Kind:** protocol documentation. **Source:** [GraphQL response semantics](https://graphql.org/learn/response/).

A response can contain partial data and field errors simultaneously. An HTTP success or decoded outer shape does not mean every requested operation succeeded.

<a id="s73"></a>

## S73: gRPC-Web basics

**Kind:** interop documentation. **Source:** [gRPC-Web basics](https://grpc.io/docs/platforms/web/basics/).

The official browser tutorial establishes a distinct browser integration path. This review makes no claim that every native gRPC transport or streaming mode is available in every browser.

<a id="s74"></a>

## S74: IndexedDB transaction lifecycle

**Kind:** standard. **Source:** [IndexedDB transaction lifecycle](https://www.w3.org/TR/IndexedDB/).

Transactions alternate active/inactive states and can auto-commit when requests finish. Holding a language-level transaction reference does not keep its browser transaction active.

<a id="s75"></a>

## S75: rust-analyzer architecture

**Kind:** tooling documentation. **Source:** [rust-analyzer architecture](https://rust-analyzer.github.io/book/contributing/architecture.html).

Input facts and on-demand derived semantic state are separated. Error-tolerant syntax can support editor features without admitting invalid code for deployment.

<a id="s76"></a>

## S76: Do Users Write More Insecure Code with AI Assistants?

**Kind:** study abstract. **Source:** [Do Users Write More Insecure Code with AI Assistants?](https://arxiv.org/abs/2211.03622).

A study using an older Codex-based assistant found worse security outcomes and overconfidence in its tested tasks. It does not establish error rates or rankings for 2026 coding agents.

## Repository evidence at the audited revision

All relative repository references in the census describe commit `0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45`, not whatever later happens to be at `master`. Base tree: `4b2e50395750d97cb1ec9be2eda04191ae5d9b05`.

<a id="r1"></a>

### R1

[PROJECT_CHARTER.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/PROJECT_CHARTER.md)

Broad intended semantics and milestone requirements, not proof of implementation.

<a id="r2"></a>

### R2

[docs/STATUS.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/STATUS.md)

Current E9 reopening and E10 component-lowering progress; recorded evidence was not rerun in this research session.

<a id="r3"></a>

### R3

[docs/NEXT.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/NEXT.md)

Required resolved-type, argument/return checking, contract, ABI, and compiled-execution sequence. Callable generic syntax does not yet support the generic test example.

<a id="r4"></a>

### R4

[docs/MILESTONES.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/MILESTONES.md)

Scoped E7 browser and E8 host evidence; E9 reopening. Historical milestone headings are not sufficient proof.

<a id="r5"></a>

### R5

[docs/SEMANTICS.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/SEMANTICS.md)

Contains an earlier no-compiler snapshot and layout-spike observations; some present-tense summaries are stale.

<a id="r6"></a>

### R6

[docs/ARCHITECTURE.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/ARCHITECTURE.md)

Earlier architecture snapshot. Must not override current status or type-repair evidence.

<a id="r7"></a>

### R7

[docs/RISK_REGISTER.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/RISK_REGISTER.md)

Existing risks including layout instrumentation and runtime/toolchain limitations.

<a id="r8"></a>

### R8

[docs/RISK_QUEUE.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/RISK_QUEUE.md)

Existing false-green witnesses, causal diagnostics, cursor and resource-identity distinctions.

<a id="r9"></a>

### R9

[docs/DECISIONS.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/DECISIONS.md)

Accepted decision index; proposals in this census do not silently amend accepted ADRs.

<a id="r10"></a>

### R10

[docs/DECISIONS/ADR-0025-an-optimistic-clause-targets-a-resource-entry.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/DECISIONS/ADR-0025-an-optimistic-clause-targets-a-resource-entry.md)

Pure optimistic transform and no handwritten inverse; overlapping-success/failure restoration needs a stronger protocol contract.

<a id="r11"></a>

### R11

[web-recompiled/whitepaper.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/web-recompiled/whitepaper.md)

Design draft already covers much of caching, compatibility, privacy, offline policy, interop, and source-level diagnostics.

<a id="r12"></a>

### R12

[web-recompiled/proof-roadmap.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/web-recompiled/proof-roadmap.md)

Existing bug museum and proof strategy; this census extends rather than replaces it.

<a id="r13"></a>

### R13

[docs/research/technology-matrix.md](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/docs/research/technology-matrix.md)

Baseline for prior-art additions. Absence here is not proof of absence from the entire repository.

<a id="r14"></a>

### R14

[spikes/layout-phase-scheduler/public/thrash.html](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/spikes/layout-phase-scheduler/public/thrash.html)

Reads e.forcedStyleAndLayoutDuration from a frame entry, rather than script attribution records.

<a id="r15"></a>

### R15

[spikes/layout-phase-scheduler/public/phased.html](https://github.com/Kimeiga/perfect-web/blob/0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45/spikes/layout-phase-scheduler/public/phased.html)

The full file shows the same frame-level property lookup at the audited base revision; both probes require correction and remeasurement.

## Retrieval limitations

An inaccessible WebKit security issue, failed legacy PDF/document URLs, empty search results, and unavailable clone/network access are not counted as reviewed evidence. The initial research pass did not execute these systems. A subsequent targeted instrumentation patch has unit and inline-browser evidence in IMPLEMENTATION.md; the compiler suite, database fault runs, and a complete dependency vulnerability scan were not performed. The requested core documents were inspected, including the full charter and whitepaper; long historical status/risk logs and most implementation files were sampled rather than audited exhaustively.
