# The Web, Recompiled
## A Semantic Application Platform for Browser, Edge, and Origin

**Design draft 0.1 — August 2026**  
**Status:** Research proposal and evaluation plan, not a claim of completed implementation.

---

## Abstract

Modern web applications are assembled from a programming language, a UI framework, a meta-framework, transport conventions, caches, server runtimes, databases, deployment rules, and a large body of unwritten architectural knowledge. Many of the properties that determine whether such an application is correct are absent from the program’s static meaning. A compiler may know the shape of a TypeScript object while knowing nothing about whether a render may start a request, whether a response may enter a shared cache, whether a mutation is safe to retry, whether a task outlives its owner, whether a secret can cross into a browser artifact, or which data change invalidates a materialized page.

This document proposes a clean-slate but incrementally deployable web application platform. A single readable source language describes algebraic state, typed effects, capabilities, privacy, consistency, freshness, queries, commands, subscriptions, resources, and semantic HTML. A whole-application compiler transforms that description into browser, edge, and origin artifacts. Static content remains ordinary HTML and CSS. Interactive expressions become stable document parts rather than virtual trees. Server-rendered applications resume rather than replay their initial component computation. Public views can be materialized near users according to declared dependencies; private values are prohibited from influencing shared artifacts. Runtime components begin without ambient authority and receive only compiler-generated capabilities.

The proposal does not claim that compilation can prove product requirements, eliminate distributed-system failure, or make every performance tradeoff universal. Its narrower thesis is that a large and important class of current web failures exists because application meaning is represented only as convention. Moving that meaning into a sound semantic model should reduce invalid implementation choices for both humans and coding agents, while enabling the platform to optimize placement, rendering, caching, and synchronization automatically.

The project is evaluated through a public proof ladder: a corpus of programs that must fail compilation, a reconstruction of a real request-amplification incident pattern, a zero-application-JavaScript static route, privacy-safe cache placement, a hostile-network marketplace, incremental materialization, a multi-role offline/realtime application, an AI coding benchmark, and eventually a browser-native runtime experiment.

---

## 1. The problem is not simply JavaScript

JavaScript has unusual historical semantics, and TypeScript deliberately preserves compatibility rather than providing a fully sound type system. Replacing JavaScript with a stronger language would remove many value-level errors. It would not by itself solve the defining problems of modern web application architecture.

A memory-safe language can still compile an application that:

- starts the same request repeatedly because of UI lifecycle semantics;
- places user-specific data in a globally shared cache;
- retries a payment mutation without an idempotency policy;
- serializes a secret into browser-visible state;
- launches a task that survives after its page is gone;
- fetches several dependent resources in an accidental waterfall;
- invalidates an entire route when one small resource changes;
- performs the initial render on both server and client;
- ships code before the user can possibly need it;
- expresses impossible combinations of loading, error, and success flags;
- handles only some states of a business process;
- presents valid types but the wrong product behavior.

The immediate trigger in Cloudflare’s September 12, 2025 dashboard and API incident was a React effect whose object dependency was recreated, causing unnecessary Tenant Service API calls. Cloudflare’s postmortem also describes the service deployment, retry behavior, authorization dependency, insufficient recovery behavior, and thundering herd that amplified the trigger. The lesson is not that React alone caused a distributed outage. The lesson is that a consequential request-lifecycle rule existed outside the programming language and was represented through referential identity and developer discipline.[^cloudflare]

React’s own documentation describes effects as an escape hatch and repeatedly explains that many effects should be removed or replaced by more specific patterns.[^react-effects] Svelte’s current best-practices documentation similarly calls effects an escape hatch that should mostly be avoided.[^svelte-effects] These warnings are useful, but they reveal a structural problem: a general primitive remains available, while the distinction between query, command, subscription, local derivation, and lifecycle resource is enforced primarily through guidance.

The web-development burden is therefore distributed across several incomplete verifiers:

```text
language type checker
+ framework linter
+ formatter
+ test suite
+ schema validator
+ code reviewer
+ security review
+ performance review
+ deployment policy
+ production monitoring
+ incident response
```

Each is valuable. The proposal is to move every rule that can be stated mechanically into the lowest reliable layer that can enforce it.

```text
review advice
    -> architecture check
        -> linter
            -> API restriction
                -> type/effect property
                    -> unrepresentable state
```

The goal is not a compiler that “knows the product.” The goal is a compiler that stops asking humans and models to remember implementation obligations that are already formal enough to check.

---

## 2. Design goals

### 2.1 One semantic application

Application authors should describe the domain and its policies once. The compiler should derive browser, edge, and origin artifacts, typed interfaces, serialization, loading behavior, cache materialization, and invalidation structure.

### 2.2 Familiar readable source

