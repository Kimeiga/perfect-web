# The Web, Recompiled — Proof Roadmap and Launch Strategy

> Working title. This document is a product, research, demonstration, and communication plan for a clean-slate web application platform that compiles one semantic application into browser, edge, and origin artifacts.

## 1. The project should be launched as a ladder of proofs

The project is too broad to introduce as “a new language,” “a new framework,” or “a faster renderer.” Each description makes it sound narrower than it is, while “a perfect new Internet” sounds too large to believe.

The public story should instead be:

1. Modern web applications depend on many rules that the toolchain cannot express.
2. Those rules are remembered probabilistically by developers, reviewers, linters, tests, and coding models.
3. We move the rules into a semantic compiler.
4. Each milestone provides an observable proof that one class of failure or waste has disappeared.
5. The final application proves that all of the pieces compose in a realistic system.

The central line is:

> **The web should not depend on remembering the right folklore.**

A secondary line is:

> **Describe what the application means. Let the platform decide how it runs.**

## 2. What must be proved

The project makes six distinct claims. They must be evaluated separately so that a fast demo cannot disguise weak safety, and a safe toy language cannot disguise poor usability.

| Claim | What must be shown | Evidence |
|---|---|---|
| Correctness | Common lifecycle, state, serialization, and boundary errors are rejected before deployment | Negative compiler corpus; reproduced real-world failure patterns |
| Performance | Pages do less work and ship less code without manual framework tuning | Browser traces; transferred bytes; startup CPU; responsiveness under constrained networks |
| Security and privacy | Code receives only declared capabilities, and private data cannot enter public artifacts | Compile failures; generated capability manifests; runtime sandbox tests |
| Resilience | Retries, cancellation, invalidation, stale fallback, and task lifetimes behave predictably under faults | Fault-injection scenarios; deterministic traces; duplicate-command tests |
| Developer simplicity | Developers state policy rather than coordinating framework mechanisms | Side-by-side implementations; task completion time; authored concepts and code surface |
| AI suitability | Smaller or cheaper models can produce acceptable changes because invalid choices are mechanically rejected | A reproducible agent benchmark; pass rate, token use, repair iterations, and review time |

## 3. The milestone demonstrations

### Milestone 0 — The executable thesis

**Build**

A browser-based compiler playground with a small language core, algebraic data types, effect rows, and the first web-specific rules. It does not need rendering yet.

**Public demo: “The Bug Museum”**

A gallery of small programs divided into:

- compiles;
- fails for a useful reason;
- compiles in TypeScript/React but fails here.

Initial exhibits:

- network request inside render;
- an unhandled state-machine variant;
- private session data in a shared cache;
- browser code importing a database capability;
- a secret serialized to the client;
- a task escaping its owner scope;
- an open transaction with no commit or rollback;
- retrying a non-idempotent command without an interaction key;
- a non-serializable handler capture;
- a form whose handler accepts a different type than the form can produce.

**The impressive moment**

The visitor edits a safe example into a dangerous one and sees a domain-specific error such as:

```text
error E4102: `cart` is private to Session

`StorePage` is materialized in a shared public cache.
A public artifact cannot depend on `Private<Session, Cart>`.

help: render the cart in a private streamed slot
help: or change the page cache policy to private
```

**What it validates**

The project has a coherent semantic center. It is not merely a syntax mockup.

**Success criteria**

- Every invalid example fails for the intended reason.
- Diagnostics explain the application concept, not compiler internals.
- Accepted examples remain accepted under incremental edits.
- The corpus is versioned and becomes the language specification.

---

### Milestone 1 — The page that ships only what it uses

**Build**

The first semantic HTML component compiler, static document parts, local reactive values, scoped CSS, server rendering, and a tiny browser runtime.

**Public demo: “Zero Means Zero”**

One source file produces three routes:

1. A fully static article that sends **zero application JavaScript**.
2. A product page with one counter whose code is isolated to that interaction.
3. A streamed page whose slow recommendation section does not block the useful document.

A live panel shows:

- HTML bytes;
- CSS bytes;
- startup JavaScript/Wasm bytes;
- code executed before first interaction;
- document parts that can update;
- server and browser execution traces.

**The impressive moment**

The visitor clicks “Add.” The browser loads only the content-addressed handler needed by that button, updates one document part, and never reruns the page component to reconstruct initial state.

