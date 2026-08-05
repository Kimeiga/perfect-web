# MASTER AGENT PROMPT — Build a Compiler-Checked, Resumable, Distributed Web Application Platform

> Copy this entire document into the root of the repository as `PROJECT_CHARTER.md`. Create short `AGENTS.md` and `CLAUDE.md` files that point to it and summarize only the operating rules. Treat this charter as the durable project constitution, not as a one-shot request to generate a giant speculative code dump.

## 0. Your role

You are the principal architect, compiler engineer, runtime engineer, web-platform engineer, test engineer, security reviewer, benchmark designer, and technical writer for this project.

Your job is to **research, design, implement, test, benchmark, document, and incrementally validate** a clean-slate web application platform that makes as much invalid application behavior as practical impossible to express or impossible to compile.

Do not merely write a plan. Execute the plan milestone by milestone. Do not attempt all milestones concurrently. Preserve a coherent architecture, create small vertical slices, and require objective gates before proceeding.

The target user is an experienced web and iOS engineer working on an Apple Silicon MacBook Pro with an M3 Max and 64 GB RAM. The initial system must run entirely on that MacBook using ordinary browsers and local processes. Later milestones may use Linux VMs on the same Mac, an iPhone as a real client, an existing Windows/Linux PC as an optional origin node, and an experimental Servo-based browser host.

The working repository name is `perfect-web` unless an existing repository already has a name. Do not spend time inventing branding.

---

## 1. Mission

Build a credible path toward a web stack with these properties:

1. **One high-level application language**, rather than JavaScript plus TypeScript plus a view library plus a meta-framework plus a query library plus a validation library plus a cache library plus deployment-specific conventions.
2. **Algebraic data types, nominal domain types, `Option`, `Result`, and exhaustive pattern matching** so impossible state combinations are unrepresentable.
3. **Typed effects and capabilities** so functions declare and the compiler infers whether they can access the network, database, clock, randomness, secrets, sessions, logs, storage, tasks, or device APIs.
4. **Pure rendering**. A view function cannot perform a request, mutate domain state, read time, generate randomness, start a task, or access a secret.
5. **No general-purpose application-level `useEffect` equivalent**. External behavior is represented by distinct typed primitives such as `query`, `command`, `subscription`, `resource`, and structured `task`.
6. **Structured concurrency**. Every ordinary task belongs to a request, route, component, session, or application scope and is automatically cancelled or completed when that scope ends. Detached durable work is explicit and capability-gated.
7. **Compiler-known placement** across build time, browser, edge, and origin. Browser code cannot access database or secret capabilities. Public shared renders cannot depend on private session data.
8. **Privacy-aware and cache-safe data flow**. Public, session, user, organization, device, and secret data cannot cross invalid boundaries or enter incompatible caches.
9. **Declarative remote resources** with keys, freshness, consistency, deduplication, cancellation, retry policy, invalidation, optimistic updates, and idempotency.
10. **Semantic HTML and CSS**, not an opaque canvas or generic cross-platform scene graph.
11. **Static document parts and fine-grained updates**, not repeated whole-component rendering plus virtual-DOM reconciliation.
12. **Frame-phase and layout safety**. Ordinary application code cannot synchronously interleave layout-invalidating DOM writes with geometry/style reads. Measurements, mutations, animation work, and post-paint work are represented by distinct typed phases and scheduled in batches.
13. **Streaming server rendering and resumption**, not general hydration that re-executes an already-rendered application tree.
14. **Incremental materialization**, where the modern equivalent of ISR is derived from resource dependencies and invalidation events rather than manually configured route timers.
15. **Capability-secure server and edge components**, eventually using WIT, the WebAssembly Component Model, WASI, and Wasmtime.
16. **A browser-compatible implementation first**, followed only later by experimental native browser primitives and Servo work.
17. **AI-friendly development**: a small regular language, deterministic compiler feedback, compile-fail tests, semantic diffs, generated evidence, and a benchmark that measures whether smaller agents can succeed more reliably than on React.

The governing principle is:

> Application developers declare domain meaning, permissions, freshness, consistency, state transitions, and UI. The compiler and platform own placement, scheduling, rendering, caching, serialization, invalidation, synchronization, cleanup, and low-level optimization.

---

## 2. What this project is not

Do **not** initially build any of the following:

- a database engine;
- a SQL optimizer;
- a new TCP or QUIC implementation;
- a complete browser engine from scratch;
- a CSS layout engine;
- a global CDN;
- a distributed database;
- a package registry;
- a bespoke binary replacement for HTML;
- a custom router appliance;
- a local foundation model;
- a universal CRDT system;
- a full production framework before proving the semantic model.

Reuse commodity infrastructure for those areas. The novel work is the semantic spine:

```text
value types
+ algebraic effects and capabilities
+ privacy and placement
+ resource lifecycle and consistency
+ cache and invalidation graph
+ UI document-parts graph
+ resumption metadata
+ compiler-generated evidence
```

---

## 3. Operating contract

### 3.1 Execute, do not just speculate

At the start of each milestone:

1. Inspect the current repository and `docs/STATUS.md`.
2. Verify current upstream APIs against primary documentation and source repositories.
3. Write or update an architecture decision record before a consequential design change.
4. Add failing tests or a reproducible benchmark before implementation where practical.
5. Implement the smallest end-to-end vertical slice that proves the milestone.
6. Run all relevant checks.
7. Record measured results, limitations, and unexpected findings.
8. Make a local Git commit with a focused message.
9. Update the milestone gate checklist.
10. Proceed only when the gate passes, or document precisely why it cannot yet pass.

Do not claim a milestone is complete merely because code exists. A milestone is complete only when its objective gate passes.

### 3.2 Do not ask broad design questions that can be resolved experimentally

Make reasonable assumptions, record them in an ADR, and validate them. Ask the user only when a decision is genuinely subjective, irreversible, expensive, or requires credentials or interactive administrator approval.

### 3.3 Never hide uncertainty

When an upstream project is experimental, an API changed, or an assumption failed, say so in the repository documentation. Do not invent APIs or pretend a compatibility layer is permanent.

### 3.4 Preserve progress across context windows

Maintain these files continuously:

```text
docs/STATUS.md
docs/NEXT.md
docs/KNOWN_LIMITATIONS.md
docs/RISK_REGISTER.md
docs/DECISIONS/ADR-*.md
docs/BENCHMARKS.md
docs/SEMANTICS.md
docs/ARCHITECTURE.md
```

`docs/STATUS.md` must always contain:

```text
current milestone
last passing commit
completed gate items
failing gate items
exact commands to reproduce
known environmental issues
last benchmark summary
next three concrete tasks
```

### 3.5 Git and subagent discipline

- Initialize Git if needed.
- Commit locally at each meaningful gate.
- Do not push, publish, open a remote PR, or modify an external repository without explicit authorization.
- Never let two agents edit overlapping files simultaneously.
- If the host agent uses subagents, give each a separate Git worktree and a non-overlapping assignment.
- One integrator owns architecture and merges subagent work only after tests.
- Subagents must read `PROJECT_CHARTER.md`, `docs/ARCHITECTURE.md`, and the relevant ADRs before changing code.
- Delete abandoned worktrees after integrating or rejecting them.

### 3.6 Dependency and license discipline

For every dependency that becomes part of the implementation:

- record its version and license;
- prefer stable public extension points over forks;
- pin versions and commit lockfiles;
- avoid GPL-derived implementation code in a permissively licensed core unless explicitly approved;
- use GPL projects such as Links as research references and test-corpus inspiration, not copied implementation;
- run license and vulnerability checks in CI;
- create an adapter boundary around experimental dependencies.

The intended project license is dual MIT/Apache-2.0 unless existing repository constraints say otherwise.

### 3.7 Security rules

- Never commit credentials, tokens, private certificates, or user data.
- Use local fixture secrets only.
- Do not use broad `--dangerously-skip-permissions` or equivalent modes unless the user explicitly chose that execution environment.
- Do not run destructive commands outside the repository or named local VMs.
- Before executing remote install scripts, download them from the official source, inspect them, and record the installed version.
- Require explicit capabilities for filesystem, network, secrets, database, process execution, and device APIs.

---

## 4. Reuse, fork, tape, or build decision framework

For every subsystem, classify it as one of four strategies and record the decision.

### Reuse permanently

Use an existing project as a durable dependency when:

- it solves commodity infrastructure rather than defining the novel semantics;
- it has a stable public API or standard;
- replacing it would not materially improve the research question;
- its license and maintenance model are acceptable.

Expected permanent reuse candidates:

- Rust toolchain;
- WIT and the WebAssembly Component Model;
- WASI where supported;
- Wasmtime;
- SQLite and PostgreSQL;
- standard HTML, CSS, URLs, HTTP, HTTP/3, and QUIC;
- ordinary browser accessibility and DOM semantics;
- Lima and Linux traffic control for the local lab.

### Tape together temporarily

Use two systems through a narrow generated boundary when that lets the project validate semantics before reimplementing machinery.

Expected temporary composition:

- custom source language → generated Koka for type/effect checking;
- custom templates → generated Marko 6 for streaming and resumption;
- Marko/Node frontend → generated calls into a Rust/Wasmtime host;
- JavaScript DOM shim → document-parts runtime until native browser experiments exist.

Every temporary adapter needs:

- a documented deletion condition;
- tests that define behavior independently of the dependency;
- no leakage of the temporary dependency’s semantics into application source.

### Fork only after evidence

Fork when all are true:

1. the semantics have been validated;
2. a public extension point cannot implement a measured requirement;
3. the fork can stay narrow and regularly rebased;
4. the project can test the fork against upstream behavior;
5. an ADR documents maintenance cost and exit strategy.

Likely future narrow forks:

- Koka only to expose structured typed AST/effect metadata if upstream cannot;
- Servo only after profiling proves a browser-native primitive materially helps.

### Build from scratch

Build the parts that define the project’s unique value:

- the source language and grammar;
- semantic IR;
- effect/capability model as applied to web applications;
- privacy and placement checker;
- resource graph;
- cache/materialization semantics;
- document-parts compiler and resumption manifest;
- semantic PR diff;
- compile-fail corpus;
- project-specific AI benchmark.

---

## 5. Research sources and design donors

Before implementing a borrowed idea, inspect primary documentation, papers, source code, and current release notes for the relevant project. Maintain `docs/research/technology-matrix.md` with what is borrowed, what is rejected, and why.

Study these projects for specific ideas:

| Project | Ideas to study | Do not assume |
|---|---|---|
| Koka | inferred effect rows, handlers, ADTs, `maybe`, Perceus, optimized reference counting | that its browser, async, package, Wasm Component, or production ecosystem is sufficient |
| Elm | pure view, commands, subscriptions, decoders, readable ADTs | that its full-stack and deployment model is complete |
| Gleam | small readable syntax, explicit failure, algebraic types, friendly inference | that BEAM/JS targets or its effect model solve this project |
| Rust | `Option`, `Result`, exhaustive enums, affine resources, deterministic cleanup, robust compiler diagnostics | that ownership syntax belongs in ordinary UI/business code |
| Effekt | scoped capabilities and effect safety | that it is the runtime or web stack to ship |
| Unison | content-addressed code and abilities | that its deployment model should be copied wholesale |
| Roc | platform-owned capabilities and narrow application APIs | that its compiler maturity or effect system fits immediately |
| Links | one language split across browser/server/database and typed RPC | that its renderer and runtime match modern resumable web needs |
| Ur/Web | compile-time web-safety guarantee checklist | that its implementation should be reused directly |
| Skip | effects tied to safe memoization and incremental invalidation | that it supplies the full UI and server platform |
| Jane Street Incremental | a stable dependency DAG, cutoffs, stabilization, and incremental recomputation of arbitrary derived values | that its OCaml implementation should become the permanent cross-target runtime |
| Bonsai/Bonsai_web | purely functional state machines, a static computation DAG, lifecycle/scoping, whole-program incrementality, and unusually strong UI expect tests | that its virtual-DOM diff/patch loop, Js_of_ocaml target, or lifecycle APIs prevent forced layout or provide SSR/resumption/placement |
| Svelte/SvelteKit | SFC ergonomics, semantic HTML, scoped CSS, compiled targeted updates, typed remote-function direction | that JavaScript semantics, generic effects, hydration, and manual placement are ideal |
| Marko 6 | streaming, resumability, zero-JS static output, lazy interaction code | that JavaScript semantics should remain the permanent core |
| Qwik | serialized resumption and interaction-lazy code | that arbitrary closure serialization is automatically safe |
| Phoenix LiveView | HTML-first server state and pushed diffs | that constant network dependence suits every interaction |
| Convex | reactive queries and tracked dependencies | that the storage platform should be mandatory |
| Materialize/differential dataflow | incremental view maintenance | that a distributed dataflow engine belongs in the first prototype |
| WIT/WASI/Wasmtime | typed component interfaces and host-granted capabilities | that the newest WASI version is fully supported by every toolchain |
| Servo | modular Rust browser engine and embedding | that it is ready to replace production browsers |
| WICG declarative partial updates | native document range patches and out-of-order streaming | that a proposal is already standardized |
| TC39 Signals | common low-level reactive primitives | that a JavaScript proposal is the complete UI model |

Do not trust dates, versions, package names, or APIs in this charter without checking current primary sources. Pin what was actually tested.

---

## 6. Target architecture

The initial deployable architecture is:

```text
Application source (`.pw` or final chosen extension)
        │
        ▼
Custom compiler front end (Rust)
  ├── parser + lossless syntax tree
  ├── names + modules
  ├── domain/value type declarations
  ├── effect/capability declarations
  ├── privacy labels
  ├── placement constraints
  ├── query/command/subscription/resource graph
  ├── HTML/template graph
  └── CSS/style metadata
        │
        ├── generated Koka modules              early milestones
        ├── generated Marko 6 templates         early milestones
        ├── generated JS bridge                 early milestones
        ├── generated resource manifests
        ├── generated semantic reports
        └── source maps
                  │
                  ▼
       Node/Marko development host
                  │
                  ▼
       Rust capability host + Wasmtime
          ├── database interfaces
          ├── secret/session interfaces
          ├── cache/materialization
          ├── tracing
          └── WIT-defined capabilities
```

The later architecture removes temporary layers:

```text
Application source
        │
        ▼
Own compiler + semantic IR
        │
        ├── HTML + typed CSS + document-parts manifest
        ├── browser JS or Wasm handlers
        ├── edge/origin Wasm Components
        ├── resource dependency graph
        ├── cache/materialization plan
        ├── WIT worlds
        ├── semantic diff
        └── debug/trace metadata
                  │
                  ▼
Browser ───── edge ───── origin ───── database
   one compiler-known distributed application
```

---

## 7. Language semantics

These semantics are more important than final syntax. Do not optimize syntax before they are demonstrated.

### 7.1 Values and types

Support, in stages:

- booleans, integers, floating-point values, strings, byte arrays;
- immutable records;
- tuples;
- lists and maps with explicit identity requirements where needed;
- generic algebraic data types;
- tagged unions;
- exhaustive pattern matching;
- nominal opaque domain types;
- `Option<T>` with `Some(T)` and `None`;
- `Result<T, E>` with `Ok(T)` and `Err(E)`;
- typed units and eventually generic quantities such as `Money<USD>`;
- no ambient `null` or `undefined` in application code;
- no implicit numeric/string coercions;
- no unchecked `any`, non-null assertion, or unvalidated cast in normal code;
- no unchecked exceptions as ordinary domain control flow;
- explicit decoding at every external boundary.

Example domain model:

```text
opaque type StoreId = String
opaque type ConsumerId = String
opaque type MenuItemId = String
opaque type InteractionId = String

type Money<Currency> = Money {
    minor_units: Int64
}

type OrderState =
    | Draft
    | Pricing
    | AuthorizingPayment
    | Submitting
    | SearchingForDasher
    | Confirmed(OrderConfirmation)
    | InProgress(OrderProgress)
    | Delivered(DeliveryReceipt)
    | Cancelled(CancellationReason)
    | Failed(OrderFailure)
```

### 7.2 Effects

Function types must represent effects. Exact syntax may evolve, but the semantic model should support signatures like:

```text
calculate_subtotal : Cart -> Money<USD> !{}

load_store : StoreId
    -> Result<Store, StoreError>
    !{ database.read<Stores>, trace }

request_location : Permission<Location>
    -> Result<Location, LocationError>
    !{ device.location, ui.prompt }
```

Core effect families:

```text
database.read<Resource>
database.write<Resource>
network<Origin>
clock.monotonic
clock.wall
random.secure
random.nondeterministic
trace
log<PrivacyLevel>
session
secret<Name>
storage<Namespace>
device.location
device.camera
ui.prompt
task.spawn
resource.acquire<Type>
durable_job.enqueue<Type>
```

Effects should be row-polymorphic where practical so generic library functions preserve the effects of callbacks.

### 7.3 Capabilities

Effects describe what code requires. Deployment capabilities determine what each target receives.

Examples:

```text
browser world:
- public HTTP client to approved origins
- session-scoped storage
- optional device permissions
- no database
- no server secrets

edge world:
- public cache
- restricted origin RPC
- tracing
- no payment secret

origin world:
- database
- payment secret
- session verification
- durable jobs
```

The compiler must reject code whose required effects are unavailable at its placement.

### 7.4 Pure rendering

A `view` function has an empty external effect row. It may construct semantic UI descriptions from values and local reactive reads, but may not:

- perform network or database I/O;
- mutate domain state;
- write storage;
- read clock or randomness;
- log private values;
- start a task;
- acquire an imperative resource;
- read a secret;
- issue analytics;
- enqueue durable work.

The system should make this compile-time invalid:

```text
view StorePage(store_id) {
    let store = fetch_store(store_id) // compile error: network effect in view
    ...
}
```

### 7.5 No universal lifecycle effect

Do not expose a general application primitive equivalent to React `useEffect` or an unrestricted Svelte `$effect`.

Use distinct concepts:

| Primitive | Meaning |
|---|---|
| `signal` | ephemeral local UI state |
| `derived` | pure value computed from other values |
| `query` | keyed remote read with cache, freshness, lifecycle, cancellation, and dedupe |
| `command` | explicit mutation with authorization, idempotency, transaction, optimistic behavior, and invalidation |
| `subscription` | scoped stream of changing external values |
| `resource` | acquire/release lifecycle for an imperative handle |
| `task` | structured child computation owned by a scope |
| `durable` | explicit background job that outlives the request |
| `unsafe lifecycle` | infrastructure-only escape hatch, visibly audited and forbidden by default in app packages |

### 7.5A Layout effects and frame phases

Fine-grained DOM updates do not by themselves prevent forced synchronous layout. The language and browser runtime must distinguish layout-sensitive operations and make the safe path the default.

Model at least these effect/phase families:

```text
dom.mutate                 # attributes, classes, text, insertion/removal
style.mutate<LayoutAffect> # writes that may invalidate style/layout
layout.measure             # geometry such as boxes, scroll metrics, computed layout
observe.resize             # browser-delivered element-size changes
observe.intersection       # browser-delivered visibility/intersection changes
animation.composite        # transform/opacity-style compositor work
paint.custom               # canvas/custom painting escape hatch
post_paint                 # work intentionally deferred until after presentation
```

Ordinary application code must not directly call synchronous geometry APIs such as `offsetWidth`, `clientHeight`, `scrollHeight`, `getBoundingClientRect`, or layout-dependent computed-style reads. Expose typed framework primitives that return a scheduled `LayoutSnapshot<T>` instead.

The default frame transaction is:

```text
1. receive input and resource changes
2. stabilize pure/incremental computations
3. collect all requested measurements while layout is clean
4. compute the mutation plan without touching the DOM
5. apply all DOM/style mutations in one batched commit
6. allow browser style/layout/paint/composite
7. deliver observer and post-paint results into a later transaction
```

A mutation that requires a fresh measurement of its result must observe that result in a later frame or through a browser observer; it must not force the browser to synchronously flush layout mid-transaction. Provide an explicit audited infrastructure escape hatch for cases where this latency is unacceptable, and attribute its forced-layout cost.

The compiler/runtime should additionally:

- group all reads before all writes;
- deduplicate measurements of the same element/property in one frame;
- coalesce DOM mutations by stable document part;
- cancel measurements for scopes that unmount;
- prefer `ResizeObserver` and `IntersectionObserver` over polling;
- warn on feedback loops where a resize observation immediately changes the observed size;
- generate CSS `contain` or `content-visibility` only when subtree independence is semantically valid;
- require virtualization/windowing for unbounded rendered collections;
- prefer transforms and opacity for high-frequency visual motion when equivalent to the requested semantics;
- expose frame-budget and layout-causality data in developer tools.

Do not claim that the compiler can abolish layout cost. Text, fonts, intrinsic sizing, CSS Grid/Flexbox, images, viewport changes, and genuine document geometry still require browser layout. The goal is to eliminate accidental read/write thrashing, constrain the affected subtree, and make unavoidable layout observable and budgeted.

### 7.6 Structured concurrency

Every ordinary task is attached to a parent scope. Exiting a component, route, request, or session scope must cancel or complete children.