The ordinary application language should feel closer to Gleam, Elm, and Svelte than to a low-level systems language. Business and UI code should not require explicit lifetime notation. Algebraic data types, exhaustive matching, nominal domain types, immutable values, and explicit failure should be normal.

### 2.3 Render purity

A view describes document structure. It cannot start network requests, read secrets, access the clock, generate randomness, mutate domain state, or spawn detached tasks. External interaction occurs through typed domain primitives.

### 2.4 No generic application effect primitive

The platform distinguishes:

- `derived` — pure local computation;
- `query` — keyed remote read with cache, freshness, cancellation, and consistency;
- `command` — explicit mutation with authorization, idempotency, transaction, and invalidation policy;
- `subscription` — scoped stream of external changes;
- `resource` — acquire/release lifecycle for an imperative handle;
- `task` — structured child computation owned by a scope;
- `unsafe effect` — an audited infrastructure escape hatch.

The type system records capabilities; the framework gives each capability a lifecycle with domain meaning.

### 2.5 Semantic HTML remains the document format

The platform preserves links, forms, headings, labels, tables, native controls, accessibility semantics, search, selection, translation, browser extensions, and progressive fallback. It does not replace the web with an opaque canvas or proprietary cross-platform scene graph.

### 2.6 No conventional hydration requirement

The server should not compute a component tree only for the browser to download and replay it before interaction. The compiler emits semantic HTML, the minimal resumable state needed by interactive parts, and content-addressed handlers loaded when useful. Marko and Qwik demonstrate important parts of this direction.[^marko-fast][^qwik-resume]

### 2.7 Privacy and authority are static program properties

A value can carry a visibility label such as public, session-private, user-private, device-private, or secret. A component or function can access only capabilities granted by its placement and host world. A shared artifact cannot depend on a private value.

### 2.8 Policy, not mechanism

Application code declares freshness, consistency, privacy, invalidation, offline behavior, and fallback. The platform chooses static generation, private server rendering, public materialization, browser execution, edge execution, streaming, prefetching, and code splitting when those choices follow from policy.

### 2.9 Reproducible evidence

Performance and AI claims require public harnesses, raw traces, hardware and network profiles, accepted and rejected programs, and reproducible model rollouts. Marketing numbers are not part of the language specification.

---

## 3. Explicit non-goals

The project does not attempt to:

- prove that business requirements are correct;
- eliminate every outage or distributed race;
- replace SQL databases, QUIC, HTTP, HTML, CSS, or URLs;
- make every application offline-first;
- apply one consistency model to all resources;
- promise deterministic real-time latency under automatic memory management;
- force ownership syntax into ordinary UI code;
- invent a browser engine before validating the application model;
- claim that effects, resumability, reactive signals, capability security, or materialized views are individually new;
- require a full rewrite before adoption.

A clean design that cannot interoperate with existing applications is unlikely to influence the web.

---

## 4. Core language

### 4.1 Algebraic domain state

Boolean flags and optional fields permit invalid combinations. Domain states should be represented as explicit variants.

```text
type OrderState =
    | Draft(DraftOrder)
    | Pricing(PricingRequest)
    | AuthorizingPayment(PaymentAttempt)
    | Submitting(PreparedOrder)
    | SearchingForCourier(PlacedOrder)
    | InProgress(ActiveDelivery)
    | Delivered(Receipt)
    | Cancelled(CancellationReason)
    | Failed(OrderFailure)
```

A function that renders or transforms an `OrderState` must handle the variants relevant to its type. A new state produces compiler errors at every non-exhaustive decision point.

### 4.2 Nominal domain types

```text
StoreId
ConsumerId
CourierId
OrderId
Money<USD>
Distance<Meters>
PositiveInt
InteractionId
EmailAddress
```

These types prevent accidental interchange even when their runtime representation is identical.

### 4.3 Explicit absence and failure

```text
Option<Address>
Result<Order, PlaceOrderError>
```

External boundaries cannot assert that unknown data is a trusted domain value. They must decode or validate it.

### 4.4 Effects in function types

The language tracks externally observable capabilities in function types, following the broad direction demonstrated by Koka’s effect rows and handlers.[^koka]

```text
subtotal : Cart -> Money<USD>

load_store :
    StoreId
    -> <database.read<Stores>, trace>
       Result<Store, StoreError>

request_location :
    Permission<Location>
    -> <device.location, ui.permission_prompt>
       Result<Location, PermissionError>
```

A pure function has an empty effect row. Higher-order functions are effect polymorphic, so a pure `map` remains pure when given a pure function and reflects effects when given an effectful function.

### 4.5 Capabilities and host worlds