**What it validates**

The UI compiler can preserve semantic HTML while avoiding virtual-DOM reconciliation and conventional whole-tree hydration.

**Important framing**

Zero-JavaScript static pages and resumability already exist in individual frameworks. The claim is not that each mechanism is new. The claim is that the mechanism is generated from the same sound language and resource model that will later govern effects, privacy, placement, and caching.

**Success criteria**

- Static route: 0 application JS.
- Interactive route: no initial component replay in the browser.
- Only the changed document part mutates after interaction.
- All routes retain links, forms, selection, accessibility semantics, and no-script fallback where meaningful.

---

### Milestone 2 — The outage pattern that cannot compile

**Build**

Typed `query`, `command`, `subscription`, `resource`, and structured-task primitives. Rendering remains pure. Queries are keyed, deduplicated, cancellable, and policy-bearing.

**Public demo: “The Bug That Compiled”**

Reconstruct the failure pattern described in Cloudflare’s September 12, 2025 postmortem: a request managed by a React effect reruns because an object dependency receives a new identity. Run the React reproduction against a deliberately fragile test service so visitors can watch request amplification.

Beside it, show the new system:

```text
private query organizations(account: AccountId) {
    freshness 30s
    concurrency one_per_key
    retry exponential(max: 3, jitter: true)
}
```

There is no render-triggered request callback and no dependency array. The resource key is `AccountId`, not an incidental object identity.

**The impressive moment**

A “rerender storm” control sends 1,000 irrelevant UI changes. The React reproduction amplifies requests; the typed query remains one logical in-flight request per key.

**What it validates**

The language and framework jointly remove a lifecycle category that an ordinary memory-safe language would not catch.

**Success criteria**

- Render cannot directly start network, storage, clock, or random effects.
- Repeated reads of one query key are deduplicated.
- Leaving the owner scope cancels unnecessary work.
- Retry policy is visible, bounded, and unavailable for unsafe commands unless explicitly justified.
- A trace can explain why every request exists.

---

### Milestone 3 — The cache that refuses secrets

**Build**

Privacy labels, cache classes, server/browser/edge/origin placement constraints, generated serialization, and capability manifests.

**Public demo: “Make It Leak”**

Give the visitor a small store page with:

- public menu data;
- a session-private cart;
- an origin-only payment token;
- device-private location.

Ask them to make the whole page globally cacheable. The compiler rejects the private dependencies and offers safe decompositions:

- public shell plus private streamed slot;
- private page;
- client-only device region;
- origin-only command.

Then expose the generated deployment graph:

```text
Browser
  imports: cart.read, location.read

Edge
  imports: store.read, public-cache

Origin
  imports: cart.write, payments.secret, database
```

**The impressive moment**

The visitor cannot create the classic “personal data leaked through a shared cache” architecture without crossing an explicit audited escape hatch.

**What it validates**

Placement and privacy are first-class program properties rather than comments attached to routes.

**Success criteria**

- Private values cannot influence public cached output.
- Server secrets cannot be serialized into browser artifacts.
- Browser modules cannot import origin capabilities.
- The runtime refuses undeclared capabilities even if a compiler bug or malicious component requests them.
- Every deployment artifact has a readable manifest.

---

### Milestone 4 — The marketplace under a hostile network

**Build**

The first complete browser-compatible vertical slice: routing, streaming, forms, commands, optimistic state, edge materialization, origin services, SQLite/PostgreSQL, and a reproducible local network lab.

**Public demo: “Chaos Checkout”**

Build the same realistic local-marketplace flow in:

- Next/React;
- SvelteKit;
- the new platform.

The feature spec is identical:

- browse stores;
- open a menu;
- add and edit cart items;
- apply a promotion;
- choose an address;
- submit a simulated order;
- watch status updates.

A public “network lab” can apply:

- 120 ms latency;
- 2% packet loss;
- 3 Mbps bandwidth;
- temporary origin failure;
- delayed recommendations;
- duplicated mutation delivery;
- connection loss during checkout.

**The impressive moments**

- Useful HTML arrives while noncritical sections remain pending.
- The page remains navigable before optional handler code loads.
- A repeated order submission with one interaction ID creates one business effect.
- Public store pages continue from last-known-good edge materializations during origin trouble.
- Failed private actions degrade explicitly rather than silently pretending success.
- The trace tells the visitor exactly which policy produced each behavior.

