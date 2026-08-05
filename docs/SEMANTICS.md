# Semantics

The semantic model being validated. Charter §7 defines it; this file records
which parts are **specified**, which are **demonstrated**, and which are still
**open**, so no one has to guess how much is real.

Syntax here is provisional (ADR-0010). Charter §7 preamble: *"These semantics are
more important than final syntax. Do not optimize syntax before they are
demonstrated."*

Status key: **specified** = written in the corpus · **demonstrated** = a spike
proves it works · **open** = neither.

---

## 1. Values and types (charter §7.1)

| property | status | where |
|---|---|---|
| ADTs with payload and payload-free variants | **demonstrated** | Koka spike: `order-state` round-trips to JS |
| Exhaustive pattern matching | **partially demonstrated** | Koka enforces it only for `exn`-free functions (F-8). `pw` must own the rule (M9A). Corpus R-007. |
| `Option<T>` | **demonstrated** | `maybe<a>` crosses the boundary; but `Nothing === null` (F-7). Corpus R-008. |
| `Result<T, E>` with generic `E` | **demonstrated** | had to be declared ourselves — Koka's `error<a>` fixes `E` to `exception` (F-3) |
| Nominal opaque domain types | **specified, NOT enforceable via Koka** | single-field `value struct`s are erased (F-4). Corpus A-001. |
| Typed units / `Money<USD>` | **specified** | same erasure problem; `pw` manifest must carry it. Corpus A-001. |
| No ambient `null`/`undefined` | **specified; violated at the Koka/JS boundary** | F-7. Corpus R-008. |
| No implicit coercion | **demonstrated** | decoder rejects a number where a string is required |
| No unchecked cast | **specified** | corpus R-009 |
| Explicit decoding at every external boundary | **demonstrated** | `node/decode.mjs` — every field checked, failures returned not thrown. Corpus A-012. |

**Learned:** the erasure and null findings mean nominal typing and absence cannot
be delegated to a backend that does not preserve them. This is the single
strongest architectural constraint Milestone 0 produced.

---

## 2. Effects (charter §7.2)

Function types carry an effect row. Empty means pure.

```text
calculate_subtotal : Cart -> Money<USD> !{}
load_store         : StoreId -> Result<Store, StoreError> !{ database.read<Stores>, trace }
```

**Demonstrated.** Read straight out of Koka's `.kki`:

```text
calculate-subtotal     !{}                                <- pure
available-items        !{}                                <- pure
describe-order         !{}                                <- pure
load-store             !{database-read, trace-effect}
load-store-subtotal    !{database-read, trace-effect}
load-store-for-js      !{}      <- handlers discharged the effects
main                   !{console}
```

This is charter §14 M1's gate item *"every effectful example has a visible
inferred effect"*, demonstrated a milestone early.

Core effect families (charter §7.2) — **specified**, none enforced yet:

```text
database.read<Resource>   database.write<Resource>   network<Origin>
clock.monotonic           clock.wall                 random.secure
random.nondeterministic   trace                      log<PrivacyLevel>
session                   secret<Name>               storage<Namespace>
device.location           device.camera              ui.prompt
task.spawn                resource.acquire<Type>     durable_job.enqueue<Type>
```

**Open:** row polymorphism (so generic library functions preserve callback
effects) is untested.

---

## 3. Capabilities (charter §7.3)

Effects say what code *requires*; worlds say what each target *receives*.

**Demonstrated** at the component boundary. A guest whose WIT world imports only
`perfect-web:store/stores` cannot reach anything else, and withholding that import
makes instantiation fail with a diagnostic naming it. Fuel limits are enforced.

**Learned:** ambient authority can arrive through the *language runtime* rather
than the application. A `std` Rust guest demanded 15 WASI instances for a
one-capability world. The build must assert the import list against the declared
world.

**Specified, not enforced:** browser/edge/origin world definitions (corpus R-002,
R-003, R-025, R-026).

---

## 4. Pure rendering (charter §7.4)

A `view` has an empty external effect row. It may not perform I/O, mutate domain
state, read the clock or randomness, start a task, acquire a resource, read a
secret, log private values, or enqueue durable work.

**Specified** — corpus A-002 (pure exhaustive view), R-001 (network in a view,
the canonical rejection). **Not yet enforced:** no compiler exists.

---

