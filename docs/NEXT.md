# Next

The next executable tasks, in order, with acceptance criteria. Charter §3.4.

---

## Now: E8, the remaining half

`runtime/pw-host` decides admission and audits artifacts. What it does not yet
do is *run* anything.

| # | task | acceptance |
|---|---|---|
| 1 | Generate a WIT world per `ComponentContract` | the world's imports are exactly `contract.imports`, and `wit-bindgen` accepts it |
| 2 | Typed linking from a `Granted` | `linkable()`'s list becomes real `Linker` entries; a component whose contract omits an import fails to instantiate, with the engine's own diagnostic |
| 3 | Run the store's `add_to_cart` as a component | the dev server's command path goes through the host instead of a Rust closure |
| 4 | Fuel and memory limits per instance | E0's `check:fuel` moved from the spike into `pw-host`, driven by policy rather than a constant |
| 5 | Retire `worlds_for` in favour of the declared topology | the compiler keeps solving placement; the *table* of which world grants which family comes from a declaration the deployment owns |

**What is already proved** (`just e8-host`, `docs/evidence/E8/`):

- placement and capability are separate checks that disagree in both directions;
- a capability's type argument is part of what is granted;
- an admitted instance receives its contract, not the node's capability set;
- a handle carries nothing about the value behind it, and one instance's handle
  does nothing in another's hands;
- the ordinary `std` guest is refused for fourteen `wasi:*` interfaces its WIT
  world never declared.

---

## Then: E9, the permanent type and effect compiler

The architect's ruling of 2026-08-07 places it after E8 and says why: E8
*consumes* capabilities and must not define effect semantics. E9 defines them —
real inference, real ADTs, permanent effect rows, and capability/effect lowering
into the `ComponentContract` that E8-0 froze. Koka is retained as a differential
oracle for a while.

Two things E8 found that E9 owns:

- `Inference::known` is keyed by the BARE declaration name, so two declarations
  called `Cart` in different modules share an effect set
  (`docs/RISK_QUEUE.md` 34). Contracts avoid it by inferring from the body;
  nothing else does.
- The placement solver still reads *declared* effect rows in some callers, while
  contracts read inferred ones. One of those is wrong.

---

## Then: E10 onward

`docs/MILESTONES.md` has the register: own backends, deployment, the networking
lab, HTTP/3, Servo, tooling, hardening. None is started.

---

**E1 is closed** on RQ-2's measured Outcome 1. **E1A is algorithmically
implemented, source integration pending.** **E2 is in progress** and now owns
body parsing, HIR, and source-to-checker integration — see `docs/MILESTONES.md`.

---

## The revised plan

Fixed by the project architect after reviewing E2's results. Items 1–3 are done.

| # | task | status |
|---|---|---|
| 1 | Correct the E7 evidence wording — Marko is a resumption/DOM oracle, not a lazy-loading one | **done** |
| 2 | Adopt Rowan | **done** — green tree, `SyntaxKind`, invariants carried over, 68/68 corpus files round-trip |
| 3 | Write the formatter ADR, do not implement | **done** (ADR-0013) |
| 4 | Implement the durable core body grammar | **done** — 68/68 corpus files parse and round-trip; see `docs/milestones/E2.md` |
| 5 | Lower bodies into HIR | **done** — ADR-0014; id-indexed arenas, span on every node, 11 tests |
| 6 | Move declaration rules from `pw-cli` into `pw-core` | **done** |
| 7 | Connect the tested `pw-core` algorithms to `.pw` source | **partial** — exhaustiveness, the E2A-S scope graph, privacy, placement, markup rules, effect inference and the layout relations all run on source; ABI and capability still unconnected, and the placement solver still reads *declared* effect rows rather than inferred ones |
| 8 | Ratchet semantic corpus enforcement upward from 4/44 | **done — 44/44**, ratcheted in `checking_source.rs`; per-fixture table at `docs/evidence/E2D/corpus-enforcement.txt` via `just evidence-corpus` |
| 9 | Implement `pw fmt` after the syntax/HIR boundary stabilises | **done** — gate in the ADR-0013 amendment; 23 of 68 corpus files reformatted |
| 10 | Begin E2A-R only once body-level task operations can be represented | **unblocked** — `Expr::Keyword` represents them |

### The single biggest architectural decision

> **Do not build an effect-only body parser. Build the real lossless expression
> syntax now, but stage how much semantic meaning is implemented over it.**

Both alternatives were rejected with reasons worth keeping:

- **An effect-only grammar** creates a second mini-language. The real parser
  eventually sees one program and the effect parser currently sees another; it
  can misassociate lambda bodies, call arguments, operator precedence, nested
  blocks or match arms while still producing plausible-looking effect results.
  Generic callbacks are specifically a load-bearing test, so syntactic
  approximation is the wrong layer to economise on.
- **Declarations alone** cannot analyse
  `List.map(items, item => database.read(item.id))`. It would prove only direct
  declared-call propagation — not higher-order source programs, which is the
  exact failure mode the corpus exists to prevent.