Static effects are interpreted by runtime capabilities. A component’s external interface is generated as a typed world. The WebAssembly Component Model’s WIT language already defines language-neutral records, variants, options, results, resources, interfaces, and worlds.[^wit] WASI applications begin without ambient authority and can only use capabilities explicitly provided by a host.[^wasi]

Conceptually:

```text
effect database.read<Stores>
    -> import stores.read

effect payments.secret
    -> import secret-store.open("payments")

effect outbound.http<eta-service>
    -> import eta-http.send
```

A browser artifact has no database or secret-store import. An edge artifact may have a public store-read interface but no payment capability. An origin command can receive the narrow interfaces required for its function.

### 4.6 Automatic memory with affine resources

Ordinary immutable values use automatic memory management. The implementation may combine stack allocation, escape analysis, reference counting, regions, arenas, and tracing fallback. Koka’s Perceus work demonstrates that high-level functional memory can be compiled using optimized reference counting and reuse rather than a conventional tracing collector in every case.[^koka]

Scarce resources can be affine:

```text
DatabaseTransaction
SubscriptionHandle
OpenStream
MapInstance
FileUpload
LockGuard
```

The compiler requires each resource to be consumed or closed according to its protocol. This gives Rust-like discipline where it represents a real lifecycle, without imposing ownership notation on every string and record.

### 4.7 Structured concurrency

Every task belongs to a scope:

- interaction;
- component;
- route;
- request;
- session;
- durable job.

A child task cannot silently outlive its owner. Leaving a scope cancels or joins its children according to declared policy. Creating a durable background operation requires an explicit durable-job capability.

---

## 5. Application resources

### 5.1 Local state

Local state covers ephemeral UI facts such as a menu being open or the current value of an unfinished input. It is not used as a generic container for remote data, business processes, or replicated state.

```text
state menu_open : Bool = false
state draft_note : String = ""
```

### 5.2 Derived values

Derived values are pure and automatically recomputed from their dependencies.

```text
derived subtotal = cart.items.sum(by: _.price * _.quantity)
```

They cannot perform I/O or mutate state.

### 5.3 Queries

A query is a keyed remote read with declared policy.

```text
public query store(id: StoreId) {
    freshness 30s
    consistency snapshot
    invalidates StoreChanged(id)
    fallback last_known_good

    Stores.get(id)
}
```

The compiler and runtime know:

- its key;
- its visibility;
- effects and capabilities;
- acceptable staleness;
- consistency requirement;
- invalidation events;
- fallback policy;
- placement constraints;
- serialization requirements;
- owning scope.

Repeated reads of the same logical key can be deduplicated. A changed key cancels irrelevant work. A query is not started because a newly allocated object receives a different identity.

### 5.4 Commands

A command is an explicit state-changing operation.

```text
command add_to_cart(item: MenuItemId, quantity: PositiveInt) {
    requires SignedIn
    idempotent_by InteractionId
    optimistic cart.add(item, quantity)
    transaction required
    emits CartChanged(current_session)

    Carts.add(current_consumer, item, quantity)
}
```

A command can declare:

- authorization;
- idempotency key;
- optimistic transition;
- transaction boundary;
- retry admissibility;
- emitted invalidation events;
- compensating behavior;
- conflict policy;
- placement.

The system does not promise magical exactly-once delivery. It can enforce that repeated delivery of one interaction identifier produces at most one intended business effect when the backing system and transaction design support that contract.

### 5.5 Subscriptions

A subscription is a scoped stream:

```text
subscription order_updates(order: OrderId) {
    authorization CanViewOrder(order)
    reconnect exponential(max: 30s)
    resume_from cursor
}
```

The owner scope controls cancellation and reconnection. Incoming values are decoded and associated with explicit ordering semantics.

### 5.6 Resources

An imperative library such as a map, editor, media session, or WebRTC connection is represented as a resource with acquisition and cleanup.

```text
resource map = Maps.mount(node, options)
```

The compiler knows that `map` owns a handle and ensures the release path exists. There is no generic callback whose cleanup must be remembered through convention.

### 5.7 Replicated and offline state

Offline behavior is opt-in because conflict semantics are domain-specific.

```text
offline command courier_arrived(order: OrderId) {
    queue when_disconnected
    conflict server_validates_transition
    expires 10m
}
```

Some values can use mergeable structures; others require server arbitration. The platform requires the policy rather than silently choosing last-write-wins.

---

## 6. Privacy and information flow

### 6.1 Labels

Illustrative labels include:

```text
Public
Private<Session>
Private<UserId>
Private<Device>
Secret<Payments>
Internal<Operations>
```

Labels form a policy lattice. A value can flow only into an equal or more restricted context unless an explicit declassification function is authorized.

### 6.2 Public artifacts