## 5. No universal lifecycle effect (charter §7.5)

There is no `useEffect` equivalent. Distinct primitives instead:

| primitive | meaning | corpus |
|---|---|---|
| `signal` | ephemeral local UI state | — |
| `derived` | pure value computed from other values | — |
| `query` | keyed remote read: cache, freshness, lifecycle, cancellation, dedupe | A-003, A-004, A-008 |
| `command` | mutation: authorization, idempotency, transaction, optimistic behavior, invalidation | A-005, A-010 |
| `subscription` | scoped stream of changing external values | A-006 |
| `resource` | acquire/release for an imperative handle | A-007 |
| `task` | structured child computation owned by a scope | R-013 |
| `durable` | explicit background job outliving the request | — |
| `unsafe lifecycle` | audited escape hatch, forbidden by default in app packages | R-031 |

**Specified.** Rejections R-013 (detached task), R-028 (subscription outliving
scope), R-029 (optimistic with no rollback), R-031 (unsafe without justification)
encode the boundaries.

---

## 5A. Layout effects and frame phases (charter §7.5A)

Added by charter v2. The premise:

> *"Fine-grained DOM updates do not by themselves prevent forced synchronous
> layout."*

Phase/effect families — **specified**, none enforced yet:

```text
dom.mutate                 # attributes, classes, text, insertion/removal
style.mutate<LayoutAffect> # writes that may invalidate style/layout
layout.measure             # geometry: boxes, scroll metrics, computed layout
observe.resize             # browser-delivered element-size changes
observe.intersection       # browser-delivered visibility/intersection changes
animation.composite        # transform/opacity-style compositor work
paint.custom               # canvas/custom painting escape hatch
post_paint                 # work deferred until after presentation
```

Ordinary application code may not call `offsetWidth`, `clientHeight`,
`scrollHeight`, `getBoundingClientRect`, or layout-dependent computed-style reads
directly. Typed primitives return a scheduled `LayoutSnapshot<T>` instead.

The default frame transaction (§7.5A):

```text
1. receive input and resource changes
2. stabilize pure/incremental computations
3. collect all requested measurements while layout is clean
4. compute the mutation plan without touching the DOM
5. apply all DOM/style mutations in one batched commit
6. allow browser style/layout/paint/composite
7. deliver observer and post-paint results into a later transaction
```

**Demonstrated — and the effect size is the largest measured anywhere in
Milestone 0.** `spikes/layout-phase-scheduler` implements steps 3–5 as a runtime
`FrameScheduler` and compares it against the interleaved read/write anti-pattern
on identical work (checksums match):

| elements | thrash | phased | ratio |
|---:|---:|---:|---:|
| 400 | 79.1 ms | 0.3 ms | **264×** |
| 1200 | 678.6 ms | 0.8 ms | **848×** |

Long Animation Frame count: **7 long frames** thrashing, **0** phased.

**Learned:**

- **A runtime-enforced API captures the entire benefit.** The scheduler separates
  `measure()` from `mutate()` by shape alone, with no compiler involvement. This
  supports the charter's own fallback: *"keep the phase scheduler as a
  runtime-enforced API even if compile-time proof is initially incomplete."* The
  compiler's job is to make the unsafe path **unrepresentable**, not to make the
  safe path fast — it already is.
- **`forcedStyleAndLayoutDuration` is not exposed** in Chrome 150, so the
  instrumentation uses long-frame count and `blockingDuration` instead. The
  charter said "where available"; it is not available.
- **Containment is not a decoration.** `contain: layout style paint` +
  `content-visibility: auto` made building and laying out a 3,000-row subtree
  **4.18× faster** (23.8 ms → 5.7 ms), but made a forced layout after an
  unrelated host mutation marginally *slower*. §7.5A's instruction to emit it
  "only when subtree independence is semantically valid" is a performance
  constraint as well as a correctness one.
- **ResizeObserver feedback loops are detectable**: the browser emits
  `ResizeObserver loop completed with undelivered notifications` as a window
  error, which is the signal §7.5A's required warning can be built on.

**Open:** everything else in §7.5A — coalescing by document part, cancelling
measurements for unmounted scopes, virtualization requirements, frame-budget and
layout-causality devtools, and the compile-time phase separation itself.

---

## 6. Structured concurrency (charter §7.6)