### Where item 4 starts

The tree layer is in place and proven, so item 4 is now a contained task:

1. **Port the declaration parser to emit into `TreeBuilder`.** It currently
   builds the hand-rolled AST directly. `checkpoint()` / `start_at()` handle the
   cases where a node's kind is only known after its first token.
2. **Make `ast.rs` typed wrappers over `SyntaxNode`** rather than an independent
   structure — the same accessors, backed by the tree.
3. **Add the expression grammar**, filling the kinds already reserved at 200..
   and the patterns at 300...
4. Keep both losslessness suites green throughout. They are the regression net:
   the token stream and the tree are compared against each other, so a mapping
   bug shows up immediately.

### What "core body grammar" means

Not every final language feature. The durable core:

```text
names and qualified names      literals
member/field access            function calls
lambdas                        blocks
let bindings                   if/else
match and patterns             operators with real precedence
records, tuples, collections   type applications where syntactically relevant
parenthesised expressions      error/recovery nodes
```

Queries, commands, subscriptions, resources and tasks should be declarations or
typed constructs **lowering into the same core expression representation**. E4
extends the tree; it must not replace its foundation.

---

## N-0 — Linux CI (deferred, still open)

The only unmet part of the E0 gate. Risk R11 remains the highest-likelihood open
risk. Deferred by operator decision to before **E3**; it is cheap now and
expensive once generated file paths and asset casing start to matter.

| # | part | state |
|---|---|---|
| 1 | `.github/workflows/ci.yml` on ubuntu-24.04 x64 **and** arm64 | written; YAML parses; **never executed** |
| 2 | `scripts/bootstrap.sh` Linux branches, SHA-256 verified | written; checksums computed from the downloaded artifacts; macOS path re-verified; Linux path not executed |
| 3 | case-collision check | **done** — `just case-check` in `just ci`, with a synthetic collision as its control |
| 4 | `cargo deny` + Node advisory gate | **done** — `just audit`, both clean; one esbuild advisory accepted with a written reason |

**Acceptance, restated honestly:** items 3 and 4 are met. Items 1 and 2 are met
only in the sense that the code exists; neither has run on Linux. Do not record
R11 as retired until a real run is green.

---

## The next executable task

Every milestone through E5, plus the inserted E2B/E2C/E2D and E7V, is closed.
All five quality gates are green.

```text
corpus conformance        46 / 46   at C3
single-defect isolation   46 / 46
generality-tested         30 / 31   1 known narrow
headline matrices          9 / 9
resume compatibility      E7V closed
resource graph            E6 closed — 6 gate items
one parser                E6F closed — 5 tests
robustness                11 suites, 0 panics
coverage-guided fuzzing    6 targets, 3600 execs
historical compatibility   9 / 10   classified
KNOWN_GAPs                 0
```

### 1. E7 — the own renderer (E7-R / E7-P / E7-L)

**Unblocked.** E6F converged the front end, which the architect required first:

> Doing it afterward would mean debugging renderer behavior while being unsure
> which interpretation of the source produced it. Marko
is the accepted oracle for resumption and streamed patches (ADR-0002,
ADR-0017), and E7V's compatibility decision already governs the real handler
path in Chromium, Firefox and WebKit. What is missing is a renderer, not a
decision.

**Tasks 1 and 2 are done.** `just spike-own-renderer` runs the chain end to
end — `.pw` → `pw check` → `pw emit-template` → `pw-render` → HTML → a browser —
with Marko nowhere in it. 36 browser assertions in Chromium, Firefox and WebKit,
28 unit tests over escaping and rendering, and a mutation for every gate that
makes the exact measurement go red.

**Task 1.** The renderer-independent golden suite is frozen —
`spikes/pw-to-marko/e2e/golden.mjs` holds the cases as data and
`golden.spec.mjs` is the only file that knows which renderer is under test, so
running it against the own renderer is a different server rather than an edited
test. 12 oracle cases pass in Chromium, Firefox and WebKit; 1 case is the
project's own target with no oracle, recorded as *not holding* for Marko because
RQ-1 falsified it there.

**E7-R is closed.** The store page renders through the own renderer, `decide()`
authorises before attachment, and a click updates only the cart's part because
its declared resource changed.

### 2. E7-P — the own streamed patch mechanism

The foundation E7-R leaves it:

```text
IdentityDomain            E6 decides who shares one
        ↓
InstancePath              a frame per repeatable scope
        ↓
PartAddress               what a patch targets
        ↑
Patch { basis, target, op }
        ↑
server stream
        ↑
E6 resource graph + materializer
```

Three things to build against it:

- **Keyed list operations** — insert, remove, reorder, update one instance,
  replace a whole range. The `InstancePath` model is the address space they
  need; E7-R renders keyed lists and does not yet mutate them.
- **One multiplexed server→browser stream** carrying `ResourceChanged`,
  `Patch` and `Recovery`. Resource subscription and patch transport stay
  logically separate: a subscription changes, the server *may derive* a patch,
  and the transport sends it. Long-poll becomes the fallback adapter rather
  than the mechanism.
