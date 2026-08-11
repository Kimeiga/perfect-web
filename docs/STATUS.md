# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**charter version:** **v2** (`PROJECT_CHARTER.md`, 3,206 lines). Adopted
2026-08-05 mid-session; v1 archived at `docs/research/charter-v1-superseded.md`.
See `docs/ASSUMPTIONS.md` A-008.

**numbering:** engineering `E0`–`E15`, public proofs `P0`–`P9`, risk-retirement
experiments `RQ-*`. Never a bare `M`. See `docs/MILESTONES.md`.

**resource entry identity:** one semantic definition
(`pw_resource::EntryIdentity`), two derivations —
`pw-materialize::EntryKey` for storage and `ResourceEntryId` for the wire.
A storage change is not a protocol change, and a test proves it: two storage
representations of one identity derive the same wire id.

**current milestone:** **E10 — own backends and automatic memory strategy**, and
it carries **E10-I**: compile `add_to_cart` through the Pleris→component backend
and run it through the E8 host, with no alternate Rust closure path
(`docs/EVIDENCE_LEDGER.md`). It also carries **E10-P** — give the contract's
placement demand the declaration's real privacy label, blocked on the
`PolicyExpr`/`TermExpr` split.

**2026-08-10 — semantic ownership.** `add_to_cart` now lowers to Backend IR
with two host calls, and the corpus opened **C5** (`docs/CORPUS.md`). The
sequence the architect set out was worked through in order:

```text
1  policy-value consumer audit      tests/policy_consumers.rs      DONE, frozen
2  evidence-reachability audit      tests/evidence_reachability.rs DONE, frozen
4  policy registry, stable op ids   src/policy.rs                  DONE
5  exactly-one-semantic-owner       tests/semantic_ownership.rs    DONE, Unowned = 0
6  PW0021 examines bare calls       src/check.rs                   DONE
7  the four unresolved calls        each repaired by what it IS    DONE
8  a `view` gets a contract         src/contract.rs                DONE
3  PolicyExpr / TermExpr in HIR                                    NEXT
9  regenerate E8 evidence                                          open
10 resolved-program invariant                                      open
11 Wasm encoding, invocation-region memory, E10-I                  open
```

Four findings the audits produced, each recorded in `docs/RISK_QUEUE.md`:

- a **policy value** contributes capabilities to a real contract, narrows
  placement and fires `PW0401` — four of six consumers cannot tell a policy
  value from a term;
- **nine of twenty-four** accepted fixtures could not witness their own claim;
- `contract.rs` discarded the author's pinned placement, so a page written
  `placement build` shipped a contract permitting the browser — repaired;
- a `for` loop lowers as `Expr::Call` with the callee `Name("for")`, and binds
  nothing, so its loop variable looks like an undeclared name.

**E9 is complete.** Its gate was amended on 2026-08-08 by ruling — items 2 and 4
each specified a technique where the gate wanted an outcome — and then met in
full:

```text
corpus            24/24 accepted clean, 46/46 rejected for their own invariant
E9-K              four shared effect properties, plus four divergences kept
no Koka           no crate depends on it; `just ci` never invokes it
latency           37 ms cold, after an edit, and while producing a diagnostic
fuzz              3,600 executions, 0 findings
Koka optional     a conformance tool, not a build dependency
```

E9 was largely completed by work pulled forward into E2B, E2C, E2D and E8 rather
than by a rewrite. That is allowed: a milestone is a gate, not a schedule, and
rewriting working machinery to match the original ordering would have been the
opposite of evidence.

**E8 is complete**, gate amended 2026-08-08 (ADR-0023).

**E8-0 is complete** (ADR-0020). The compiler hands the host a six-field
`ComponentContract` — component_id, abi_schema, required_capabilities,
allowed_placements, imports, exports — emitted by `pw emit-contracts` as data.
One contract per DECLARATION, because per module a database query and a browser
component each inherit the other's capability. A page does not inherit its
handlers' authority: the store page rendered `on:press={.. => add_to_cart(..)}`
and therefore required `database.write` **to render** until derivation started
excluding lambda subtrees.