A shared public materialization cannot depend on:

- session state;
- user identity;
- private cart data;
- secrets;
- unrestricted current time or randomness;
- device-local values.

If a page combines a public menu and private cart, the compiler can split it into a public document and private slot, make the entire result private, or request a deliberate audited escape hatch.

### 6.3 Serialization

The compiler derives serialization only for values allowed to cross a boundary. A server secret, open transaction, host resource, or non-serializable closure cannot be captured by a browser handler.

### 6.4 Runtime defense

Static checking is not the sole security boundary. Generated components run in capability-based sandboxes. If an artifact has no secret-store import, it cannot access the store even if it contains malicious code or the compiler emitted an incorrect call path. WIT worlds make imports and exports explicit, and WASI’s model begins without ambient authority.[^component-worlds][^wasi]

---

## 7. Placement across browser, edge, and origin

### 7.1 Placement as constraint solving

A computation’s legal placements follow from:

- required capabilities;
- privacy label;
- data residency;
- consistency;
- latency preference;
- deterministic/cacheable behavior;
- code-size and compute budgets;
- device-only APIs;
- origin-only secrets;
- user-declared placement constraints.

Examples:

```text
Public + deterministic + build-known
    -> static artifact

Public + mutable + stale-tolerant
    -> edge materialization

Private<Session> + read-your-writes
    -> private edge/origin execution

Secret<Payments>
    -> origin only

Private<Device> + location capability
    -> browser only
```

The compiler explains its decision. A developer may constrain placement when the policy is underdetermined, but does not manually duplicate APIs and client wrappers.

### 7.2 Generated interfaces

Browser, edge, and origin artifacts communicate through generated typed interfaces. SvelteKit’s remote functions are a useful present-day step: `query`, `form`, `command`, and `prerender` functions run on the server and are transformed into client wrappers.[^svelte-remote] The proposal extends this idea into a sound language where the operations also carry effects, privacy, consistency, invalidation, and capability semantics.

### 7.3 Failure domains

Placement does not erase failure. The application model must represent:

- network unavailable;
- stale data served;
- origin unavailable;
- authorization failure;
- command uncertain;
- materialization failed;
- subscription disconnected;
- client version incompatible with resumable state.

These are algebraic states and policies rather than implicit promise rejections.

---

## 8. Rendering model

### 8.1 Source structure

The authoring unit is a Svelte-like single-file component containing optional script, semantic HTML template, and scoped style section.

```svelte
<script lang="application">
public query store(id: StoreId) { ... }
private query cart() { ... }
command add_to_cart(item: MenuItemId) { ... }
</script>

<main>
    <h1>{store.name}</h1>
    <ul aria-label="Menu">
        {#each store.items as item (item.id)}
            <li>
                <span>{item.name}</span>
                <Money value={item.price} />
                <button
                    disabled={!item.available}
                    on:press={() => add_to_cart(item.id)}
                >
                    Add
                </button>
            </li>
        {/each}
    </ul>
</main>

<style scoped>
main { padding: token(space.page); }
</style>
```

The exact syntax is provisional. The semantic distinction between state, resource, and effect is not.

### 8.2 Static HTML and document parts

The compiler separates:

```text
StaticNode
TextPart
AttributePart
RangePart
KeyedListPart
EventPart
```

An expression such as `cart.total` maps directly to a text part. An availability value may map to a disabled attribute and label text. A keyed list maps stable domain identity to a document range.

When a value changes, only dependent parts update. There is no need to rerun a complete component function and reconcile two virtual trees.

### 8.3 Fine-grained client inclusion

Marko 6 demonstrates fine-grained bundling in which static content can require no client JavaScript even inside a template containing interactive content, and server work is not automatically replayed in the browser.[^marko-fast] This proposal uses the same broad principle but derives the client boundary from a sound application language.

### 8.4 Resumption

Qwik describes resumability as pausing execution on the server and resuming in the browser without replaying the application’s initial component logic.[^qwik-resume] The proposed compiler emits:

- semantic HTML;
- stable identifiers for interactive parts;
- serialized allowed captures;
- content-addressed handler identities;
- reactive dependency metadata when needed.

The browser can register one delegated event path and retrieve a handler on intent or interaction. Static content has no application runtime entry.

### 8.5 Streaming and patches

Independent document regions can stream as their data resolves. A compatibility runtime can apply template patches. The WICG declarative partial-updates proposal explores script-free `<template>` patches and out-of-order HTML streaming.[^partial-updates]

The long-term browser primitive should support named document ranges, declarative patching, and typed state association without framework-specific inline scripts.

### 8.6 Styling

The style language remains CSS-like because the web requires inheritance, logical directions, user overrides, media and container queries, print, accessibility preferences, and thematic cascade.