Every ordinary task belongs to a parent scope and is cancelled or completed when
that scope ends. No fire-and-forget.

**Specified** (R-013, R-028). **Entirely undemonstrated** — the Koka spike tested
no async at all. This is the largest untested area of the model and is tracked in
`docs/RISK_REGISTER.md`.

---

## 7. Affine resources (charter §7.7)

Scarce or correctness-sensitive handles are affine: `DatabaseTransaction`,
`OpenStream`, `WebSocketConnection`, `SubscriptionHandle`, `MapHandle`,
`SecretHandle`, `LockGuard`, `FileUpload`.

A transaction must end exactly once, in commit or rollback; a resource must not
escape its scope.

**Specified** — A-007, R-011, R-012. **Open** — the Koka spike did not attempt
linearity; charter §14 M1 task 7 warns explicitly against claiming Koka proves
linear usage when it does not. Milestone 9C owns this.

---

## 8. Privacy labels (charter §7.8)

```text
Public   Session<SessionId>   User<UserId>   Organization<OrganizationId>
Device   Secret<Capability>
```

Not a total order — some labels are incomparable. Joins are tracked through data
flow; invalid serialization, logging, placement and caching are rejected.

**Demonstrated in miniature.** `spikes/compiler-diagnostic` implements
`SharedCache<Session>` rejection end to end, producing charter §16.3's target
diagnostic from real spans:

```text
error: [PW0100] cannot materialize `cart` in a shared public cache
1 | session query cart(id: SessionId)
  | ------- `cart` is declared here, so its result is labeled `Session<SessionId>`
4 |     cache shared
  |     ^^^^^^^^^^^^ this cache is shared across all sessions
  = note: a shared cache may contain only `Public` values; this query's result is `Session<SessionId>`
  = help: change this to `cache private`, or move the value into a private streamed slot
```

Must fail (charter §7.8), all in the corpus: `SharedCache<Cart@Session>` (R-004),
`BrowserValue<PaymentSecret>` (R-003), `PublicRender<CurrentUserEmail>` (R-030),
`Log<Public>(PaymentToken@Secret)` (R-006). Plus R-005 (cache key missing tenant).

---

## 9. Placement (charter §7.9)

```text
Build   Browser   Edge   Origin   Database
```

Derived by default from required effects and capabilities, privacy labels, data
dependencies, latency preference, consistency policy, browser-only APIs,
secret/database restrictions, and cache sharing policy. Explicit constraints are
allowed.

**Demonstrated in miniature.** `pw-spike-diag --explain` derives placement from
visibility plus cache partition alone:

```text
privacy      Session<SessionId>
cache        private  (partitioned per session)
placement    Origin (session-scoped read; never shared-cached)
```

Seeing this made the §7.9 claim concrete: placement is a function of policy
annotations the developer is already writing.

**Specified:** R-002, R-016, R-025, R-026.

---

## 10. External decoding (charter §7.10)

Everything entering from JSON, forms, headers, cookies, browser storage,
databases, third-party APIs or queues starts as `Unknown`. Decoders return
`Result<T, DecodeError>`. No unchecked assertions are generated.

**Demonstrated.** `node/decode.mjs` decodes Koka's JS output as untrusted
external data — because from JavaScript's side, it *is*. Every field is
type-checked, failure is a returned value, and malformed input is rejected rather
than guessed. Corpus A-012, R-009.

**Learned:** the backend's own type predicates are not validators.
Koka's `is_just(x)` is `x !== null`, so `is_just(42)` is `true`, and `is_qok(null)`
throws. A decoder must confirm shape structurally first and use the backend's
predicate only to choose among confirmed variants.

---

## Summary of what is actually proven

| area | proven |
|---|---|
| effect inference distinguishes pure from effectful, machine-readably | **yes** |
| handlers discharge effects so a pure value can cross a boundary | **yes** |
| ADTs survive a language boundary with explicit decoding | **yes** |
| capability withholding blocks execution at instantiation | **yes** |
| charter-quality privacy diagnostics are reachable | **yes** (one rule) |
| streaming and resumption without whole-tree hydration | **yes** (Marko) |
| exhaustiveness as an independent rule | **no** — Koka does not provide it |
| nominal domain types at a backend boundary | **no** — erased |
| structured concurrency, affine resources, row polymorphism | **not attempted** |