Require explicit semantics for:

- cancellation;
- timeout;
- retry;
- backoff and jitter;
- error aggregation;
- task ownership;
- whether results may arrive after the initiating UI state changed.

No fire-and-forget promise by default.

### 7.7 Affine resources

Ordinary values use automatic memory management. Scarce or correctness-sensitive handles may be affine or linear:

```text
DatabaseTransaction
OpenStream
WebSocketConnection
SubscriptionHandle
MapHandle
SecretHandle
LockGuard
FileUpload
```

The compiler should eventually require that a transaction ends exactly once in commit or rollback and that a resource does not escape its valid scope.

### 7.8 Privacy labels

Start with an explicit, conservative label system:

```text
Public
Session<SessionId>
User<UserId>
Organization<OrganizationId>
Device
Secret<Capability>
```

Do not force all labels into a simplistic total order. Some are incomparable. Track joins through data flow and reject invalid serialization, logging, placement, and caching.

Examples that must fail:

```text
SharedCache<Cart@Session>
BrowserValue<PaymentSecret>
PublicRender<CurrentUserEmail>
Log<Public>(PaymentToken@Secret)
```

### 7.9 Placement

Initial placement categories:

```text
Build
Browser
Edge
Origin
Database
```

Placement is solved from:

- required effects and capabilities;
- privacy labels;
- data dependencies;
- declared latency preference;
- consistency policy;
- browser-only APIs;
- secret/database restrictions;
- cache sharing policy.

Allow explicit constraints, but derive placement by default.

### 7.10 External decoding

Every value entering from JSON, forms, headers, cookies, browser storage, databases, third-party APIs, or message queues starts as `Unknown` until decoded.

Decoders must return `Result<T, DecodeError>`. Do not generate unchecked assertions.

---

## 8. Framework and UI model

### 8.1 Authoring format

Use a Svelte-like single-file component format because it keeps logic, semantic markup, and scoped styles close together. The exact syntax is provisional; semantics are not.

A source sketch:

```svelte
<script lang="pw">
type StorePageState =
    | Loading
    | Ready(Store, Cart)
    | Failed(StoreError)

public query store(id: StoreId)
    freshness 30.seconds
    consistency snapshot
    invalidates StoreChanged(id)
    fallback last_known_good
{
    Stores.get(id)
}

session query cart()
    consistency read_your_writes
    cache private
{
    Carts.current()
}

command add_to_cart(item: MenuItemId, quantity: PositiveInt)
    requires SignedIn
    idempotent_by InteractionId
    optimistic cart.add(item, quantity)
    invalidates cart
{
    Carts.add(current_consumer(), item, quantity)
}
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
                    on:press={() => add_to_cart(item.id, 1)}
                >
                    Add
                </button>
            </li>
        {/each}
    </ul>
</main>

<style scoped>
main {
    padding: token(space.page);
}

li {
    display: grid;
    grid-template-columns: 1fr auto auto;
}
</style>
```

Explicit tags are the canonical representation. Indentation may be editor sugar, but must not be the only stored structure. Application source is not transmitted over the network, so HAML-like brevity is not a network optimization.

### 8.2 Semantic HTML

Preserve standard elements and behavior:

```text
main, nav, section, article, form, label, input, button, table, a
```

Do not replace them with generic `View`, `Container`, or `ActionSurface` nodes. Compile-time checks should cover as many of these as practical:

- invalid nesting;
- missing labels;
- invalid form/handler types;
- keyboard-inaccessible click targets;
- invalid ARIA relationships;
- unstable list keys;
- dead internal routes;
- unsafe raw HTML;
- hydration/resumption mismatch hazards;
- duplicate IDs.

### 8.3 Styling

Keep a CSS-like declarative language with:

- scoped styles by default;
- explicit global cascade layers;
- typed design tokens;
- typed custom properties and units where practical;
- media and container queries;
- logical properties and writing modes;
- user preference support;
- print and accessibility modes;
- static dead-rule and conflict analysis.

Do not require runtime CSS-in-JS. Tailwind-like utilities may exist as compile-time macros that lower into the same style representation.

### 8.4 Static document parts

Compile templates into stable parts:

```text
StaticNode
TextPart
AttributePart
RangePart
KeyedListPart
EventPart
StylePart
```

Construct an explicit dependency graph:

```text
cart.total
    └── TextPart(cart-total)

item.available
    ├── AttributePart(add-button.disabled)
    └── TextPart(availability-label)

store.items
    └── KeyedListPart(menu-items, key=item.id)
```

When one value changes, update only dependent parts. Do not conceptually rerender the entire component and compare virtual trees.

### 8.5 Resumption

SSR should emit semantic HTML plus only the metadata needed to resume interactions. Do not generally re-execute the complete component tree in the browser.

A handler may capture only:

- serializable immutable values;
- resource keys;
- component-local signal identifiers;
- explicit capability-safe handles designed for resumption.

It may not capture:

- a database connection;
- server secret;
- open transaction;
- arbitrary nonserializable closure;
- DOM node from another scope;
- live process or thread handle.

Handlers should have stable content-addressed identifiers. Stale manifests must be detected by code-version or handler-hash mismatch and recover safely.

### 8.6 Declarative patches

Support out-of-order server completion using document-range patches. Initially implement this with a tiny, well-tested browser shim and `<template>`-like payloads. Keep the abstraction compatible with possible future declarative browser primitives.

---

## 9. Resource, SSR, ISR, and consistency model

### 9.1 Four kinds of state

Do not represent everything with one generic state primitive.

1. **Ephemeral local UI state**: open menus, tab selection, unfinished inputs.
2. **Domain state machines**: checkout, authentication, payment, order lifecycle.
3. **Remote resources**: store, cart, menu, profile, order history.
4. **Replicated/offline state**: intentionally local and synchronized data with explicit conflict policy.

### 9.2 Resource declarations

A resource declaration must be able to express:

```text
key
privacy
freshness
consistency
cache partition
stale fallback
invalidation source
retry policy
timeout
request deduplication
concurrency limit
placement preference
offline behavior
serialization
observability class
```

Example:

```text
public query Store(StoreId) {
    freshness 30s
    consistency snapshot
    cache shared
    invalidates_on StoreChanged(StoreId)
    fallback last_known_good
    retry bounded_exponential(max=3, jitter=true)
    concurrency one_per_key
}

session query Cart(SessionId) {
    freshness 0s
    consistency read_your_writes
    cache private
    concurrency one_per_key
}
```

### 9.3 SSR/SSG/ISR derivation

Developers do not ordinarily choose `SSR`, `SSG`, `ISR`, `CSR`, `server component`, or `client component` directly. Derive an implementation from semantics:

| Semantic properties | Likely implementation |
|---|---|
| public, deterministic, build-known | static generation |
| public, mutable, staleness tolerated | shared edge materialization |
| private/session-specific | private SSR or browser resource |
| secret/privileged | origin execution |
| browser/device-only | browser execution |
| slow independent subtree | streamed region |
| realtime resource | subscription |
| offline-writable | local replica plus explicit conflict policy |

### 9.4 ISR successor

Treat ISR as a materialized view over resources, not a route timer.

```text
StorePage(store_47)
    ├── Store(store_47)
    ├── Menu(store_47)
    └── Promotions(store_47)
```

`MenuChanged(store_47)` should invalidate only affected resource snapshots and page fragments.

Start with explicit typed invalidation events and a transactional outbox. Do not attempt automatic SQL dependency inference first.

Later, deterministic query execution may record logical reads and derive dependencies automatically.

### 9.5 Reliability against effect storms

The platform must mechanically reduce the class of failures exemplified by a reactive identity change repeatedly issuing a network request.

For every query key, support:

- at most one in-flight request unless policy says otherwise;
- cancellation when no subscriber remains;
- bounded retry with jitter;
- retry only when the operation is safe;
- request coalescing;
- cache stampede protection;
- server rate limits and circuit breakers;
- traceable ownership from UI event to request;
- deduplicated reconnect behavior.

A rerender or local reactive recomputation must not create a new request unless the semantic resource key or policy changed.

---

## 10. Server, edge, components, and capabilities

### 10.1 Initial host

Use Node only for the first Marko integration. Keep all app-specific I/O behind generated interfaces so Node is replaceable.

### 10.2 Permanent host direction

Build a small Rust host around Wasmtime. Use WIT worlds and host imports to represent capabilities.

Conceptual mapping:

```text
language effect                    host interface

database.read<Stores>          ->  stores.read
database.write<Carts>          ->  carts.write
clock.monotonic                ->  clocks.monotonic
random.secure                  ->  random.secure
network<approved-origin>       ->  outbound-http capability
secret<Payments>               ->  payments-secret handle
trace                          ->  telemetry.span
```

A component without an imported capability cannot use it.

### 10.3 WASI version policy

Prefer the newest stable WASI version that the selected Wasmtime/toolchain supports reliably. Verify support experimentally. If WASI 0.3 async/component tooling is incomplete for a required language or binding generator, use stable WASI 0.2 behind an adapter and record the migration plan. Do not block the semantic prototype on the newest preview.

### 10.4 Database policy

Use SQLite first, then PostgreSQL when separating edge, origin, and database nodes.

Do not build a database. Use ordinary transactions and a transactional outbox for invalidation events.

### 10.5 Observability

Every generated operation should carry stable semantic identifiers:

```text
page
component
resource
query key
command
interaction ID
task scope
cache key
placement
privacy label
code version
```

Produce traces that explain causality, not merely low-level spans.

---

## 11. Browser and protocol strategy

### 11.1 Existing browsers first

The first implementation must run in current Safari, Chrome, Firefox, and Playwright’s WebKit/Chromium/Firefox using:

- semantic HTML;
- standard CSS;
- a small generated JavaScript parts/resumption runtime;
- optional Wasm for pure computation;
- ordinary HTTP responses;
- standard streaming and event channels.

### 11.2 Preserve HTML and HTTP

Do not replace HTML, URLs, CSS, or HTTP wholesale. They provide important interoperability, accessibility, streaming, indexing, caching, and debugging properties.

### 11.3 HTTP/3 later, not a custom transport

Use an existing HTTP/3/QUIC implementation or server. Do not write QUIC. A navigation may eventually coordinate prioritized resources for:

```text
semantic HTML shell
critical CSS
public resource state
private resource state
resume manifest
immediately usable handlers
delayed patches
live invalidations
```

Prove the application semantics over ordinary HTTP first.

### 11.4 Servo experiment only after measurement

Use Servo as an experimental embedded browser host only after the compatibility implementation works and profiling identifies specific overhead.

First embed unmodified Servo. Fork only to test measured ideas such as:

- direct Wasm access to typed DOM parts;
- native resumable handler registration;
- native declarative patch streams;
- browser-enforced component capabilities;
- content-addressed handler caching;
- causal reactive devtools.

Do not attempt to make Servo a production Safari/Chrome replacement.

---

## 12. Repository layout

Do not create empty directories merely to look complete, but converge toward this structure as milestones need it:

```text
perfect-web/
├── PROJECT_CHARTER.md
├── AGENTS.md
├── CLAUDE.md
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── Cargo.toml
├── pnpm-workspace.yaml
├── package.json
├── rust-toolchain.toml
├── justfile
├── docs/
│   ├── ARCHITECTURE.md
│   ├── SEMANTICS.md
│   ├── STATUS.md
│   ├── NEXT.md
│   ├── KNOWN_LIMITATIONS.md
│   ├── RISK_REGISTER.md
│   ├── BENCHMARKS.md
│   ├── DECISIONS/
│   ├── milestones/
│   └── research/
├── compiler/
│   ├── pw-cli/
│   ├── pw-syntax/
│   ├── pw-hir/
│   ├── pw-types/
│   ├── pw-effects/
│   ├── pw-placement/
│   ├── pw-resources/
│   ├── pw-template/
│   ├── pw-codegen-koka/
│   ├── pw-codegen-marko/
│   ├── pw-codegen-wit/
│   ├── pw-diagnostics/
│   └── pw-semantic-diff/
├── runtime/
│   ├── browser-parts/
│   ├── marko-adapter/
│   ├── node-dev-host/
│   ├── rust-host/
│   ├── materializer/
│   └── protocol/
├── stdlib/
│   ├── koka/
│   └── pw/
├── examples/
│   ├── accepted/
│   ├── rejected/
│   ├── hello-static/
│   ├── counter/
│   └── store/
├── benchmarks/
│   ├── baselines/
│   │   ├── next-react/
│   │   ├── sveltekit/
│   │   └── marko/
│   ├── harness/
│   └── results/
├── lab/
│   ├── lima/
│   ├── netem/
│   ├── certs/
│   └── profiles/
├── tools/
├── scripts/
└── .github/workflows/
```

---

## 13. MacBook Pro development environment

### 13.1 Detect before installing

Record the machine state in `docs/environment/macbook.md`:

```bash
uname -a
uname -m
sw_vers
sysctl -n hw.memsize
xcode-select -p || true
brew --version || true
rustup --version || true
node --version || true
pnpm --version || true
koka --version || true
wasmtime --version || true
limactl --version || true
```

The expected architecture is Apple Silicon `arm64`. Detect rather than assume.

### 13.2 Base tools

Use Xcode Command Line Tools and Homebrew for commodity host utilities. Prefer official language installers and pin versions.

Likely host tools:

```text
git
jq
just
hyperfine
mise or another minimal version manager
mkcert
lima
openssl
caddy or another existing HTTP/3-capable edge proxy later
```

Use:

- `rustup` and `rust-toolchain.toml` for Rust;
- a pinned current Node LTS and Corepack/pnpm for JavaScript tooling;
- the official Koka installer or pinned release artifact;
- the official Wasmtime installer or pinned release artifact.

Do not blindly run `curl | sh`. Save official scripts, inspect them, then execute.

### 13.3 Toolchain files

Create:

```text
rust-toolchain.toml
packageManager field in package.json
pnpm-lock.yaml
tools/versions.lock
```

`tools/versions.lock` should record:

```text
macOS version
Xcode CLT version
Rust toolchain
Cargo tools
Node
pnpm
Koka
Marko
Vite
Wasmtime
WASI/WIT tooling
Lima
Playwright
browser versions
```

### 13.4 Required `just` commands

Provide these stable developer commands as they become applicable:

```bash
just doctor
just bootstrap
just fmt
just lint
just test
just test-unit
just test-compile
just test-integration
just test-e2e
just test-security
just bench
just demo
just lab-up
just lab-test
just lab-down
just ci
```

`just doctor` must be read-only and explain missing dependencies.

### 13.5 macOS-specific concerns

- Detect case-insensitive filesystem assumptions and run Linux CI to catch casing errors.
- Prefer arm64-native binaries.
- Avoid hardcoded `/usr/local`; account for Apple Silicon Homebrew under `/opt/homebrew`.
- Keep generated files and caches out of iCloud-synced directories.
- Do not allocate all 64 GB to VMs.
- Ensure file watcher limits and build caches remain reasonable.
- Use deterministic paths and source maps despite generated Koka/Marko artifacts.

---
## 14. Milestone program

The milestones are ordered to answer the highest-risk questions early. Do not skip directly to a custom browser or full compiler backend.

Each milestone document in `docs/milestones/` must contain:

```text
question being tested
rationale
scope
non-goals
deliverables
tests
gate
measured result
risks discovered
next decision
```

---

### Milestone 0 — Repository bootstrap, research matrix, specification corpus, and feasibility spikes

#### Question

Can the proposed semantic architecture be made concrete enough that accepted and rejected programs define the project before a large implementation exists? Can Koka, Marko, Wasmtime, and the Mac toolchain interoperate sufficiently to justify the bootstrap plan?

#### Tasks

1. Initialize the repository, licenses, formatting, CI skeleton, `justfile`, toolchain pins, and status documents.
2. Build `just doctor` and `just bootstrap` without installing unnecessary tools.
3. Research current primary sources for Koka, Marko 6, Svelte/SvelteKit remote resources, Qwik resumption, Links, Ur/Web, Roc, Effekt, Skip, Jane Street Incremental, Bonsai/Bonsai_web, WIT/WASI, Wasmtime, Lima, Servo, declarative partial updates, browser rendering phases, forced synchronous layout, CSS containment, `content-visibility`, ResizeObserver, and Long Animation Frame attribution.
4. Create `docs/research/technology-matrix.md` with columns:

```text
project
useful idea
maturity
license
extension points
known limitations
reuse/fork/tape/build decision
deletion or migration condition
```

5. Write initial ADRs for:
   - Koka as temporary semantic compiler;
   - Marko as temporary renderer;
   - Rust as permanent compiler/host language;
   - standard HTML/CSS/HTTP preservation;
   - SQLite-first data layer;
   - Wasmtime capability host;
   - no browser fork before profiling;
   - explicit invalidation before automatic dependency tracking.
6. Create the first accepted/rejected program corpus as plain specification files, even before they compile.
7. Build six isolated feasibility spikes:

```text
spikes/koka-js-interop
spikes/marko-stream-resume
spikes/wasmtime-component
spikes/compiler-diagnostic
spikes/bonsai-incremental-model
spikes/layout-phase-scheduler
```

8. In `koka-js-interop`, prove:
   - Koka ADTs and effect handlers compile on Apple Silicon;
   - generated JavaScript can be imported from Node as an ES module or through a documented adapter;
   - a simple `maybe`/`result` value crosses the boundary without unchecked guessing.
9. In `marko-stream-resume`, prove:
   - a static Marko page emits no application JavaScript or document the actual output;
   - a delayed subtree streams independently;
   - one interactive button resumes or activates without eagerly hydrating the entire page;
   - emitted assets and runtime bytes are measured.
10. In `wasmtime-component`, compile and run one minimal typed component with one host-provided capability. If WASI 0.3 bindings are not yet reliable in the tested toolchain, record that and use the stable supported path.
11. In `compiler-diagnostic`, build a tiny Rust CLI that parses one toy declaration and emits a source-span diagnostic. This validates the development ergonomics before choosing parser libraries.
12. In `bonsai-incremental-model`:
   - build or run a minimal current Bonsai/Bonsai_web application on Apple Silicon, or document the exact blocker if the current toolchain cannot be installed reproducibly;
   - demonstrate that an unrelated state update does not recompute an instrumented expensive derived value;
   - inspect the static computation DAG, lifecycle/scoping model, and expect-test workflow;
   - confirm from source which portions use Jane Street Incremental and which portions still produce a virtual DOM and diff/patch it;
   - record which semantics should be borrowed into the own computation/resource graph and why Bonsai is not the permanent renderer by default.
13. In `layout-phase-scheduler`:
   - create an intentionally thrashing page that alternates layout-invalidating writes and geometry reads;
   - create a phase-scheduled implementation that batches measurements before mutations;
   - instrument both with Chromium performance traces and, where available, Long Animation Frame `forcedStyleAndLayoutDuration`;
   - test size observation with ResizeObserver and one feedback-loop case;
   - test CSS containment and `content-visibility` on an isolated large subtree;
   - record a reproducible baseline proving the project can detect forced synchronous layout before attempting to prevent it.
14. Create `docs/vision/non-goals.md` to resist scope creep.

#### Gate

Milestone 0 passes only when:

- `just doctor` works on the Mac;
- all six spikes run from documented commands, or a primary-source-backed blocker is recorded for any upstream toolchain that cannot currently run;
- versions and licenses are pinned;
- at least 10 accepted and 20 rejected semantic examples exist;
- the reuse/fork/tape/build matrix is complete enough to justify Milestone 1;
- known failures are documented honestly;
- one clean `just ci` command passes the existing checks.

#### Rationale

The greatest early risk is not code volume. It is discovering that the assumed integration boundaries do not exist. Spikes make that visible before the repository becomes committed to a fragile architecture.

---

### Milestone 1 — Koka semantic kernel

#### Question

Can Koka’s existing ADTs, effect rows, handlers, and automatic memory model represent the desired application semantics well enough to serve as a bootstrap oracle?

#### Tasks

1. Build Koka libraries under `stdlib/koka/` for provisional effects:

```text
database_read
database_write
network
clock
random
trace
session
secret
storage
task_scope
resource_scope
query
command
subscription
```

2. Define domain ADTs and opaque wrappers for:

```text
StoreId
ConsumerId
MenuItemId
InteractionId
Money
Option/Maybe
Result
StoreError
CartError
OrderState
```

3. Implement handlers for local development:

```text
database -> in-memory map or SQLite adapter boundary
clock -> deterministic test clock
random -> seeded deterministic test handler
trace -> collected event list
session -> fixture session
secret -> named local fixture handle
```

4. Demonstrate that effect inference distinguishes:

```text
pure subtotal calculation
store query
database mutation
nondeterministic render attempt
secret access
```

5. Implement a compile-test harness that runs Koka over accepted and rejected snippets and snapshots diagnostics.
6. Prove that a pure view wrapper cannot call an effectful query without the effect appearing and the wrapper rejecting it.
7. Prototype scoped cleanup with handlers or explicit affine wrappers. Do not claim Koka statically proves linear usage if it does not; document the gap.
8. Determine whether machine-readable inferred types/effects can be obtained without a fork.
9. If not, build a narrow parser for Koka compiler output or explicit generated metadata only as a temporary bridge. Do not fork yet unless a tiny structured-output patch is clearly justified.
10. Add differential examples comparing the desired language notation with generated Koka.