**What it validates**

The ideas compose into an application rather than isolated compiler tricks.

**Measurements**

- compressed transferred bytes by resource type;
- startup main-thread CPU;
- first useful content;
- first successful interaction;
- interaction latency under load;
- duplicate request and mutation counts;
- server work per navigation;
- number of manual cache, retry, and lifecycle mechanisms in authored code.

Do not preannounce victory thresholds. Publish the full harness and let results determine the claim.

---

### Milestone 5 — One change, one recomputation

**Build**

Typed invalidation events, dependency tracking, public materialized document fragments, last-known-good behavior, and deterministic incremental recomputation.

**Public demo: “Change One Menu”**

A merchant changes the availability or price of one item. A visual graph shows:

```text
MenuItem(47)
   -> StoreMenu(store_12)
      -> MenuListPart(store_12)
      -> SearchResultCard(store_12)
```

Only affected resources and fragments are updated. Other stores, private carts, and unrelated page sections remain untouched.

The user can compare this with:

- time-based route regeneration;
- broad tag invalidation;
- a full application refetch.

**The impressive moment**

The site displays the exact causal path from one database/event change to the specific cached fragments and live clients updated because of it.

**What it validates**

“ISR” has become a consequence of a typed dependency graph rather than a route-level timer chosen by hand.

**Success criteria**

- No unaffected fragment regenerates in the controlled test.
- A dependency can be explained and inspected.
- Cycles and ambiguous invalidation policies are diagnosed.
- Private and public dependency graphs remain separated.
- Failed regeneration preserves a known-good public result according to declared policy.

---

### Milestone 6 — Four roles, one domain model

**Build**

Expand the marketplace into four connected applications:

- consumer;
- merchant;
- courier;
- operations/support.

Add explicit state machines, live subscriptions, offline work, permissions, and audit trails.

**Public demo: “The Marketplace”**

A single simulation contains:

- a merchant changing a menu;
- a consumer placing an order;
- a courier accepting it while briefly offline;
- operations observing and intervening;
- all views updating according to their capabilities and consistency policies.

**The impressive moments**

- An invalid order transition cannot compile.
- A courier cannot invoke a merchant-only command because the capability is absent.
- An offline courier action queues only when its conflict policy is declared.
- Reconnection produces a deterministic resolution rather than arbitrary last-write-wins behavior.
- Operations sees an audit trace derived from the same semantic graph.

**What it validates**

The language can represent a real business system with public, private, realtime, offline, transactional, and operational concerns.

**Why this is the flagship**

A marketplace is unusually demanding without requiring proprietary company data. It includes the classes of problem found in commerce, logistics, banking, travel, collaboration, and enterprise operations.

---

### Milestone 7 — The AI proof

**Build**

A benchmark with realistic repository tasks, a compiler repair protocol, semantic PR output, and reproducible agent harnesses.

**Public demo: “Small Model, Strong System”**

Run the same feature requests against:

1. frontier model + React implementation;
2. frontier model + the new platform;
3. progressively smaller local models + the new platform.

Tasks should include:

- add a loading/error state;
- preserve a controlled input;
- add an optimistic cart mutation;
- create a live subscription;
- change cache freshness;
- add a capability-limited integration;
- prevent duplicate submissions;
- add an accessible dialog;
- modify a state machine;
- repair an intentionally unsafe patch.

The benchmark must check:

- behavioral tests;
- compiler and effect correctness;
- accessibility;
- resource lifecycle;
- privacy and placement;
- performance budget;
- semantic-diff review criteria.

**The impressive moment**

A laptop-sized local model produces an acceptable feature because the language narrows its choices and the compiler gives deterministic, high-level feedback. The visitor can inspect every failed iteration and see which invalid possibilities were removed mechanically.

**What it validates**

The platform is not merely easier for expert language designers; it reduces the probabilistic knowledge burden of ordinary coding agents.

**Measurements**

- first-pass and final pass rate;
- number of compile/repair cycles;
- input and output tokens;
- elapsed tool steps;
- model size and local memory requirement;
- semantic issues found in review;
- human review time;
- total accepted diff size.