Additions include:

- typed design tokens;
- scoped rules by default;
- explicit global layers;
- unit-aware values;
- typed custom properties;
- dead-rule detection;
- design-system validation;
- generated critical-style extraction.

Utility syntax can be compiler sugar, not a separate runtime styling architecture.

---

## 9. Materialization, caching, and invalidation

### 9.1 From route regeneration to dependency maintenance

Traditional incremental static regeneration commonly associates freshness or invalidation with a route. The proposal treats public rendered results as materialized views with explicit dependencies.

```text
StorePage(store_12)
    <- Store(store_12)
    <- Menu(store_12)
    <- PromotionSet(store_12)
```

A `MenuChanged(store_12)` event invalidates only affected resources and document fragments.

### 9.2 Initial explicit model

The first implementation should require declared events:

```text
command update_menu(...)
    emits MenuChanged(store_id)

query menu(store_id)
    invalidates_on MenuChanged(store_id)
```

This is inspectable and deterministic.

### 9.3 Later automatic dependency tracking

A deterministic query executor can record logical resources, rows, or indexes read and derive invalidation dependencies. Related systems provide useful design evidence:

- Skip connects tracked effects to safe memoization and reactive invalidation.[^skip]
- Convex tracks query dependencies and updates subscribed clients as source data changes.[^convex]
- Materialize incrementally maintains query results rather than recomputing complete snapshots.[^materialize]

The platform should borrow principles without requiring a custom database for the first release.

### 9.4 Cache safety

The compiler checks:

- public/private compatibility;
- deterministic inputs;
- clock/random dependence;
- consistency policy;
- allowed staleness;
- failure fallback;
- invalidation completeness or declared time fallback.

### 9.5 Last-known-good behavior

A public result can declare that a failed regeneration continues serving a previous valid materialization. Private or transactional data may require an explicit error instead. The policy is visible at the resource declaration.

---

## 10. Network mapping

### 10.1 Preserve HTTP semantics

The project should use HTTP and URLs, with HTTP/3/QUIC where available. It does not require a new transport to validate the application model.

### 10.2 Prioritized application streams

A navigation can conceptually deliver:

1. semantic HTML shell;
2. critical styles;
3. public resource state;
4. private resource state;
5. resume manifest;
6. immediately useful handlers;
7. delayed document patches;
8. live invalidation events.

These may initially map to ordinary responses, streaming bodies, and event streams. Future protocol extensions could make prioritization and patching more direct.

### 10.3 Typed state representation

HTML remains the document representation. Typed application state can use schema-generated compact encodings derived from the same interface types used across components. The design avoids repeatedly hand-writing JSON contracts but does not turn every document into an opaque binary scene.

### 10.4 Content-addressed artifacts

Handlers, schemas, and compiled components can be identified by content hash:

```text
handler:add-to-cart@a8f13
schema:Cart@04e77
component:Money@31ce9
```

A browser can retain identical code across pages and versions. Resumable state records the exact artifact version required, making version compatibility explicit.

---

## 11. Runtime and compilation strategy

### 11.1 Compatibility-first implementation

The first deployable version targets current browsers:

- ordinary HTML and CSS;
- a small JavaScript document-parts/resumption runtime;
- generated JavaScript or Wasm for handlers;
- standard HTTP;
- a Rust origin/edge host;
- SQLite locally and PostgreSQL for separated nodes.

Browser changes are postponed until measurements show which compatibility costs matter.

### 11.2 Compiler pipeline

```text
lossless source tree
    -> name resolution
    -> value type inference
    -> effect and capability inference
    -> affine resource checking
    -> privacy/information-flow checking
    -> placement constraint solving
    -> resource dependency graph
    -> document-parts graph
    -> target splitting
        -> browser artifact
        -> edge component
        -> origin component
        -> public materializations
        -> generated interfaces
        -> semantic report
```

### 11.3 Bootstrap path

A pragmatic research path can use existing systems as temporary backends:

- Koka for effect and value semantics;
- Marko as a streamed/resumable renderer reference or generated backend;
- Qwik as a handler-resumption reference;
- WIT/Wasm Components for typed host boundaries;
- Wasmtime for capability-hosted server components.

The permanent semantic compiler should eventually be implemented directly, likely in Rust, so that web-specific effects, privacy, placement, and rendering share one intermediate representation.

### 11.4 Browser-native experiment

Servo is a modular Rust browser engine intended for embedding and customization, making it a reasonable later research host.[^servo] Experiments may include:

- direct typed Wasm access to document parts;
- native resumable-handler registration;
- declarative patch streams;
- browser-enforced component capability manifests;
- content-addressed code caches;
- causal reactive developer tools.