- **A causal basis per patch**, as a list of `(ResourceEntryId, Version)` even
  while every patch has one entry. A part can eventually be derived from a
  cart, a promotion and a store's pricing at once, and a field that starts as a
  collection does not need a breaking redesign to hold three.

**The five steps before E7-P proper are done.** The dependency graph is the
contract:

```text
server/runtime                     browser runtime
      \                                /
       \                              /
                 pw-protocol
                /           \
       pw-resource        pw-document
```

`pw-document` holds the address vocabulary — `TemplateSchemaId`,
`InstancePath`, `LocalPartId`, `PartAddress`, `IdentityDomain`, `InstanceToken`
— without `TemplateIR`, the renderer or any server serialization. `pw-protocol`
holds `Patch`, `PatchOp`, `CausalBasis`, `StreamFrame`, `Recovery`, the
protocol version and the decoder, and depends on neither endpoint. Four
structural tests assert that, with a negative control.

What remains for E7-P: **wire the actual Rust materializer to the actual
browser runtime through the protocol.** Today the browser talks to a ~200-line
Node server. A compact Rust development server owning `pw-materialize`,
`pw-resource`, `pw-render` and `pw-protocol` — with the browser seeing only the
protocol — removes one temporary semantic adapter from the integration path.

The three identities are separated and built:

```text
ResourceEntryId    which data changed      opaque, keyed, 128 bits
ResourceVersion    which state of it       monotonic, per ENTRY
PartAddress        where the consequence   IdentityDomain + InstancePath + PartId
                   appears
```

`EntryIdentity` in `pw-resource` is the one semantic answer to "which entry";
`pw-materialize::EntryKey` and `ResourceEntryId` both derive from it. A test
asserts that two storage representations of one identity yield the SAME wire
id — which is what proves the protocol does not depend on the materializer.

**Then E7-L**, where Marko is the negative oracle: RQ-1 measured that its
interaction module loads during initial page load, so the implementation has to
demonstrably do something the scaffolding does not.

Two smaller pieces are E6's and are deliberately not claimed there:

- **Cache-key auditing in `pw explain`** (charter §14 M6 task 9).
  `Graph::key_gaps` computes the missing dimensions and `PW5102` enforces the
  one that is always wrong; reporting the rest as advice needs `explain` to
  read the whole-program graph, which today it does not.
- **A materialized fragment that renders.** `just materialize` proves what
  recomputes and what does not; the bodies are strings a test wrote.

### 2. The large remaining ones

None is started, and each is a milestone rather than a task:

```text
E7-R/E7-P/E7-L  pw's own renderer. Marko is the accepted oracle for
                resumption and streamed patches, and has NO oracle for
                interaction-lazy loading — RQ-1 falsified that property.
E8              Rust capability host, WIT worlds, Wasmtime execution
E9              permanent value type checker and algebraic effect compiler.
                `annotations.rs` answers three corpus questions from written
                types; this is the real thing.
E10–E15         own backends, network lab, HTTP/3, Servo, tooling, hardening
```

### 3. Standing work that is never "done"

The three items the architect required before E6 are closed. They are kept
here, with what closed them, because the standing obligation does not end when
the first version lands.

- **A real coverage-guided fuzzer.** `just fuzz` — stable `-C
  instrument-coverage` plus `llvm-profdata`, an evolving corpus, six targets.
  Reported separately from `just robustness` and never merged into one
  "fuzzed" figure, because structured generation and coverage feedback fail in
  different directions.
- **A browser that calls `decide`.** `runtime/pw-resume-wasm` compiles the
  decision to wasm and the store page's Add button goes through it, in
  Chromium, Firefox and WebKit. Fails closed while the decision is loading.
- **Loop-binding types.** `{#each xs as x}` now gives `x` the element type of
  `xs`, seeing through `Result` and `Option`; `just each-typing`, evidence at
  `docs/evidence/E9/each-typing.txt`. The store demo's Add button is resumable
  because of it. An unresolved type is `PW5016`, never a quiet downgrade to an
  ordinary handler — which would make which handlers resume depend on where
  inference happens to be blind.

- **One parser.** E6F, closed. `just one-parser`. The standing part is the
  structural test: a second semantic tree can be added tomorrow, and only that
  test would notice.

Each is a first version, not a finished one. The fuzzer runs 600 iterations per
target in CI; the browser path exercises four manifests; the type rule knows
three carriers. What matters is that none of the three is now a *claim* with
nothing behind it.

### How to add work here

A new invariant needs, before it counts: a corpus fixture (C1 is frozen, so
this opens C2 — see `docs/CORPUS.md`), a challenge witness, a valid
neighbour, and a registry entry with a symbol. `just generality` and
`just ci` will refuse it otherwise, which is the intent.

---

## Standing obligations (every milestone)

Charter §3.1, plus the admissibility rule in `docs/RISK_QUEUE.md`: a
measurement or checker result is not admissible evidence until its instrument
has a negative control proving it can detect the corresponding failure.