**E8 is in progress.** `runtime/pw-host` decides admission in three parts —
topology, placement, artifact audit — and never a fourth. Placement and
capability are separate checks because they disagree in both directions: a
read-only replica is an origin node that cannot be written to, and a laptop
granting `dom.mutate` still may not run origin-only code.

`just e8-host` runs the audit against real components. The adversarial guest is
the ordinary one: `spikes/wasmtime-component/guest` declares one interface in
its WIT world and its component demands fifteen, because Rust `std` on
`wasm32-wasip2` injects fourteen `wasi:*` interfaces during runtime
initialization. It is refused, and every one is named.

**The command path no longer has ambient authority.** The dev server calls
`admit` against a declared topology before running a command: `add_to_cart`
requires `database.write<Carts>` by its own contract, and on a node without it
the command refuses and the state does not move. Each command is authorised on
its own contract, so one command's authority is never another's.

**What remains for E8, stated exactly.** The command BODY is still a Rust
closure. Running it as compiled Wasm needs a Pleris→component backend, and
there is none — `pw emit-koka` covers the pure subset and no code generator
sits behind it. That is a milestone of its own rather than a step in this one,
and `docs/EVIDENCE_LEDGER.md`'s rule is why it is written here instead of the
gate item being called done.

**Typed linking and resource limits are done, and the engine is what refuses.**
`engine::instantiate` populates a `Linker` from `linkable()` and from nothing
else; a contract that omits an import produces no definition, and wasmtime says
*component imports instance `perfect-web:store/stores@0.1.0`, but a matching
implementation was not found in the linker*. A pre-flight check comparing lists
would have been a second implementation of instantiation's own rule.
`Limits { fuel, memory_bytes, table_elements }` sits beside `Topology` — a
deployment's declaration rather than a constant in the host. Instantiating the
minimal guest costs 16,386 fuel, measured rather than assumed, and one byte of
memory is refused with *memory minimum size of 18 pages exceeds memory limits*.
Evidence: `docs/evidence/E8/artifact-audit.txt`, via `just e8-host`.

**WIT worlds are generated** (`pw emit-wit`, `just e8-wit`,
`docs/evidence/E8/store.wit`): one world per `ComponentContract`, whose imports
are exactly the contract's imports, resolved by `wit-parser` — the crate
`wasm-tools` and `wit-bindgen` are built on — rather than by a reader written
here. The host's own WIT is a test fixture and stays one: a deployment has to
publish a package for the capabilities it grants.

**Boundary transfer is one analysis with two policies.** `pw-core/boundary.rs`
answers whether a typed value can safely cross; `resume.rs` asks it about a
capture and `binding.rs` about an interface signature. The facts moved out of
`resume.rs` rather than being copied.