The same source application must run in compatibility and native modes so that browser modifications remain measurable substitutions rather than a separate platform.

---

## 12. Developer tooling and semantic review

### 12.1 Diagnostics

An effect-and-placement compiler is only useful if errors are expressed in application language.

Poor diagnostic:

```text
cannot unify row e1 with {db, session | e2}
```

Target diagnostic:

```text
`StorePage` is shared publicly, but it reads `cart`, which is private to a session.

Safe options:
- render `cart` in a private streamed slot;
- make the whole page private;
- pass a public summary produced by an authorized declassification function.
```

### 12.2 Causal traces

Developer tools should answer:

- Why did this request start?
- Which query key owns it?
- Which interaction caused this command?
- Why was it retried?
- What invalidated this resource?
- Why did this document part update?
- Why was this code placed at the origin?
- Which private value prevented shared caching?
- Which artifact added client bytes?

### 12.3 Semantic diffs

A pull request should include generated changes such as:

```text
Effects added
- database.write<Carts>

Capabilities added
- carts.write to origin component

Privacy changes
- none

Cache changes
- Store freshness: 30s -> 10s

Placement changes
- add_to_cart: edge-capable -> origin-only

Client impact
- +1 lazy handler
- +1.8 KB compressed
- no startup code

State changes
- added OutOfStock transition
```

This makes AI-generated code review about changed meaning rather than uniformly reading every generated line.

---

## 13. AI-assisted development

### 13.1 Why a stronger platform may need a smaller model

A coding model working in React must probabilistically remember lifecycle, referential identity, hook rules, accessibility conventions, performance patterns, data-fetching architecture, and repository-specific style. ReactBench evaluates realistic React changes using both behavior tests and production-oriented React quality checks; the current published leaderboard remains far from complete reliability.[^reactbench]

A constrained language does not eliminate reasoning. It reduces the invalid action space:

- render cannot start a query;
- a private value cannot enter a public cache;
- external data cannot become a domain value without decoding;
- a non-idempotent command cannot receive automatic retries silently;
- a state machine cannot omit a variant;
- a browser artifact cannot import a database;
- a task cannot detach by accident.

Compiler feedback also creates a deterministic repair loop.

### 13.2 Benchmark design

The project should publish realistic tasks in comparable applications:

- React/Next baseline;
- SvelteKit baseline;
- new-platform implementation.

Evaluation includes:

- behavior;
- accessibility;
- effect and capability correctness;
- privacy and placement;
- resource lifecycle;
- performance budget;
- semantic-diff quality;
- human review time.

Report the quality curve over model size, inference cost, tokens, and repair iterations. Do not choose a desired conclusion in advance.

### 13.3 Training data

The most valuable examples are not only finished programs. They are checked trajectories:

```text
requirement
-> attempted implementation
-> compiler rejection
-> repair
-> behavioral counterexample
-> reviewer objection
-> accepted semantic change
```

This teaches the relationship between intent, implementation, and verification.

---

## 14. Evaluation plan

### 14.1 Correctness corpus

Maintain accepted and rejected programs covering:

- effects;
- state exhaustiveness;
- private/public flows;
- serialization;
- capability boundaries;
- task lifetimes;
- affine resources;
- idempotency;
- cache validity;
- invalidation;
- HTML/accessibility relationships;
- form/handler correspondence.

### 14.2 Incident-pattern reproductions

Start with the Cloudflare effect-dependency amplification pattern, then add publicly documented classes of failure involving:

- stale closures;
- duplicated subscriptions;
- shared-cache personalization;
- unbounded retries;
- mutation duplication;
- hydration mismatch;
- accidental server/client secret exposure.

Each reproduction must separate the trigger from the systemic amplifiers.

### 14.3 Performance comparison

Implement one marketplace specification in multiple stacks and measure under identical data and network conditions:

- transferred bytes;
- startup code executed;
- main-thread CPU;
- first useful content;
- first successful interaction;
- interaction latency;
- server CPU;
- query waterfall depth;
- cache regeneration work;
- memory;
- behavior on a real phone.

Use browser traces, Lighthouse where appropriate, WebPageTest or an equivalent reproducible harness, and a Linux `netem` network lab. Publish raw artifacts.

### 14.4 Resilience testing

Inject:

- latency;
- packet loss;
- origin termination;
- delayed dependencies;
- duplicated deliveries;
- disconnect during command;
- stale materialization;
- subscription reconnect;
- incompatible client/server artifact versions.

The expected behavior derives from declared policies.

### 14.5 Security testing

Test:

- capability denial;
- secret access attempts;
- private-to-public flow;
- malicious component imports;
- serializer confusion;
- HTML injection boundaries;
- dependency substitution;
- sandbox escape assumptions.