#### Gate

- Pure examples compile and run.
- Every effectful example has a visible inferred effect.
- A network request in a declared pure view fails the harness.
- External data decoding returns explicit errors.
- At least 30 compile-pass and 30 compile-fail cases are automated.
- The Koka adapter boundary and its limitations are documented.
- No application authoring syntax is permanently coupled to Koka syntax.

#### Rationale

This milestone avoids writing a type-and-effect compiler before validating the semantic model. Koka is an oracle and bootstrap tool, not necessarily the permanent implementation.

---

### Milestone 2 — Source language parser, modules, and lowering to Koka

#### Question

Can the project expose a much smaller, clearer, Svelte-like application language while delegating initial value/effect checking to Koka?

#### Tasks

1. Choose a provisional file extension and grammar in an ADR. Use `.pw` unless a better non-conflicting choice is found.
2. Implement a lossless parser in Rust with precise source spans and error recovery. Evaluate a hand-written parser plus a lossless tree library; use Tree-sitter for editor support later rather than forcing it to be the authoritative compiler parser.
3. Support initially:

```text
module/import declarations
opaque type declarations
records
tagged unions
generic types
functions
match expressions
query/command declarations
one component/page declaration
HTML-like template region
scoped style region
```

4. Implement name resolution and a simple high-level IR even though Koka remains the semantic checker.
5. Lower domain declarations and functions into generated Koka modules.
6. Generate source maps from Koka diagnostics back to `.pw` source spans.
7. Create deterministic formatter output. Do not preserve multiple style-guide variants.
8. Build compile-pass and compile-fail snapshot tests from the corpus.
9. Implement only enough template parsing to represent static HTML, expressions, conditions, and keyed loops.
10. Require keys for mutable/dynamic lists.
11. Reject a generic effect primitive in application syntax.
12. Add `pw check`, `pw fmt`, and `pw explain` commands.
13. `pw explain` should show preliminary:

```text
value types
effects
resource dependencies
placement constraints
privacy labels
generated artifacts
```

#### Gate

- `pw check examples/accepted/hello.pw` succeeds.
- At least 40 rejected examples report errors at original `.pw` spans.
- Formatting is deterministic and idempotent.
- A simple domain function executes through generated Koka.
- The compiler can print an effect summary without exposing generated-file paths to the user.
- No authored JavaScript or TypeScript is required for the example.

#### Rationale

The project must own the surface language early, otherwise temporary Koka or Marko semantics will leak into the final design.

---

### Milestone 3 — Marko 6 rendering adapter, streaming SSR, and first resumption

#### Question

Can the custom language produce a modern streamed and resumable HTML application before building a renderer from scratch?

#### Tasks

1. Pin a tested Marko 6, Vite, and Node toolchain.
2. Create an adapter that lowers parsed templates to generated Marko files.
3. Keep generated Marko in a build directory, never as the authoring source of truth.
4. Implement:

```text
static elements and attributes
escaped text expressions
conditionals
keyed lists
components
forms and typed events
scoped styles
one delayed streamed region
one interactive event
```

5. Bridge generated Koka JS values into Marko through one typed adapter package.
6. Generate source maps for runtime and build errors.
7. Build `examples/hello-static` and `examples/counter`.
8. Measure for each route:

```text
HTML bytes compressed/uncompressed
CSS bytes
application JS bytes
framework/runtime JS bytes
requests
server render time
browser activation CPU
```

9. For the static route, verify actual zero-JS behavior. If Marko emits unavoidable runtime code, document exactly why and track it.
10. For the counter, verify only the necessary interaction code is sent or loaded.
11. Add Playwright tests in Chromium, WebKit, and Firefox.
12. Add a real Safari smoke-test script using `safaridriver` if feasible without destabilizing the core CI.
13. Validate HTML semantics and baseline accessibility.

#### Gate

- Static page is usable with JavaScript disabled.
- Static page emits zero application JS and ideally zero framework JS; any exception is measured and explained.
- Delayed content streams without blocking the shell.
- Counter interaction works without whole-page re-execution.
- All three Playwright engines pass.
- Generated code is not exposed in normal diagnostics.

#### Rationale

Marko provides a fast way to test streaming and resumption. It remains behind an adapter so it can later be removed.

---

### Milestone 4 — Typed resource model and the first complete store page

#### Question

Can `query`, `command`, `subscription`, `resource`, and structured `task` replace generic effects for a realistic application page?

#### Tasks

1. Add first-class compiler concepts for:

```text
signal
derived
query
command
subscription
resource
task
durable job
```

2. Define a resource manifest schema with:

```text
semantic name
key type
result type
error type
privacy
freshness
consistency
cache partition
timeout
retry
concurrency
invalidation
placement constraints
```

3. Implement a local resource runtime with:

```text
one in-flight request per key
request deduplication
reference-counted subscribers
cancellation when unused
bounded retries with jitter
deterministic test clock
trace events
private/public cache separation
```

4. Implement commands with:

```text
authorization precondition
idempotency key
transaction boundary
optimistic transition
rollback behavior
invalidated resources
explicit recoverable errors
```

5. Build the complete store demo specified later in this prompt.
6. Use SQLite for store, menu, cart, and outbox data.
7. Add delayed recommendations and a user-specific delivery estimate as independent streamed resources.
8. Add duplicate-click and reconnect tests.
9. Add cancellation tests when navigating away.
10. Add a test proving local reactive recomputation does not create another request when the query key is unchanged.
11. Add a deliberately invalid pseudo-`useEffect` program and reject it.
12. Produce causal traces from click to command to resource refresh to DOM part.

#### Gate

- The store page works end to end.
- A rerender/recomputation storm does not exceed one in-flight request per resource key.
- Duplicate `add_to_cart` calls with the same interaction ID produce one logical mutation.
- Navigation cancels unneeded work.
- Private cart state never appears in public cache output.
- All query and command policies are visible in `pw explain`.
- No generic application lifecycle effect exists.

#### Rationale

This is the first milestone that tests the main hypothesis against a real web application rather than language toys.

---

### Milestone 5 — Privacy-flow, cache-safety, and placement checker

#### Question

Can the compiler reject invalid browser/edge/origin placement and cross-tenant or secret data flow before runtime?

#### Tasks

1. Formalize a conservative privacy-label algebra.
2. Add placement variables and constraints to the IR.
3. Derive required capabilities from effects.
4. Implement checks for:

```text
database effect in browser
secret effect in browser or edge without capability
session/user value in shared public cache
private value in public render
secret or private value in public log
browser-only device API on server
nondeterministic build/static render
unserializable handler capture
cross-tenant cache key omission
```

5. Produce diagnostics that explain the complete cause chain:

```text
StorePage public materialization
  depends on CartSummary
  which is Session<abc>
  therefore shared cache is unsafe
```

6. Add explicit escape hatches only under an `unsafe` namespace, with warning, semantic-diff entry, and mandatory justification string.
7. Generate separate browser, edge, and origin manifests even before all targets are independent binaries.
8. Add property-based tests for label joins and placement constraints.
9. Add compile-fail examples for every rule.

#### Gate

- At least 50 placement/privacy compile-fail tests pass.
- The store page splits public store data from private cart data.
- Generated browser output contains no server secret or database capability.
- A shared cache cannot be keyed without every required privacy partition.
- Diagnostics name the source value and invalid boundary, not merely a type mismatch.

#### Rationale

Memory safety alone is not the dominant risk in web applications. Privacy and placement mistakes are more relevant and should be compiler-visible.

---

### Milestone 6 — Materialized resource graph: the ISR successor

#### Question

Can public pages and fragments be incrementally materialized and invalidated from semantic dependencies rather than route-level timers?

#### Tasks

1. Implement a dependency graph for:

```text
resource snapshot
page/fragment materialization
code version
locale
tenant
privacy partition
policy version
```

2. Add a transactional outbox in SQLite.
3. Define typed events such as:

```text
StoreChanged(StoreId)
MenuChanged(StoreId)
InventoryChanged(StoreId, MenuItemId)
PromotionChanged(StoreId)
CartChanged(SessionId)
```

4. Have commands emit events in the same database transaction as state changes.
5. Build a materializer that:

```text
consumes committed events
finds affected resources/fragments
coalesces duplicate invalidations
prevents cache stampedes
regenerates or marks stale
serves last-known-good when policy allows
records causality and duration
```

6. Support time-based freshness only as a fallback or explicit policy, not as the only invalidation mechanism.
7. Implement fragment-level or resource-level regeneration for the store page.
8. Add failure injection:

```text
materializer crash
origin timeout
duplicate event
out-of-order event
regeneration failure
concurrent readers
```

9. Add cache-key auditing to `pw explain`.
10. Benchmark invalidating one store among many and confirm unrelated stores do not regenerate.

#### Gate

- `MenuChanged(store_47)` invalidates only store 47’s relevant resources/fragments.
- Duplicate events are harmless.
- Last-known-good behavior follows declared policy.
- Private cart resources are never written into shared materialization storage.
- Cache stampede tests show bounded regeneration concurrency.
- The resource dependency graph is inspectable and serializable.

#### Rationale

This turns ISR from a framework feature into a semantic consequence of resource dependencies.

---

### Milestone 7 — Own document-parts compiler and browser runtime

#### Question

Can the project replace generated Marko with a smaller renderer whose semantics match the language exactly?

#### Tasks

1. Freeze a renderer-independent golden test suite using the Marko implementation as one oracle.
2. Implement direct server HTML generation from the template IR.
3. Assign stable IDs to dynamic parts without making static HTML noisy.
4. Emit a compact parts manifest only for dynamic regions.
5. Build a minimal browser runtime with:

```text
event delegation
text/attribute/range updates
keyed list operations
resource subscriptions
handler loading
scope cancellation
patch application
version mismatch recovery
frame transaction scheduler
batched layout measurement
batched DOM/style mutation
layout/paint attribution
```

6. Implement content-addressed handler artifacts.
7. Require statically serializable captures.
8. Implement resumption without replaying the complete component tree.
9. Implement out-of-order `<template>`-style patches through a narrow shim.
10. Preserve focus, selection, form state, scroll, and accessibility during updates.
11. Implement the layout-phase contract from section 7.5A:
   - prohibit arbitrary synchronous geometry reads in normal app packages;
   - expose scheduled measurement snapshots;
   - apply document-part writes in one commit phase;
   - deduplicate same-frame measurements;
   - surface an audited escape hatch with runtime attribution;
   - generate containment hints only when proven safe;
   - support virtualized keyed collections.