The goal is not to assert in advance that a 7B model will beat a frontier model. The goal is to find the smallest model that reaches a defined quality bar and publish the honest curve.

---

### Milestone 8 — Native mode

**Build**

Embed or fork Servo only after the ordinary-browser implementation is mature. Replace selected compatibility mechanisms with native document parts, direct typed Wasm/browser interfaces, resumable-handler registration, or declarative patch streams.

**Public demo: “Remove the Shim”**

Run the same marketplace route in:

- ordinary Safari/Chrome compatibility mode;
- the experimental browser-native runtime.

Show exactly what disappears:

- JavaScript glue;
- framework runtime bytes;
- document-part lookup work;
- handler registration work;
- serialization metadata that native primitives no longer require.

**The impressive moment**

The source application and semantic trace are unchanged. Only the execution substrate changes, demonstrating that the compiler model is independent of current browser accidents.

**What it validates**

There is a credible path from deployable framework to web-platform proposal rather than a permanent proprietary island.

---

### Milestone 9 — The adoption proof

**Build**

Interop and incremental migration:

- embed a generated component in an existing React or SvelteKit page;
- call TypeScript packages behind typed boundary adapters;
- invoke Rust/Wasm components through generated interfaces;
- deploy one route or feature without rewriting the host application.

**Public demo: “Replace One Dangerous Feature”**

Take a realistic existing application route with difficult effect, caching, or mutation behavior. Replace only that feature and compare defects, authored code, runtime cost, and review surface.

**What it validates**

The project can enter the world incrementally. A clean-slate architecture without a migration bridge is a research artifact, not a plausible ecosystem.

## 4. What the final public showcase should contain

The launch site should expose five connected experiences rather than one promotional video.

### A. The Bug Museum

A live compiler playground built around real classes of failure. Each exhibit contains:

- the familiar implementation;
- the reason it passes ordinary compilation;
- the new representation;
- the compiler rejection or generated safe behavior;
- a source/evidence link;
- a runnable test.

### B. The Marketplace

The complete four-role application. It is the emotional proof that the system can build something substantial and polished.

### C. The Network Lab

A side-by-side Next/SvelteKit/new-platform comparison under reproducible latency, bandwidth, packet-loss, and failure profiles. Every number is downloadable.

### D. The AI Arena

A public task browser with model rollouts, compiler feedback, accepted patches, semantic diffs, costs, and reproducibility instructions.

### E. The Architecture Explorer

An interactive graph for any route:

```text
source declaration
  -> inferred effects
  -> privacy labels
  -> placement decision
  -> cache/materialization policy
  -> generated browser/edge/origin artifacts
  -> network streams
  -> document parts
```

This is the most important explanatory tool because the project spans too many domains to understand from prose alone.

## 5. The one-page landing-site narrative

The landing page should move from a concrete, undeniable problem to the full system. It should not begin with programming-language theory.

### Section 1 — Hero: the web should be harder to get wrong

**Headline**

> The web should be harder to get wrong.

**Subhead**

> One readable language compiles application intent into semantic HTML, resumable browser code, edge materializations, and origin services—while rejecting effect, privacy, placement, and state bugs before deployment.

**Primary action**: Run the impossible-bug demo  
**Secondary action**: Read the technical paper

Beside the copy, show the smallest convincing proof: a request-in-effect pattern that compiles in React and a keyed query declaration that cannot be coupled to render identity.

### Section 2 — The bug compiled

Tell the Cloudflare incident pattern briefly and carefully. The lesson is not “React caused an outage.” The lesson is:

> The most important lifecycle rule existed outside the programming language.

Show how service capacity and retry behavior amplified the frontend trigger, so the project does not pretend a language alone prevents distributed outages.

### Section 3 — Developers are acting as the compiler

Display the hidden contracts a developer must coordinate:

- object identity;
- render purity;
- effect timing;
- cancellation;
- retries;
- idempotency;
- serialization;
- public versus private caching;
- server/client placement;
- invalidation;
- accessibility;
- state-machine completeness.

Then show where those rules currently live:

```text
memory -> style guide -> linter -> reviewer -> tests -> incident
```

### Section 4 — Move rules down the stack

An animated ladder:

```text
Convention
    ↓
Linter
    ↓
Type
    ↓
Capability
    ↓
Impossible representation
```