A runtime security audit remains necessary even with static checking.

### 14.6 Usability

Measure:

- time to first feature;
- concepts authored directly;
- diagnostic comprehension;
- time to repair invalid program;
- experienced developer comparison;
- new developer onboarding;
- ability to predict placement and caching;
- review time.

A language that proves more but is impossible to understand has not achieved the project goal.

---

## 15. Related systems and lessons

This project is an integration thesis, not an assertion that prior work is missing.

### Koka

Provides effect rows, algebraic handlers, algebraic data types, and optimized automatic memory management. It is the strongest bootstrap donor for effect semantics, but not a complete web platform.[^koka]

### Elm and Gleam

Demonstrate readable algebraic application programming, explicit failure, controlled effects, and small language surfaces. They are major syntax and usability donors.

### Links and Ur/Web

Demonstrate typed client/server/database integration and ambitious compile-time web guarantees. Their accepted and rejected examples should influence the project’s test corpus.

### Svelte and SvelteKit

Demonstrate excellent HTML-first authoring, compiler-generated targeted DOM updates, scoped component styling, and increasingly explicit remote `query`, `command`, `form`, and `prerender` operations.[^svelte-remote] They remain constrained by JavaScript/TypeScript semantics and do not provide the proposed whole-system effect, privacy, and placement model.

### Marko

Demonstrates server-first streaming, fine-grained client bundling, and a resumption-oriented model in which static content can require no client application JavaScript.[^marko-fast]

### Qwik

Provides the clearest current articulation of resumability and interaction-driven code loading.[^qwik-resume]

### Phoenix LiveView

Demonstrates HTML-first server state with pushed updates and strong operational simplicity for network-connected applications, while highlighting the tradeoff between server authority and rich offline/local computation.

### Skip, Convex, and Materialize

Provide relevant models for tracked effects, reactive queries, and incremental maintenance.[^skip][^convex][^materialize]

### WebAssembly Component Model and WASI

Provide a language-neutral typed ABI and capability-based host boundary suitable for browser/edge/origin components.[^component-model][^wasi]

### WICG declarative partial updates and TC39 Signals

Show that common framework mechanisms may eventually become platform primitives, though both areas remain evolving work rather than a complete application model.[^partial-updates][^signals]

### Servo

Provides a modular, embeddable browser engine in Rust suitable for later native-runtime experiments.[^servo]

---

## 16. Limitations and unresolved questions

### 16.1 Effect inference versus local readability

Inferring effects reduces annotation but can make behavior appear distant. The language needs editor-visible inferred signatures, explicit public interfaces, and diagnostics that show effect provenance.

### 16.2 Information-flow practicality

A useful privacy lattice must handle declassification, aggregation, logging, analytics, identifiers, and third-party integrations without either becoming unsound or making ordinary work intolerable.

### 16.3 Placement stability

Automatic placement can make deployments surprising and performance unstable across compiler versions. Placement decisions need inspectable plans, lockable policies, and change reports.

### 16.4 Resumable state versioning

A browser may hold HTML and serialized state from an earlier deployment. Content-addressed handlers, compatibility schemas, expiry, and safe fallback require careful design.

### 16.5 Cache dependency completeness

Automatically deriving dependencies through databases and external services is difficult. The first version should prefer explicit typed events and conservative invalidation over unsound cleverness.

### 16.6 Optimistic and offline conflict policy

The compiler can require a policy but cannot invent correct business resolution. Some conflicts need product decisions or centralized authority.

### 16.7 Compile time and diagnostics

Whole-application analysis can become slow. Incremental queries, stable interfaces, cached intermediate representations, and bounded analyses are essential.

### 16.8 Ecosystem interop

The platform must call JavaScript, TypeScript, Rust, and existing services. Every foreign boundary weakens guarantees unless validated and capability-wrapped.

### 16.9 Open-web standardization

A custom compatibility runtime can ship independently. Native browser primitives require multi-vendor evidence, standards participation, accessibility analysis, and security review.

---

## 17. Adoption strategy

### 17.1 Start as an embeddable island

A generated component should mount inside an existing React, SvelteKit, or plain HTML application. It receives explicit props and capabilities and exposes typed events.

### 17.2 Replace dangerous boundaries first

Good early targets include:

- checkout mutation flows;
- permission-sensitive admin panels;
- realtime subscriptions;
- complex effect-heavy forms;
- public pages mixed with private personalization;
- cache-sensitive catalog routes.

### 17.3 Preserve standard output

Generated HTML, CSS, URLs, forms, and HTTP endpoints reduce lock-in. A project can remove the runtime and retain a functional document where appropriate.

### 17.4 Open specification and governance