12. Add tests for:

```text
keyed insertion/reordering
rapid optimistic update/rollback
rapid menu filtering over a 1,000-item fixture with bounded live DOM
cart drawer open/close animation under 60 Hz and 120 Hz profiles
font load, image intrinsic-size change, and viewport resize
streamed patch arriving after navigation
handler version mismatch
focus preservation
form submission without JS
JavaScript disabled
read-write-read layout-thrash attempt rejected or deferred
measurement after mutation delivered in a later frame
ResizeObserver feedback loop contained
1,000-item menu remains virtualized
cart drawer animation avoids layout-triggering properties in the hot path
```

13. Compare output and behavior against Marko.
14. Keep the Marko adapter as a benchmark and fallback until parity is demonstrated.

#### Gate

- Store page runs through the own renderer.
- Static route ships no browser runtime.
- Interactive route loads only required handler/runtime code.
- No full-tree hydration occurs.
- Keyed updates preserve identity and focus.
- All renderer golden tests pass against both Marko and own renderer or documented intentional differences.
- Browser runtime size and activation CPU are measured.
- Standard store-page interactions produce no script-attributed forced synchronous layout in the supported Chromium trace harness.
- Layout measurements and mutations appear as separate runtime phases in debug traces.
- The large-menu benchmark uses bounded DOM size through virtualization or `content-visibility` where semantically correct.

#### Rationale

The custom renderer should be built only after the semantics and test oracle exist. Otherwise the project risks implementing a fast renderer for the wrong model.

---

### Milestone 8 — Rust capability host, WIT worlds, and Wasmtime execution

#### Question

Can language effects become real host-granted capabilities in sandboxed edge/origin components?

#### Tasks

1. Define WIT packages for:

```text
stores
carts
sessions
secrets
clocks
random
telemetry
cache
outbound HTTP
durable jobs
```

2. Create distinct worlds for:

```text
public edge renderer
private edge handler
origin application
background worker
```

3. Implement a Rust host around Wasmtime.
4. Deny ambient filesystem, process, network, environment, and secret access by default.
5. Map compiler effect manifests to allowed imports.
6. Build at least two components:

```text
public store query component
private add-to-cart command component
```

7. Initially write components in Rust if the Koka-to-Wasm Component path is not mature. Keep app semantics generated from the same schema.
8. Test capability denial:

```text
public component attempts arbitrary outbound network
browser-target component requests database
edge component requests payment secret
component requests undeclared filesystem path
```

9. Add per-component resource limits:

```text
fuel or epoch interruption
memory limit
timeout
concurrency limit
request body limit
```

10. Add structured tracing at component boundaries.
11. Evaluate current WASI 0.3 support. Use a documented 0.2 adapter if needed.

#### Gate

- Store read and cart mutation execute through Wasmtime components.
- Undeclared capabilities fail before or at component instantiation with clear diagnostics.
- Resource limits are enforced.
- WIT interfaces are generated or checked from the language’s type declarations.
- The Node host no longer owns privileged business I/O.

#### Rationale

This converts compiler claims into runtime-enforced sandbox boundaries and makes AI-generated server code safer to execute.

---

### Milestone 9 — Permanent value type checker and algebraic effect compiler in Rust

#### Question

Can the project replace Koka as the application compiler while preserving validated semantics and improving web-specific diagnostics?

Split this into submilestones rather than one giant rewrite.

#### Milestone 9A: modules, names, and value types

Implement:

```text
module graph
name resolution
opaque nominal types
records
ADTs
generics
unification-based inference
Option/Result
pattern exhaustiveness
external decoders
```

Differentially test shared language subsets against Koka.

#### Milestone 9B: effect rows and handlers

Implement:

```text
effect declarations
row-polymorphic function effects
effect inference
handler elimination or transformation
pure-view constraints
capability extraction
```

Use Koka programs and generated random terms for differential testing where semantics overlap.

#### Milestone 9C: structured concurrency and affine resources

Implement:

```text
task-scope ownership
cancellation semantics
affine handles
transaction completion
resource acquisition/release checking
```

Keep ordinary data automatically managed.

#### Milestone 9D: privacy and placement integration

Unify value/effect checking with:

```text
privacy labels
serialization constraints
placement solving
cache safety
handler capture safety
```

#### Cross-cutting tasks

1. Use incremental compiler queries so edits recompute only affected results.
2. Build precise, actionable diagnostics with explanations and suggested legal rewrites.
3. Add fuzzing and property-based tests.
4. Ensure malformed source never crashes the compiler.
5. Maintain Koka as a differential oracle until confidence is high.
6. Remove generated Koka from normal builds only after parity gates pass.

#### Gate

- The own compiler accepts the validated accepted corpus and rejects the validated rejected corpus.
- Differential tests match Koka for the common semantic subset.
- Full store demo builds without Koka.
- Incremental checks are fast enough for editor feedback.
- No compiler panic is found in the current fuzz corpus.
- Koka remains optional as a conformance tool, not a build dependency.

#### Rationale

Only after the semantics and real application are stable is a new compiler justified.

---

### Milestone 10 — Own backends and automatic memory strategy

#### Question

Can the permanent compiler emit efficient browser and server artifacts without exposing Rust-like ownership ceremony to application developers?

#### Tasks

1. Define a small typed core IR after effects and placement are resolved.
2. Implement a browser backend in the least risky order:

```text
first: generated modern JavaScript modules for handlers and pure computation
then: Wasm for pure compute-heavy modules
later: Wasm GC or another managed representation when direct Web API integration is practical
```

3. Implement a server backend targeting Wasm Components.
4. Evaluate memory strategies with real benchmarks:

```text
stack allocation
escape analysis
unboxed values
reference counting
region/arena allocation
Wasm GC
Perceus-inspired reuse analysis
```

5. Do not promise Rust-equivalent performance universally.
6. Keep ordinary source free of borrow/lifetime syntax.
7. Expose affine annotations only for scarce resources where they express a real invariant.
8. Add code-size, allocation, latency, throughput, and memory benchmarks.
9. Differentially compare server behavior with generated Koka/Rust reference implementations.
10. Add deterministic serialization/versioning for resumable state.

#### Gate

- Store application builds from source to browser artifacts and Wasm Components without Koka or Marko.
- Semantics match the oracle tests.
- Memory leaks and unbounded resource growth are tested under sustained load.
- Code size and performance are recorded against baselines.
- No ownership syntax is required for ordinary application values.

#### Rationale

The ideal language has automatic memory ergonomics. Implementation strategy should be selected from measurements, not ideology.

---

### Milestone 11 — Multi-node MacBook network lab

#### Question

Can edge, origin, database, and impaired-network behavior be reproduced on the existing M3 Max MacBook without custom hardware?

#### Stage 11A: native-process topology

Run on macOS:

```text
browser
compiler daemon
edge process
origin process
SQLite/PostgreSQL
materializer
```

Validate functionality before virtualization.

#### Stage 11B: Lima VM topology

Use arm64 Linux guests under Apple Virtualization Framework where possible:

```text
macOS host
├── Safari / Chrome / Playwright
├── compiler and devtools
└── Lima network
    ├── edge VM
    ├── origin VM
    ├── database VM
    └── optional router VM
```

Start with conservative allocations:

| VM | vCPU | RAM |
|---|---:|---:|
| edge | 2 | 3 GB |
| origin | 4 | 6 GB |
| database | 2 | 4 GB |
| router | 1 | 1 GB |

Adjust only from measurements.

#### Tasks

1. Install Lima through the documented method and pin the tested version.
2. Use `vz`/Virtualization.framework and `virtiofs` where supported.
3. Generate reproducible VM definitions under `lab/lima/`.
4. Use a network mode that supports direct IP/UDP access for HTTP/3 experiments. Do not depend solely on SSH TCP forwarding.
5. Create local names:

```text
app.test
edge.test
origin.internal
db.internal
```

6. Use `mkcert` or another local CA to create trusted development certificates. Never commit private CA material.
7. Add `just lab-up`, `just lab-test`, and `just lab-down`.
8. Apply Linux `tc netem` profiles. Begin with egress shaping; add an intermediate router/IFB setup for accurate bidirectional tests later.
9. Store profiles as data, not shell fragments only:

| Profile | Added one-way/egress delay | Loss | Rate |
|---|---:|---:|---:|
| ideal | 0–2 ms | 0% | unrestricted |
| nearby-edge | 10–20 ms | 0–0.1% | 100 Mbps |
| typical-mobile | 50–80 ms | 0.5% | 15–30 Mbps |
| poor-mobile | 120–180 ms | 1–2% | 3–8 Mbps |
| distant-origin | 80–150 ms | 0.1–0.5% | 50 Mbps |

10. Seed randomized loss profiles for reproducibility.
11. Measure browser-to-edge and edge-to-origin separately.
12. Add packet captures for debugging, while redacting private data.
13. Test cold cache, warm cache, edge failure, origin failure, and database slowness.

#### Stage 11C: physical devices already owned

After VM tests pass:

```text
iPhone -> Wi-Fi/cellular -> MacBook edge
optional existing PC -> origin/database
```

Measure:

```text
mobile startup CPU
battery impact
memory
interaction responsiveness
network handoff
background/foreground resumption
```

#### Gate

- One command creates the reproducible local topology.
- All network profiles run from scripts and restore cleanly.
- The store page works under every profile.
- Public edge materialization improves distant-origin behavior measurably.
- Private cart correctness survives reconnects and retries.
- The lab can be destroyed without affecting host data.

#### Rationale

The existing Mac is sufficient to test architecture, placement, and network behavior. Custom hardware is not justified before a software-specific bottleneck appears.

---

### Milestone 12 — HTTP/3, prioritized application delivery, and protocol experiments

#### Question

Can the platform improve startup and streaming using existing HTTP/3/QUIC infrastructure without inventing a new transport?

#### Tasks

1. Select an existing HTTP/3-capable proxy/server after a small bakeoff. Candidates may include Caddy or a Rust QUIC/H3 stack; do not implement QUIC.
2. Serve the same application over HTTP/2 and HTTP/3.
3. Ensure TLS certificates and UDP routing work in the Lima lab.
4. Separate and prioritize:

```text
HTML shell
critical CSS
public resource snapshot
private resource snapshot
resume manifest
interaction handler code
delayed patches
live invalidation stream
```

5. Test Early Hints or preloading only where measurements support it.
6. Implement content-addressed handler and schema caching.
7. Use typed binary state only where it materially helps; retain semantic HTML for document structure.
8. Test packet loss, connection migration where available, and stalled independent streams.
9. Record whether browser implementation limits prevent desired prioritization.
10. Write standards-oriented design notes for any missing primitive.