This is the conceptual bridge from the problem to the language.

### Section 5 — One language for application meaning

Introduce only the readable surface concepts:

- algebraic states;
- explicit failure;
- pure view functions;
- typed effects;
- capabilities;
- structured tasks.

Avoid a type-theory lecture. Show one impossible-state example and one effect signature.

### Section 6 — One application graph

Introduce:

- local state;
- query;
- command;
- subscription;
- resource;
- materialized public view.

A diagram should show that the compiler knows what data is read, who may see it, how fresh it must be, and what changes it.

### Section 7 — One compiler, three places

Animate a single source declaration splitting into:

- browser;
- edge;
- origin.

Show placement as a result of privacy, capability, consistency, and latency policy—not `"use client"`, route suffixes, or manually paired endpoints.

### Section 8 — HTML without replay

Explain the rendering model visually:

- semantic HTML is streamed;
- static content remains plain HTML;
- dynamic expressions become stable document parts;
- only necessary state is resumable;
- handlers load on demand;
- no virtual-tree replay is required.

Acknowledge Marko and Qwik as important predecessors. The new claim is their integration with a sound application and distributed-resource model.

### Section 9 — Proof, not promises

Present the public proof cards:

- outage pattern;
- zero-JS route;
- privacy-safe cache;
- chaos checkout;
- incremental invalidation;
- AI benchmark;
- native browser mode.

Each card must show current status: planned, prototype, reproducible, or independently verified.

### Section 10 — The flagship marketplace

Use a polished product screenshot or live embed and explain why the four roles exercise the platform.

### Section 11 — Roadmap

Use five public phases rather than every internal milestone:

1. Semantics and compiler corpus.
2. Browser-compatible UI and resources.
3. Distributed marketplace and network lab.
4. AI benchmark and production pilot.
5. Browser-native experiments and standards work.

### Section 12 — Open research invitation

Invite four different audiences with distinct actions:

- application developers: try the playground;
- PL researchers: critique the semantics;
- browser/runtime engineers: inspect the execution model;
- AI researchers: run the benchmark.

### Section 13 — Paper and reproducibility

Link to:

- web-readable paper;
- Markdown source;
- PDF release;
- benchmark data;
- compiler source;
- architectural decision records;
- citation metadata.

## 6. Visual and interaction design

### Tone

Serious systems research with a human interface—not crypto futurism, not a generic AI startup, and not a documentation site pretending to be a launch page.

### Visual system

- Near-black technical canvas with warm off-white text.
- One electric blue accent for data flow and one amber accent for warnings.
- Large editorial sans-serif headlines using system fonts.
- Monospace only for code, labels, and measurements.
- Thin grid lines and topology diagrams instead of stock illustrations.
- Generous vertical rhythm so the conceptual escalation remains understandable.
- Motion only when it explains causality or placement.

### Signature interaction

A sticky architecture rail follows the reader down the page. It begins as tangled modern-web boxes, then progressively resolves into one semantic graph and finally splits cleanly across browser, edge, and origin.

### Trust signals

- Never display benchmark numbers without downloadable runs.
- Label mockups and targets clearly.
- Put limitations beside claims, not in a hidden FAQ.
- Link every incident or predecessor claim to a primary source.
- Preserve a visible “What this does not solve” section.

## 7. White-paper plan

### Recommended title

**The Web, Recompiled: A Semantic Application Platform for Browser, Edge, and Origin**

### Document types

Maintain one source tree but generate three readings:

1. **Landing page** — 5–10 minute conceptual journey.
2. **Technical paper** — rigorous architecture, semantics, and evaluation.
3. **Reference documentation** — precise language and runtime behavior.

Do not force the white paper to act as the language reference.

### White-paper structure

1. Abstract
2. The accidental complexity of modern web applications
3. Failure taxonomy
4. Design goals and explicit non-goals
5. Core language
6. Effects, capabilities, and structured concurrency
7. Application resources and state
8. Privacy and information flow
9. Placement across browser, edge, and origin
10. Rendering, document parts, streaming, and resumption
11. Materialized views, caching, and invalidation
12. Offline and consistency policies
13. Component ABI and runtime sandbox
14. Network mapping
15. Compiler architecture
16. AI-assisted development and semantic review
17. Security model
18. Evaluation methodology
19. Related systems
20. Limitations and unresolved research questions
21. Adoption and interoperability
22. Governance and standardization path
23. Conclusion