The accepted/rejected corpus, semantic specification, compiler, runtime, benchmark harness, and paper should be public. Architectural decisions should be recorded in RFCs and ADRs. Browser-facing proposals should be separated from proprietary deployment features.

---

## 18. Publication and communication

The project needs three levels of explanation:

1. A one-page narrative for developers and systems readers.
2. This design paper for architecture, tradeoffs, and evaluation.
3. A language/runtime reference for exact semantics.

Before results exist, the paper must remain labeled a design draft. After the first end-to-end evaluation, publish versioned source, a generated PDF, benchmark artifacts, and citation metadata. GitHub Pages can publish repository-backed static HTML; Cloudflare Pages can deploy static output and preview changes.[^github-pages][^cloudflare-pages] Zenodo can archive GitHub releases and issue a persistent DOI for a published record.[^zenodo] An eventual arXiv submission should be a scholarly paper with empirical evidence and related work, and is subject to arXiv’s moderation and first-category endorsement process.[^arxiv]

---

## 19. Conclusion

The modern web is not difficult only because it has too many tools. It is difficult because the most important relationships between those tools are missing from the program’s static meaning.

A component tree does not say whether a request is a query or command. A TypeScript type does not say whether data is public. A server route does not say whether a mutation is safe to retry. A cache key does not explain what invalidates it. A client directive does not prove that a secret remains at the origin. A test does not tell a reviewer which capability a generated patch added. A successful compilation does not mean that the web application’s architectural obligations have been met.

The proposed platform makes those obligations first-class. It combines a readable algebraic language, typed effects and capabilities, explicit resources, information-flow labels, placement constraints, semantic HTML, document parts, resumption, materialized views, and sandboxed components. It preserves the open web while asking the compiler to understand more of what application developers already mean.

The project should be judged by concrete proofs: failures rejected, work removed, private data contained, faults handled, generated changes reviewed, and model requirements reduced. If those proofs succeed, the result would not merely be another JavaScript framework. It would be a different contract between application authors and the web platform: developers declare meaning; the system owns mechanism.

---

## References

[^cloudflare]: Cloudflare, “A deep dive into Cloudflare’s September 12, 2025 dashboard and API outage.” <https://blog.cloudflare.com/deep-dive-into-cloudflares-sept-12-dashboard-and-api-outage/>

[^react-effects]: React documentation, “You Might Not Need an Effect,” “useEffect,” and related effect guidance. <https://react.dev/learn/you-might-not-need-an-effect>

[^svelte-effects]: Svelte documentation, “Best practices — `$effect`.” <https://svelte.dev/docs/svelte/best-practices>

[^svelte-remote]: SvelteKit documentation, “Remote functions.” <https://svelte.dev/docs/kit/remote-functions>

[^marko-fast]: Marko documentation, “Why is Marko Fast?” <https://markojs.com/docs/explanation/why-is-marko-fast>

[^qwik-resume]: Qwik documentation, “Resumable.” <https://qwik.dev/docs/concepts/resumable/>

[^koka]: Koka language book and documentation. <https://koka-lang.github.io/koka/doc/book.html>

[^wit]: Bytecode Alliance, “An Overview of WIT.” <https://component-model.bytecodealliance.org/design/wit.html>

[^wasi]: WASI.dev introduction and security model. <https://wasi.dev/>

[^component-worlds]: Bytecode Alliance, “WIT Worlds.” <https://component-model.bytecodealliance.org/design/worlds.html>

[^component-model]: Bytecode Alliance, “The WebAssembly Component Model.” <https://component-model.bytecodealliance.org/>

[^partial-updates]: WICG, “Declarative partial updates” and patching explainer. <https://github.com/WICG/declarative-partial-updates>

[^signals]: TC39, “JavaScript Signals standard proposal.” <https://github.com/tc39/proposal-signals>

[^skip]: Skip language repository and documentation. <https://github.com/skiplang/skip>

[^convex]: Convex documentation, “Realtime.” <https://docs.convex.dev/realtime>

[^materialize]: Materialize, incremental materialized-view architecture. <https://materialize.com/blog/self-correcting-materialized-views/>

[^servo]: Servo project and 2026 embeddable library release. <https://servo.org/>

[^reactbench]: ReactBench, benchmark and data. <https://www.reactbench.com/>

[^github-pages]: GitHub documentation, “What is GitHub Pages?” <https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages>

[^cloudflare-pages]: Cloudflare documentation, “Static HTML” on Pages. <https://developers.cloudflare.com/pages/framework-guides/deploy-anything/>

[^zenodo]: Zenodo documentation, GitHub integration and DOI records. <https://help.zenodo.org/docs/github/>

[^arxiv]: arXiv submission and endorsement guidance. <https://info.arxiv.org/help/submit/index.html>