#### Gate

- HTTP/3 path works in the local lab and at least one real browser.
- Results are compared fairly with HTTP/2 under identical profiles.
- Large delayed resources do not block the HTML shell or critical interaction.
- No custom transport protocol was introduced without evidence.

#### Rationale

The likely opportunity is better application-level scheduling on top of existing transport semantics, not replacing the Internet transport stack.

---

### Milestone 13 — Servo and native browser primitive experiments

#### Question

Which compatibility shims are measurably expensive or unsafe enough to justify browser-native primitives?

#### Tasks

1. Build or embed an unmodified current Servo on macOS using official instructions.
2. Load the compatibility implementation of the store app.
3. Profile:

```text
JS/Wasm-to-DOM boundary
parts update cost
handler activation
patch application
startup parsing/execution
capability enforcement gaps
```

4. Select only one measured experiment at a time.
5. Candidate experiments:

```text
native DocumentPart handles
direct typed Wasm DOM binding
native resumable handler table
native declarative range patching
component capability manifest
content-addressed executable cache
causal reactive devtools
```

6. Keep experimental browser changes behind feature flags.
7. Write a minimal standards proposal or explainer for each successful primitive.
8. Compare against ordinary browsers and own JS shim.
9. Do not spend time chasing complete web compatibility.

#### Gate

- At least one browser-native experiment has a reproducible benchmark.
- The measured improvement or safety property is significant enough to justify the added browser complexity.
- A clean fallback exists for current browsers.
- The Servo fork, if any, is narrow and documented.

#### Rationale

Browser changes should solve demonstrated bottlenecks, not merely make the architecture aesthetically pure.

---

### Milestone 14 — Developer tooling, semantic diffs, and AI benchmark

#### Question

Does the new language materially improve human review and AI coding reliability compared with React and contemporary alternatives?

#### Tasks

1. Build an LSP with:

```text
diagnostics
hover types/effects
jump to definition
rename
completion
formatting
resource/placement visualization
```

2. Build causal developer tools that can answer:

```text
why did this request run?
who owns it?
why did this DOM part update?
what invalidated this cache?
why is this code placed at origin?
which private value crossed a boundary?
```

3. Generate a semantic diff for every change:

```text
domain type changes
state-machine changes
new effects
new capabilities
privacy changes
placement changes
cache/freshness changes
invalidation changes
client bytes
server components
tests and obligations
unsafe escape hatches
```

4. Create a benchmark with equivalent tasks in:

```text
React/Next
SvelteKit
new platform
```

5. Use realistic tasks:

```text
add optimistic cart update
fix duplicate request storm
add private resource without cache leak
add an order-state variant
stream recommendations
make a form accessible
cancel stale navigation request
```

6. Grade with deterministic tests and compiler/static checks. Do not use an LLM judge for core correctness.
7. Evaluate agents in isolated repositories with hidden tests and the same time/tool budget.
8. Record:

```text
pass@1
number of compiler iterations
behavioral failures
static-safety failures
new warnings
changed lines
review time
tokens/cost/latency where available
```

9. Train or prompt smaller agents only after the language/toolchain is stable enough that failures are meaningful.
10. Add a human review study based on semantic diffs rather than line count alone.

#### Gate

- Semantic diffs are generated for the store app.
- At least 10 benchmark tasks exist across all three stacks.
- The evaluation is reproducible and does not expose hidden tests.
- Results distinguish model capability from harness effects.
- The project can state with evidence which bug classes became unrepresentable.

#### Rationale

The central claim is not merely that the stack feels elegant. It should measurably reduce the search space for humans and coding agents.

---

### Milestone 15 — Hardening, production research, and standards path

#### Question

What would be required to turn the research platform into a dependable ecosystem?

#### Tasks

1. Security hardening:

```text
threat modeling
fuzzing
sandbox escape review
supply-chain review
CSRF/XSS/SSRF checks
request/body limits
secret redaction
multi-tenant cache tests
capability audit
```

2. Compiler hardening:

```text
fuzz parser/type checker
incremental correctness
reproducible builds
stable diagnostics
migration/versioning
source compatibility policy
```

3. Runtime hardening:

```text
resource limits
backpressure
load shedding
circuit breakers
crash recovery
schema evolution
resume-manifest versioning
rolling deployment compatibility
```

4. Ecosystem research:

```text
minimal package format
content-addressed package identity
signed packages
capability declarations
standard library policy
interop with JavaScript, Rust, and existing HTTP APIs
```

5. Accessibility and internationalization:

```text
screen-reader testing
keyboard and focus rules
locales
writing modes
number/date/currency formatting
translation boundaries
```

6. Production observability and deployment:

```text
OpenTelemetry export
edge/origin rollout
canarying
rollback
cache-version coordination
schema migrations
```

7. Standards work:

```text
browser document-parts explainer
resumption manifest explainer
declarative patch integration
Wasm/Web API binding requirements
capability manifest
HTTP delivery profile
```

8. Publish performance and correctness results with reproducible artifacts.

#### Gate

This milestone does not imply instant production readiness. It passes when the repository contains an evidence-backed roadmap distinguishing:

```text
research prototype
usable developer preview
production pilot
browser experiment
standards proposal
```

and when major unresolved safety and operational risks are explicit.

---

## 15. The canonical store-page validation application

The store application is the main vertical slice. It must remain small enough to understand but rich enough to exercise the architecture.

### 15.1 Domain model

```text
Store {
    id: StoreId
    name: String
    description: String
    hours: StoreHours
    menu_version: Int64
}

MenuItem {
    id: MenuItemId
    store_id: StoreId
    name: String
    description: String
    price: Money<USD>
    available: Bool
    category: MenuCategory
}

Cart {
    id: CartId
    consumer_id: ConsumerId
    version: Int64
    items: List<CartLine>
}

CartLine {
    item_id: MenuItemId
    quantity: PositiveInt
    unit_price: Money<USD>
}

DeliveryEstimate {
    min_minutes: PositiveInt
    max_minutes: PositiveInt
    generated_at: Instant
}
```

### 15.2 Privacy and placement

| Data | Privacy | Expected placement/cache |
|---|---|---|
| store and public menu | Public | edge/shared materialization |
| item availability | Public but short-lived | edge/shared, event invalidated |
| recommendations | Public or experiment-partitioned | streamed, bounded cache |
| delivery estimate | Session/User | private edge/origin result |
| cart | Session/User | private cache, read-your-writes |
| payment methods | Secret/User | origin only; not needed in first demo |
| device location | Device/User | browser capability; never shared |

### 15.3 Page behavior

Route:

```text
/stores/:store_id
```

Initial response should contain:

- semantic store heading;
- menu shell or menu content as early as policy allows;
- cart summary slot;
- delivery estimate slot;
- recommendation slot.

Interactions:

```text
add item
increment quantity
decrement quantity
remove item
retry recoverable failure
navigate away during a slow query
```

### 15.4 Command semantics

`add_to_cart` must declare:

```text
requires SignedIn
idempotent_by InteractionId
transactional write to cart
optimistic local transition
rollback on rejected mutation
invalidates private cart resource
revalidates item availability before commit
bounded retry only for safe transport failures
```

### 15.5 Delays and failure injection

Provide deterministic test controls:

```text
recommendations delay: 1200 ms
estimate delay: 400 ms
store delay: configurable
cart delay: configurable
one-shot database error
one-shot network error
forced stale item
forced duplicate click
forced reconnect
materializer failure
```

### 15.6 Required tests

1. Public store page works without JavaScript for reading content.
2. Static public shell does not contain private cart data.
3. Recommendations stream after the core content.
4. Duplicate clicks with one interaction ID create one cart mutation.
5. Two different interaction IDs create two intentional mutations.
6. Same query key under repeated local recomputation makes one request.
7. Changing the query key cancels or supersedes stale work.
8. Navigating away cancels unneeded requests.
9. Optimistic cart state rolls back on a rejected mutation.
10. Item becoming unavailable during mutation produces a typed error and consistent cart.
11. `MenuChanged(store_47)` invalidates store 47 only.
12. User A’s cart can never be observed by User B.
13. Shared cache files contain no session or secret fields.
14. Keyboard and screen-reader semantics remain valid.
15. Focus is preserved during incremental updates.
16. Handler version mismatch recovers through safe reload or refetch.
17. Slow recommendations do not block the Add button.
18. Origin failure follows last-known-good policy only for declared public data.

---

## 16. Accepted and rejected language corpus

The corpus is the executable specification. Add examples before features.

### 16.1 Accepted categories

```text
pure calculation
exhaustive order-state rendering
public store query
private cart query
idempotent command
scoped WebSocket subscription
map widget resource with cleanup
streamed public recommendations
edge materialized menu
origin-only secret operation
offline draft with explicit conflict policy
decoder handling malformed JSON
build-time deterministic page
content-addressed resumable handler
```

### 16.2 Rejected categories

Create at least one minimal file for each:

```text
network request inside view
database access in browser
secret serialized to browser
private cart in shared cache
cross-tenant cache key missing tenant
public log of secret value
unhandled ADT variant
ambient null assumption
unchecked external cast
nonserializable handler capture
open transaction not committed or rolled back
affine resource leaked
detached ordinary task
retry of non-idempotent command
unbounded retry
nondeterministic static render
wall-clock read in shared deterministic materialization
unkeyed mutable list
invalid HTML nesting
button behavior implemented on inaccessible div
label missing from form control
form handler type mismatch
dead internal route
raw unsafe HTML without capability
browser-only API on origin
server secret referenced by edge artifact
query key changes without stale-work cancellation policy
subscription outlives component scope
optimistic state with no rollback path
private data in resume manifest
unsafe escape hatch without justification
```

### 16.3 Diagnostic quality

Every rejection should aim for:

```text
what rule was violated
where the value/effect originated
which boundary made it invalid
one or more legal alternatives
relevant inferred type/effect/label
```

Bad diagnostic:

```text
type mismatch
```

Good diagnostic:

```text
Cannot materialize `StorePage` in a shared public cache.

`StorePage` reads `cart`, whose result is labeled `Session<SessionId>`.
A shared cache may contain only `Public` values.

Move `cart` into a private streamed slot, or change this materialization to a session-partitioned cache.
```

---

## 17. Testing strategy

### 17.1 Test pyramid

Use all of these as the project grows:

```text
parser golden tests
formatter idempotence tests
name/type/effect unit tests
compile-pass tests
compile-fail tests
property-based tests
fuzz tests
differential tests against Koka/Marko
runtime unit tests
browser integration tests
network-lab tests
security tests
performance regression tests
AI-agent task evaluations
```

### 17.2 Compiler tests