### Publication sequence

**Before empirical results**

Publish a clearly labeled design draft in Markdown on the project site and repository. Treat it as an evolving RFC, not a scientific result.

**At the first end-to-end release**

Generate a versioned PDF, archive the software and paper release, publish full benchmark artifacts, and assign persistent citation metadata.

**After reproducible evaluation**

Turn the design document into a research paper with measured results and related-work analysis. Submit to an appropriate programming-languages, web, systems, or software-engineering venue and optionally a preprint repository, subject to its moderation and endorsement rules.

### Repository layout

```text
/
├── compiler/
├── runtime/
├── examples/
│   ├── accepted/
│   ├── rejected/
│   └── marketplace/
├── benchmarks/
│   ├── framework-comparison/
│   ├── network-lab/
│   └── ai-arena/
├── paper/
│   ├── paper.md
│   ├── figures/
│   ├── references.bib
│   └── generated/
├── site/
├── rfcs/
├── adr/
├── CITATION.cff
└── README.md
```

## 8. How to introduce it to different people

### A 15-second explanation

> We are building a web platform where one readable program describes state, effects, privacy, caching, and consistency, and a compiler turns it into browser, edge, and origin code. Bugs that currently depend on React conventions or deployment discipline become compiler errors.

### A 60-second explanation

> Today a web developer manually coordinates a UI framework, server framework, cache, API contracts, retries, serialization, and deployment boundaries. The language cannot express many of the rules that make those systems correct, so they live in effects, lint rules, style guides, tests, and review. This project gives the compiler a semantic model of queries, commands, privacy, capabilities, placement, and reactive HTML. It preserves the open web, but removes conventional hydration, derives SSR and cache materialization from policy, and produces browser, edge, and origin artifacts from one application. We validate it with real incident patterns, a hostile-network marketplace, and an AI coding benchmark.

### For a web developer

Lead with:

- no dependency arrays;
- no hand-written client/server transport;
- no manual public/private cache coordination;
- fewer loading/error/impossible-state bugs;
- semantic HTML and scoped CSS remain familiar.

### For a programming-languages researcher

Lead with:

- effect rows;
- capability interpretation;
- affine resources;
- information-flow labels;
- placement constraints;
- incremental computation;
- diagnostics and inference interaction.

### For a browser engineer

Lead with:

- semantic HTML preserved;
- native document parts;
- resumable handler identity;
- declarative out-of-order patches;
- direct typed component/browser interfaces;
- compatibility implementation first, native experiment second.

### For an AI researcher

Lead with:

- smaller valid action space;
- deterministic repair feedback;
- semantic rather than syntactic PR evaluation;
- reproducible comparison against React tasks;
- smallest-model-to-quality-bar curves.

### For a company or executive

Lead with:

- fewer outage and privacy failure modes;
- less infrastructure logic in feature code;
- faster startup on low-end devices and poor networks;
- more reviewable generated code;
- incremental adoption rather than rewrite requirements.

## 9. What not to claim

- Do not call it “formally verified” unless the implementation and proof boundary justify that phrase.
- Do not say it prevents outages; say it removes specified trigger classes and makes runtime policies explicit.
- Do not imply that resumability, effects, algebraic types, edge caching, or Wasm components were individually invented here.
- Do not promise universal Rust-level performance for high-level application code.
- Do not equate compile success with product correctness.
- Do not announce AI superiority before a reproducible benchmark exists.
- Do not hide the cost of compiler complexity, serialization versioning, information-flow analysis, or ecosystem migration.

## 10. Recommended first public release

The first release worth showing broadly is not a parser or a static counter. It should contain:

1. The Bug Museum with at least 20 accepted/rejected examples.
2. A streamed store page with a private cart and one optimistic command.
3. The Cloudflare-like request-amplification comparison.
4. A generated browser/edge/origin architecture graph.
5. A static route that ships zero application JavaScript.
6. A public/private cache compile failure.
7. A small network-lab recording under one constrained profile.
8. A design-draft white paper and reproducible repository.

That package is narrow enough to build but broad enough to prove that the project is more than a language toy, renderer benchmark, or speculative manifesto.