**Privacy has a destination at BOTH boundaries** (ruling, 2026-08-08, correcting
this module's first version). A resume manifest lands in the privacy scope of
the document or region containing it, so a `session view` may resume a
session-scoped value and a public one may not — private resumable regions were
inherently impossible under the first rule, which contradicted E7V. A remote
call's destination is the far node's scope, which `World` cannot supply: both
`Session<A>` and `Session<B>` live at the origin, and `Session<A> → Session<B>`
must still be forbidden. A build has no deployment in evidence, so a restricted
type on a remote edge is **undetermined rather than transferable** — four of the
store's ten exports moved, each naming the restriction it carries. One flow
relation, `Label::flows_into`, decides both.

**Deployment planning is done.** `runtime/pw-host/src/plan.rs` reads a program's
contracts and a deployment's topology and says which component may go where,
composing one `admit()` call per (component, node) pair rather than re-deriving
admission. No edge in the store demo is necessarily remote — `StorePage` needs
no capability and places at the origin alongside the command it calls — which is
the planner being right rather than the demo being thin: the language states
constraints, and browser/server separation existing conceptually is not one.

**`World::worlds_for` is deleted** (2026-08-07). Where an effect is meaningful
is the `placement` clause on its declaration; the hard-coded family→world table
is gone, and with it the answer it gave for effects nothing declares. That
answer was `None`, read as *grants it*, which is how `secret<Payments>` was
placeable in the browser for a milestone. The lookup is now three-valued —
`Known` / `Unrestricted` / `Blocked` — so *no restriction* and *I don't know*
cannot be the same value. Evidence: `docs/evidence/E8/placement-migration.txt`,
and the committed contracts reproduce byte for byte across the deletion.

One consequence worth knowing before it surprises someone: **a program that
selects no platform package is now told its effects have no meaning.** It was
silent, and silence was safe only because the table still answered underneath.

**The effect ontology is pulled forward ahead of WIT** (architect ruling,
2026-08-07), so E8 does not freeze today's stringly effect vocabulary into the
ABI. Three slices are done:

```text
1  the declaration form      effect database.read<T> { capability … host … }
2  resolution                EffectPath → EffectDefId → EffectInstance
3  the vocabulary            25 effects declared, 0 written and undeclared
```

`pw-core/ontology.rs` is the only place a dotted effect name is split, and
`tests/last_segment.rs` fails if a second one appears. It is the first
production consumer of ADR-0022's semantic provenance: every transition records
an edge, and a resolution that fails records nothing — which a mutation
verified, and which was wrong when first written.

Both earlier rulings are settled and landed. Arity is exact — omission is not a
wildcard, and all 21 underspecified rows were classified and specified. The
declarations were right where `World::worlds_for` disagreed: `dom`, `style`,
`layout`, `animation` and `paint` are placement-constrained rather than
authority-constrained, and five families stopped asking a host to grant them.
Six, once the table was gone and the question could be asked of the
declarations directly: `post_paint` was in the same position and the table had
no row to speak with.

**A package declares which namespace it exports.** `prelude Effect` in
`web.effects` makes effect NAMES ambient; their ARGUMENTS are not, and a file
naming one it has not imported is `PW5200`. Assumption A-017 was retired the
day after it was written — the 31 occurrences it named were repaired rather
than accommodated, and C4 opened for the three rejected fixtures among them.

`PW5201`, `PW5202` and `PW5203` now report an unknown effect family, an unknown
operation, and a wrong argument count — the three diagnostics that were
impossible before anything declared what a family was.

**A frame phase says when work runs, not what it does.** `mutate { .. }` no
longer synthesizes `style.mutate`; an effect comes from a resolved operation.
Execution context is the whole enclosing chain rather than the innermost phase,
because an inner `measure` does not escape an outer `post_paint` — R-042 is
that program, and it was caught until now only because a made-up effect landed
at a convenient span.

Phase legality reads **declared semantic facets** — `layout_read`,
`layout_write`, `paint_write`, `dom_write`, `compositor` — rather than an
effect's family. The family was too coarse to be a proxy for meaning:
`style.mutate<LayoutAffect>` and `style.mutate<PaintOnly>` share one, so the
animate rule could not catch the case its own comment described.

**The effect ontology is complete.** Next is deployment planning
(`docs/NEXT.md` step 13).

**E7, closed.** The **real store page** is rendered by the own renderer, its
handlers are authorised by `decide()` before they attach, and a click updates
only the cart's part while every menu node keeps its identity. A click changes
the browser because the **declared resource dependency changed** — the command
commits and returns nothing about the cart; the event's arguments select which
entries invalidate; the resource refreshes with a version; the subscriber is
told. 258 browser assertions across Chromium, Firefox and WebKit
(`just spike-own-renderer`), and `just golden-both` runs the shared case set
against Marko and the own renderer as separate processes.

E7-P added keyed collections — `InsertBefore`, `InsertAfter`, `RemoveInstance`
and `MoveInstance`, with DOM identity surviving a move because the nodes are
moved rather than rebuilt — the public menu materialized once and shared so
that one patch reaches every reader, and **two transports** carrying the same
frames: a held streaming connection and a long poll, under one cursor
discipline where a client's next request is its acknowledgement of the last
frames it applied.

E7-L added interaction-lazy code loading. The document carries no behaviour;
handlers are served by IDENTITY and fetched on first interaction, after the
E7V decision authorises them. An unauthorised handler is never fetched, and a
failed load is visible on the element and recoverable.

`just e7-performance` measures gate items 7–10 alone on Chromium, because a
long-animation-frame measurement taken while three engine families hammer the
machine measures the machine. Every figure has a negative control in the same
session:

```text
static route            0 bytes of script, 0 of wasm
interactive route       28,471 bytes script + 74,222 wasm, no handler bytes
activation              ~4-25 ms, marked by the runtime itself
interaction long frames 0   (control: a deliberate thrash gives 1 at 150 ms)
runtime layout reads    0   (control: the counter counts a deliberate read)
1,000-item menu         10.1 ms uncontained → 1.7 ms with content-visibility
```

Gate 9 is proved rather than sampled: the runtime performs **no layout reads at
all**, so it cannot interleave measurement with mutation.
`forcedStyleAndLayoutDuration` is not exposed by this browser — E0 measured
that — so gate 8 uses the long-animation-frame count E0 validated instead.

Live parts are addressed as `IdentityDomain + InstancePath + LocalPartId`. A
template part *definition* is not a document part *instance*: the store page's
three Add buttons share `data-pw="0"` and have three distinct addresses.

**E6 decides who shares an identity domain and E7-R decides how things inside
it are addressed** — there is no independent E7 notion of who shares. A public
materialization has one domain, so every reader of that cached entry gets the
same bytes; two session partitions derive unrelated tokens for identical
application keys.

The token is a keyed BLAKE3 derivation truncated to 96 bits, base64url, 16
characters — an opaque address and never a capability. A collision within a
domain **refuses the render**: correctness does not rest on probability.

"No component replay" is proved structurally — the client artifact contains no
template renderer, with a poisoned artifact as the negative control — and then
observed, by a MutationObserver that sees mutations only inside the cart part.

**everything before E8 is closed:** E0, E1A, E2, E2A, E2B, E2C, E2D, E3, E4,
E5, E6, E7 and the inserted E6F and E7V. The three items the architect required before E6 could start are
also closed — coverage-guided fuzzing (`just fuzz`), the compatibility decision
on the real browser handler path, and typed `{#each}` captures
(`just each-typing`).

**E6 is complete** (`docs/milestones/E6.md`): the dependency graph is derived
from declarations and serialized (`pw emit-graph`), three build-time rules
enforce what an edge may be, and `runtime/pw-materialize` consumes committed
events from a transactional outbox. Changing one menu item regenerates 1 of
1000 fragments and leaves 999 untouched — counted from the causality record,
with the argument-less event that regenerates all 1000 as its control.

Implementing it found that **every graph edge in the corpus pointed at
nothing**: a `materialize` block's policies were being parsed as expressions, so
`decl.policies` was empty for every materialization, and seven fixtures named
resources and events no file in their program declared. Corpus **C2** opened.

**next milestone:** **E8's Wasm capability host**, beginning with **E8-0**: the
frozen compiler→host `ComponentContract`. The architect's ruling of 2026-08-07
sets the order and the principle:

> The compiler decides what authority code needs. The host decides whether that
> authority physically exists. Neither should reconstruct the other's answer.

Then E9's permanent type and effect compiler. `docs/MILESTONES.md` has the
register.

Three of the four shortcuts `readiness.txt` named this morning are closed:
branch-aware affine analysis, Option inference that does not need the
annotation, and string holes lowered as expressions. Each replacement is proved
by a program the narrow rule would have passed.

E0, E1A, E2, E2A, E2B, E2C, E2D, E3, E4, E5, E6, E6F, E7V and E7 are complete.
E1 closed on RQ-2's Outcome 1. E8 onward are not started.

**E6F is complete** (`docs/milestones/E6F.md`): there is one parser. The second
declaration parser and its AST are deleted — 1,484 lines — and `pw explain`, the
declaration rules, the generality validity pipeline, the robustness suites and
the fuzz harness all read the HIR. `just one-parser` enumerates the real keyword
tables rather than a copy, and asserts structurally that no crate builds a
second semantic tree.

It found that the validity pipeline had been asking the *legacy* parser whether
a witness parsed while every analysis ran on the other one — so it certified
"parses without recovery" for a file no analysis could read.

**risk-retirement queue** (`docs/RISK_QUEUE.md`):

| | | |
|---|---|---|
| RQ-1 | Marko resumption, Chrome + Safari | **done** — passed its pre-registered rule |
| RQ-2 | Koka higher-order effect propagation | **done** — Outcome 1, clean pass |
| RQ-3 | `pw` exhaustiveness + typed ABI | **partial** — checker done; no parser until E2 |
| RQ-4 | structured concurrency | **done** — E2A-S runs on source; E2A-R is `runtime/pw-tasks` |
| RQ-5 | artifact capability audit | **partial** — rule done; not wired to a build |
| RQ-6 | effect-family rejection coverage | **partial** — layout/DOM families done |
| RQ-7 | E→P evidence ledger | **done** |

**E2 has landed a front end.** `just ci` now runs `pw check` over all 68 corpus
files and they all parse, with precise spans and error recovery. The
specification is executable in CI rather than merely well-formed.

**Corpus enforcement, as three numbers** (architect ruling, 2026-08-06 — a
single figure hides the difference between a red diagnostic, the *right* red
diagnostic, and the whole declared invariant being checked):

```text
46 / 46  rejected fixtures produce a compile error
46 / 46  emit their declared canonical code
46 / 46  fully enforce the complete declared invariant
```

**Charter §14 M2 gate item 2 — ≥40 of 44 — is met**, at corpus version **C3**
(`docs/CORPUS.md`), with the three numbers equal and no wrong-reason catches.
The floor is now the directory rather than the constant `44`: a corpus that
grows past a hardcoded floor leaves its new fixtures unenforced while the
assertion still passes.

There is now a **second gate, and it is open**:

```text
corpus conformance        46 / 46   at C3
single-defect isolation   46 / 46
generality-tested         30 / 31   1 known narrow
headline matrices          9 / 9
resume compatibility      E7V closed — 34 matrix rows, 6 fuzz targets
robustness                11 suites, 0 panics, 3 regressions retained
coverage-guided fuzzing    6 targets, 3600 execs, 0 findings
resource graph            E6 closed — 6 gate items, 24 materializer tests
one parser                E6F closed — 5 tests, 0 second trees
own renderer              E7-2 closed, E7-R slice — 120 browser + 37 unit
historical compatibility   9 / 10   the miss classified
KNOWN_GAPs                 0
```

Fuzzing is reported on its own line and never folded into the robustness one.
Structured generation and coverage feedback fail in different directions, and a
single "fuzzed" figure would let one cover for the other.

An invariant counts as *generally* enforced only when a program its fixture did
not anticipate is caught. `examples/generality/` holds those programs — and
also `slips-through.pw` files, which are known gaps written as code that must
compile clean, so `just ci` fails the moment a gap closes and the witness needs
promoting. `just generality` scores it.

Read the second number as the honest one. It moved 7 → 30 today; the first
moves only when the corpus does.

**30 / 31, not 31 / 31.** `private_in_shared_materialization` has two executable
known gaps — a private value reaching a shared fragment through a public query's
body, by a helper and by a branch — so it is *known narrow* rather than
generality-tested. Both are programs in
`examples/generality/private_in_shared_materialization/`, and `just generality`
fails the moment either stops compiling clean.

Up from four when E2 began, each with a primary span, an origin span, a note and
a legal alternative. `just evidence-corpus` regenerates
`docs/evidence/E2D/corpus-enforcement.txt`, the per-fixture table — written by
the same test the ratchet asserts on, so the published number cannot drift from
the enforced one.

The three numbers have stayed equal at every step, which is the point of
reporting three. Twice they diverged and both times it was treated as a
regression rather than banked: once when a count rose to 27 by reporting two
fixtures for effects they had declared, and once when `R-012` was caught for a
second defect that masked its declared invariant.

Zero false positives across the 24 accepted files, four of which (`A-018`,
`A-020`, `A-021`, `A-023`) exist specifically as negative controls for the
layout rules — each differs from its rejected twin in exactly the one way the
rule is about. Coverage is ratcheted by a test so it cannot silently regress, and every rule
has a negative control in `examples/rules/**` that differs from its rejected
twin in exactly the one way the rule is about.

**The front end is complete through HIR.** Rowan is adopted (ADR-0012); the
body grammar parses all 68 corpus files and they round-trip byte-for-byte;
lowering (ADR-0014) turns them into id-indexed arenas with a span on every node,
and `pw-core`'s checkers consume those ids without ever seeing a syntax node —
enforced by a test, not by convention.

**`pw fmt` ships** (ADR-0013 amendment). Idempotent, token- and
comment-preserving on all 68 files, `--check` wired into `just ci`. It is a
canonical *spacing*, not yet canonical line breaking, and the ADR says so.

**E2A is complete — both halves.** The static checker rejects `.pw` programs
(R-013, R-039) and `runtime/pw-tasks` (ADR-0016) proves the same semantics
behave as specified under real concurrency: cancellation propagation, ordered
cleanup, refusal to commit into a dead scope, and no leaked tasks. 12 tests,
0 failures in 25 consecutive runs. They are **behaviour** results and RQ-4's
rule holds — neither half may be described as covering the other.

**`pw` renders, streams and resumes in a browser. E3 is complete.** Three
applications written in `.pw` generate Marko (ADR-0017), build, and pass 30
tests across Chromium, Firefox and WebKit. The static and streamed routes ship
**zero JavaScript** — no script tag, no request; the streamed page's shell was
usable **1,201 ms before** its 1,200 ms region arrived, in two chunks; the
counter resumes without re-rendering the inert part of the page. Those are
Marko's behaviours measured through `pw`, and the evidence ledger says so.

**`pw` code executes.** `pw emit-koka` lowers the pure subset (ADR-0015) and
`just spike-pw-to-koka` compiles and runs it under the pinned Koka 3.2.3,
matching hand-computed values. Two negative controls run with it, including one
proving the generated `total` annotation is load-bearing rather than
decorative. **E2 gate: five of six items pass.** The open one is the
rejected-corpus count, which E1 and E5 own.

**public claims:** governed by `docs/EVIDENCE_LEDGER.md`. The **corpus** gate is
met at 44/44, but P0 as a whole still cannot be published: it has other claims,
several unstarted, and several resting on Marko and Koka behind adapters rather
than on `pw`'s own implementation. `docs/evidence/P0/readiness.txt` states what
may and may not be said today, and the second list is the longer one.

**last passing commit:** `7630593` — Linux CI and supply-chain scanning.
`just ci` passes at that commit on macOS 26.5.2 / arm64, with 207 tests.
`just audit` is clean; `just spike-pw-to-koka` executes generated Koka.

---

## completed gate items

All seven charter §14 M0 gate items:

1. **`just doctor` works on the Mac** — exits 0, read-only, warns on the 16 GiB host deviation.
2. **All six spikes run from documented commands** — `just spikes`, six evidence files in `docs/evidence/E0/`. Charter v2 allows recording a blocker instead; none was needed.
3. **Versions and licenses pinned** — `tools/versions.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, SHA-256-verified release tarballs, license column in the technology matrix.
4. **≥10 accepted / ≥20 rejected examples** — now **24 and 44**, covering **24/24** and **44/44** charter §16 categories including v2's layout/DOM families. Enforced by `tools/corpus-check` in `just ci`.
5. **Reuse/fork/tape/build matrix complete** — `docs/research/technology-matrix.md`, with *measured* vs *read* clearly distinguished.
6. **Known failures documented honestly** — `docs/KNOWN_LIMITATIONS.md`.
7. **One clean `just ci`** — passes on macOS.

---

## failing gate items

**None.** The E0 gate is fully closed as of 2026-08-06.

- **None.** Gate item 7 closed on 2026-08-06: `.github/workflows/ci.yml` ran on
  `ubuntu-24.04` and `ubuntu-24.04-arm` and passed on the first attempt, along
  with the supply-chain job. Charter §13.5 case checking and §3.6 license and
  vulnerability scanning both run in CI. **Risk R11 is retired**; evidence in
  `docs/evidence/E0/linux-ci.txt`.

---

## exact commands to reproduce

```bash
just doctor        # read-only environment check; exits 0 when E0 tools are present
just bootstrap     # fetch pinned Koka 3.2.3 + Wasmtime 47.0.3 into .toolchain/, pnpm install
just ci            # fmt-check + clippy -D warnings + 18 unit tests + corpus check  -> "ci: OK"
just spikes        # all six spikes; rewrites docs/evidence/E0/*.txt

# individually
just spike-compiler-diagnostic
just spike-koka
just spike-wasmtime
just spike-marko
just spike-layout    # charter v2 §7.5A forced-layout instrumentation
just spike-bonsai    # charter v2 Incremental/Bonsai study (needs opam switch pw-bonsai)

just env-record    # regenerate docs/environment/macbook.md + tools/versions.lock
```

First run on a clean machine: `just bootstrap` then `just doctor` then `just ci`.

---

## known environmental issues

- **Host is an M2 Pro / 16 GiB**, not the M3 Max / 64 GB the charter assumes
  (`docs/ASSUMPTIONS.md` A-001). Milestone 11's VM table allocates 14 GB of
  guests and does not fit; revised sizing is recorded. **All performance numbers
  are incomparable to figures from an M3 Max.**
- **Node is v22.21.1** (maintenance LTS) rather than v24 (active LTS). Satisfies
  every declared engine range (A-002).
- **WASI 0.3 is unreachable** through Rust's stable `wasm32-wasip2`; imports
  resolve at `@0.2.9` (ADR-0008).
- **Koka's `.kki` format is internal and unstable.** `node/kki.mjs` pins
  version 3.2.3 and throws on any other.
- **`just bootstrap` is macOS/arm64 only** — it refuses elsewhere with a clear
  message rather than doing something wrong.

---

## last benchmark summary

**No benchmarks yet.** Benchmarking begins in Milestone 3 (charter §14 M3 task 8);
baselines against Next/React, SvelteKit and Marko are §18.1.

The Milestone 0 spike measurements below are **single runs on one machine over
localhost** — spike evidence, not benchmark results (charter §18.5 requires
sample counts and distributions):

```text
marko /static         588 B html, 0 script tags, 0 downloaded JS
marko /stream         shell 3.1 ms | 400 ms subtree at 408 ms | 1200 ms at 1206 ms
marko resumption      HTML 9.6x larger -> route-specific client JS 1.03x
wasm component        no_std 5,276 B (1 import) vs std 43,837 B (15 imports)
layout thrash/phased  79.1 -> 0.3 ms at n=400; 678.6 -> 0.8 ms at n=1200 (848x)
containment           3,000-row subtree build+layout 23.8 -> 5.7 ms (4.18x)
incremental           5 unrelated updates -> expensive node evaluated once
```

---

## next three concrete tasks

1. **Run the store's `add_to_cart` as a component**, so the dev server's command
   path goes through the host instead of a Rust closure. The last E8 gate item,
   and the one that needs a compiled Pleris component rather than a spike guest.
2. **E9** — effect rows as a real inference algorithm, with the ontology already
   in place so the ABI does not have to change under it.
3. **E10 onward**, per `docs/MILESTONES.md`.

Linux CI is deferred by operator decision to before E3 (risk R11).

Full ordered list with acceptance criteria: `docs/NEXT.md`.