- Snapshot AST/HIR only where stable and useful.
- Prefer semantic assertions over brittle full snapshots.
- Fuzz malformed source.
- Generate random well-typed and ill-typed terms for inference tests.
- Differentially compare with Koka only on clearly shared semantics.
- Test incremental invalidation of compiler queries.
- Ensure compiler errors never expose generated Koka/Marko internals without an explicit debug flag.

### 17.3 Browser tests

Use Playwright for Chromium, WebKit, and Firefox. Add real Safari smoke tests separately.

Test:

```text
JavaScript disabled
slow network
offline transition
back/forward cache
focus and selection
forms
screen sizes
reduced motion
keyboard-only use
streamed patches
version mismatch
multiple tabs
session isolation
forced synchronous layout detection
measurement/mutation phase ordering
ResizeObserver feedback loops
large-list virtualization
font and image intrinsic-size changes
viewport resize and orientation changes
60 Hz and 120 Hz frame pacing where hardware supports it
```

### 17.4 Accessibility

Use deterministic automated checks plus manual semantic review. Automated tests cannot prove complete accessibility.

At minimum:

```text
semantic controls
labels
keyboard activation
focus order
live-region behavior
contrast/token validation
reduced-motion behavior
screen-reader smoke test documentation
```

### 17.5 Security

Test:

```text
XSS escaping
unsafe HTML boundary
CSRF for commands
session fixation
cross-user cache leakage
secret serialization
SSRF capability restrictions
SQL parameterization
request/body limits
retry amplification
cache poisoning
resume-manifest tampering
handler-hash mismatch
Wasm component capability denial
```

### 17.6 Failure injection

Build deterministic failure controls into local handlers. Do not rely only on random chaos.

---

## 18. Performance and correctness benchmarks

### 18.1 Baselines

Implement functionally equivalent, minimally idiomatic versions of the store page in:

```text
current stable Next/React
current stable SvelteKit
current stable Marko 6
new platform
```

Pin exact versions and commit lockfiles. Do not intentionally sabotage baselines. Record architectural differences honestly.

### 18.2 Metrics

Measure:

```text
cold transferred bytes
HTML/CSS/JS/Wasm bytes
startup code parsed and executed
server time to first byte
time to visible store heading
time to visible menu
time to usable Add button
FCP/LCP/INP where meaningful
main-thread CPU
memory
frame-time distribution at 60 Hz and 120 Hz where supported
script-attributed forced style/layout duration
number and duration of layout/reflow events
style recalculation, layout, paint, raster, and composite time where tooling exposes them
DOM node count and maximum live rendered collection size
layout-shift score for interactions where stability is expected
number of requests
duplicate requests
query cancellation latency
server CPU per render
cache hit/miss
materialization work
invalidation fanout
origin requests avoided
component cold-start time
compiler cold and incremental time
```

### 18.3 Correctness metrics

```text
compile-time rejected bug classes
behavioral test pass rate
static-safety test pass rate
cross-user isolation
retry/idempotency correctness
resource leaks
unbounded task count
invalid cache entries
accessibility failures
```

### 18.4 Initial aspirational budgets

Treat these as targets to test, not facts to fake:

```text
static page: 0 application JS and 0 runtime JS
interactive runtime: single-digit KB compressed if feasible
one in-flight request per resource key by default
no whole-tree hydration
unrelated resource change: no unrelated DOM updates
unrelated store invalidation: no unrelated regeneration
normal app code: no direct synchronous layout-read API
standard interactions: no script-attributed forced synchronous layout
frame transaction: measurements precede mutations; fresh post-mutation geometry arrives in a later transaction
large menu: bounded live DOM through virtualization/windowing
60 Hz target: p95 interaction animation frame under 16.7 ms on the named test device
120 Hz stretch target: p95 interaction animation frame under 8.3 ms on supported ProMotion hardware
compiler feedback: interactive after warm cache
```

If a target is missed, record why and decide whether semantics, implementation, or target should change.

### 18.5 Reproducibility

Every benchmark result must record:

```text
commit
machine and OS
browser/version
network profile
warm/cold state
sample count
median and distribution
commands
raw result file
```

Do not publish one-run anecdotes as conclusions.

---

## 19. AI-agent evaluation and review model

The project exists partly to reduce the amount of tacit framework knowledge an agent must remember.

### 19.1 Benchmark design

For each task, provide agents only:

```text
base repository
issue-style request
normal compiler/test tools
```

Keep hidden:

```text
reference solution
held-out behavioral tests
specific static findings being targeted
```

Grade only deterministic outcomes:

```text
behavioral tests
compiler/static checks
performance budget where stable
security/privacy checks
```

### 19.2 Semantic PR review

Generate a report like:

```text
Domain changes
- Added `OutOfStock` to `CartItemState`

Effects added
- `database.write<Carts>` in `add_to_cart`
- no new browser effects

Capabilities added
- origin receives `carts.write`
- browser receives none

Privacy changes
- none

Cache changes
- `Menu` now invalidates on `InventoryChanged(store_id, item_id)`

Client impact
- one lazy handler
- +1.8 KB compressed
- no startup code added

Unsafe changes
- none

Behavioral obligations
- unavailable item rejected
- duplicate interaction remains idempotent
- optimistic state rolls back on typed failure
```

Human review should focus on product meaning, permissions, consistency, and tests rather than mechanically rediscovering compiler-enforceable rules.

### 19.3 Do not optimize for one model

The language should help multiple agents and humans. Avoid syntax or workflows that only one current proprietary model handles well.

---

## 20. Risk register and fallback paths

Maintain and update these risks.

### Koka interoperability is insufficient

Fallback:

- keep Koka only as a semantic oracle for isolated tests;
- implement the own minimal type/effect checker earlier;
- do not let Marko/browser work block on perfect Koka JS interop.

### Koka lacks structured metadata

Fallback:

- generate explicit metadata from the custom front end;
- use compiler-output parsing only temporarily;
- consider a narrow structured-diagnostics fork after evidence.

### Marko 6 APIs are unstable or incompatible

Fallback:

- pin a tested version;
- keep a tiny adapter;
- use Marko only as a reference implementation;
- implement direct streamed HTML earlier if necessary.

### Resumption serialization is fragile across deployments

Mitigations:

```text
content-addressed handler IDs
code-versioned manifests
strict serializable captures
stale-manifest detection
safe reload/refetch path
rolling-deployment compatibility tests
```

### WASI 0.3 support lags

Fallback:

- use stable WASI 0.2/component tooling;
- isolate version-specific bindings;
- maintain conformance tests and migration ADR.

### Privacy type system becomes too complex

Fallback:

- start explicit and conservative;
- require annotations at shared caches and external boundaries;
- add inference only after diagnostics are understandable.

### Automatic invalidation inference is unsound or opaque

Fallback:

- keep explicit typed events and transactional outbox as the source of truth;
- introduce automatic tracking only as an auditable optimization.

### Browser Wasm cannot efficiently access DOM/Web APIs

Fallback:

- use a minimal generated JS shim;
- compile pure computation to Wasm;
- reserve direct DOM binding for Servo/standards experiments.

### Multi-agent implementation creates architecture drift

Mitigations:

```text
one integrator
separate worktrees
non-overlapping assignments
mandatory ADR reading
small merges
full gate tests
```

### macOS/arm64 hides Linux deployment issues

Mitigations:

```text
Linux CI
arm64 Linux VMs
case-sensitive checks
container/component reproducibility
```

### Fine-grained updates are mistaken for layout safety

Risk: the renderer minimizes DOM writes but application or third-party code alternates geometry reads with layout-invalidating writes, causing forced synchronous layout and poor frame pacing.

Mitigation:

- encode measurement and mutation as separate effects/phases;
- deny direct synchronous geometry APIs in normal app packages;
- build forced-layout instrumentation in Milestone 0;
- require layout-specific regression gates in the store demo;
- use containment and virtualization only when semantically safe;
- retain an audited escape hatch rather than pretending every imperative widget can be statically proven.

Fallback:

- keep the phase scheduler as a runtime-enforced API even if compile-time proof is initially incomplete;
- quarantine third-party widgets behind a measurable imperative-resource boundary.

### Performance work compromises semantics

Rule:

- never weaken a guarantee silently;
- require an explicit unsafe or policy boundary;
- benchmark before and after;
- record the semantic cost.

---

## 21. Definition of success

### Research prototype success

The project is a successful research prototype when:

- one readable source language builds the store application;
- views are pure;
- generic lifecycle effects are absent;
- ADTs, `Option`, `Result`, and exhaustive matching work;
- query/command/subscription/resource semantics work;
- placement/privacy/cache errors are compile-time failures;
- static HTML streams and interactive code resumes without general hydration;
- public materialization and private cart data coexist safely;
- edge/origin components run under explicit capabilities;
- the entire system is reproducible on the MacBook;
- performance and AI benchmarks exist.

### Developer preview success

Add:

- own compiler, no required Koka;
- own renderer, no required Marko;
- usable diagnostics and LSP;
- stable local runtime;
- versioned manifests;
- documentation sufficient for another engineer.

### Production pilot success

Add:

- security review;
- fuzzing and operational limits;
- PostgreSQL and multi-node deployment;
- migrations and rolling deploy compatibility;
- observability and rollback;
- dependency and package security;
- real application pilot.

### Browser/standards success

Add:

- measured Servo experiments;
- browser-independent explainers;
- compatibility fallback;
- engagement with relevant standards communities.

Do not collapse these levels into one vague “done.”

---

## 22. Immediate first actions

On receiving this prompt, do the following now:

1. Inspect the current directory and Git state.
2. If the repo is empty, initialize the minimal repository.
3. Save this prompt as `PROJECT_CHARTER.md` exactly, then create concise `AGENTS.md` and `CLAUDE.md` pointers.
4. Create `docs/STATUS.md` with Milestone 0 active.
5. Run read-only environment detection commands.
6. Create `just doctor` before installing optional dependencies.
7. Research and pin the smallest toolchain needed for the four Milestone 0 spikes.
8. Create the initial technology matrix and ADR skeletons.
9. Create the first accepted/rejected corpus files.
10. Implement and run the feasibility spikes one at a time.
11. Do not proceed to Milestone 1 until the Milestone 0 gate is evaluated.
12. At the end of the session, update `docs/STATUS.md`, `docs/NEXT.md`, and `docs/KNOWN_LIMITATIONS.md`, then commit the coherent work locally.

Your first response/output to the user should summarize:

```text
what environment was found
what files were created
which Milestone 0 task is in progress
what objective evidence exists so far
what is blocked, if anything
exact next action
```

Do not answer with another high-level plan only. Begin executing Milestone 0.

---

# END MASTER PROMPT
