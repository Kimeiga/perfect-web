# Next

The next executable tasks, in order, with acceptance criteria. Charter §3.4.

---

## Now: the architect's sequence of 2026-08-07 (second ruling)

Ten steps. 1-4 and 10 are done; 5-9 are E8's remaining work.

The language is **Pleris** (ADR-0021); Perfect Web stays the project, `.pw` the
source format, `pw-*` the internal crates. Whether the developer-facing CLI
becomes `pleris` rather than `pw` is undecided and purely mechanical —
`justfile`, docs, evidence scripts — so it blocks nothing.

| # | task | state |
|---|---|---|
| 1 | Delete program-wide-unique resolution | **done** — audit landed; prelude deferred by ruling |
| 1a | Repair the two `forbidden` last-segment sites | **done** — ratchet at 0 |
| 2 | Build-time diagnostic for an unresolved capability argument | **done** — `PW5200`, `Owner::Capability`, 8 controls. Unknown FAMILY and OPERATION still need the declared capability table (E9) |
| 3 | Placement consumes `effective_effects` | **done** — both discriminating controls, and it found a witness proving an annotation |
| 4 | The deployment planner abstraction | **done** — `runtime/pw-host/src/plan.rs` |
| 5 | Freeze `ComponentBinding` / remote-capable semantics | **done** — `pw-core/{boundary,binding}.rs`, `BindingSupport` on each `Export` |
| 6 | Generate WIT worlds from semantic contracts | not started |
| 7 | Typed linking only from `Granted` | not started |
| 8 | Run real `add_to_cart` through Wasmtime | not started |
| 9 | Fuel and memory limits | not started |
| 10 | Replace `worlds_for` with declared node topology | **done** — deleted 2026-08-07; placement is the declaration's own clause |

**Revised 2026-08-07 by ruling: pull the effect/capability ontology forward.**

> Do not wait for full E9 to declare effect families and operations. Pull
> forward only the effect/capability ontology before E8 hardens WIT.

`PW5200` can catch an unknown ARGUMENT and cannot catch an unknown FAMILY or
OPERATION, because nothing declares what a family is — `log` and a misspelled
`databse` are indistinguishable. That distinction must exist before WIT
generation, and it does NOT need E9's permanent inference algorithm.

A platform-level declaration model, sketched by the architect:

```text
effect log                { capability none }
effect database.read<T>   { capability database.read<T>
                            host_interface pw:host/database#read }
effect style.mutate<T>    { capability dom.style.mutate<T> }
```

giving

```text
source effect spelling → EffectDefId + resolved arguments → EffectInstance
  → effective_effects(DefId) → optional capability lowering → CapabilityId
  → ComponentContract → WIT
```

and separating three facts that are currently one string:

```text
Effect           something computation does
Capability       authority required to do it
Host interface   one ABI representation of that authority
```

**Not every effect needs a host capability.** After this lands, unknown family,
unknown operation and unknown argument are all compile errors, legitimate
non-authority effects stay valid *because they are declared*, and no checker
contains a hard-coded list of valid family spellings.

### The grammar decision — MADE, and the form LANDS

Architect ruling, 2026-08-07: **flat dotted**, with an effect-specific
qualified name.

```pleris
effect log {
    capability none
}

effect database.read<T> {
    capability database.read<T>
    host       "pw:host/database#read"
}
```

> The declaration should look like the thing that appears in an effect row. […]
> But implement `database.read` as an effect-specific qualified name, not as a
> new ability for arbitrary declarations to have dotted names.

**Slice 1 is done** (`tests/effect_declarations.rs`, 9 controls): the form
parses, lowers to `DeclKind::Effect`, and takes its own `Namespace::Effect` so
`effect log` and `fn log` coexist. `type foo.bar`, `fn foo.bar()` and
`query foo.bar` all still fail to parse — asserted, because a local grammar
change that quietly went global is the thing the ruling warned about.

`capability` and `host` are POLICY clauses, not expressions: `database.read<T>`
is a name and the expression grammar reads `<` as a comparison, which is the
same reason a `materialize` block's policies live inside its braces (E6).

### What provenance covers so far

`pw-core/src/provenance.rs`: `Fact { kind, origin, depends_on }`, an `Evidence`
DAG, and `Route` — `Qualified`, `ReceiverType`, `ScopedName`, and
`ProgramWideName`, which is deleted from the compiler and **kept nameable so a
test can forbid it**. A route with no name cannot be forbidden, and that is the
route every instance of coincidental correctness so far has travelled by.

One consumer: effect inference's member resolution records the receiver-type
edge and the callback it travelled through.

`tests/causal_evidence.rs` asserts R-037's claim as a chain — required edges,
the forbidden source, and a semantics-breaking control (a receiver whose type
has no such member produces no chain at all).

**Verified against the defect it exists for:** unbinding the callback parameter,
as before `3a0f319`, turns the two chain assertions red. Two others stay green,
which is correct — they are about the forbidden route and the breaking control.

Still to record: privacy labels, placement demands, capabilities, resource
invalidation. Tier 1 in ADR-0022, and none of them recorded yet.

### Slice 2 — and it is the first consumer of ADR-0022

Architect ruling, 2026-08-07: **do not delay slice 2 for the provenance work —
build it into slice 2.**

> So slice 2 becomes both: completion of the effect ontology; and the first
> production implementation of semantic provenance.

Each transition below must produce evidence, and a test must assert that no
later analysis splits `"database.read"` at the dot.

### The immediate sequence — the effect ontology is COMPLETE

```text
 1  package-declared Effect prelude                 DONE
 2  pw-platform-web exports through it              DONE
 3  effect arguments obey normal scope              DONE — 31 rows, C4 opened
 4  delete TypeArgument::Unscoped                   DONE — A-017 retired
 5  finish EffectPath → EffectDefId → EffectInstance DONE
 6  unknown family / operation / arity diagnostics  DONE — PW5201/5202/5203
 7  remove frame-phase intrinsic_effect             DONE — R-042 repaired,
                                                     phases_at reads the chain
 8  add database.connect                            DONE
 9  database.write becomes database.write<T>        DONE
10  declaration-driven capability/host lowering     DONE
11  re-run the ComponentContract matrix             DONE
12  semantic effect facets; phase rules consume     DONE — the family check
    them; delete the family check                    is deleted
13  deployment planning                             see the sequence below
```

### The architect's sequence of 2026-08-07, fifth ruling

```text
1  unresolved facet markers are a build error   DONE — PW5204
2  canonical policy-keyword coverage guard      DONE — directional, with the
                                                 mutation that motivated it
3  session.read: capability + topology, not     DONE — no intrinsic placement
   guessed placement
4  freeze the placement migration matrix        DONE — placement_migration.rs
5  deployment planning on ontology + topology   DONE — pw-host/src/plan.rs
6  delete World::worlds_for                     DONE — see below
7  boundary-transfer local/remote binding feasibility   DONE — see below
8  WIT generation                                       NEXT
```

### Step 6, as it landed

`World::worlds_for` is gone. Where an effect is meaningful is the `placement`
clause on its declaration and nothing else. The evidence is
`docs/evidence/E8/placement-migration.txt`, produced by `just e8-placement`.

**The lookup is three-valued**, on the ruling:

> Do not turn `ontology lookup failed` into `no placement restriction`. That
> would recreate the `secret<Payments>` hole in a new form.

```rust
pub enum PlacementLookup {
    Known(Vec<World>),          // the declared constraint
    Unrestricted,               // declared, and declared to constrain nothing
    Blocked { code: &'static str },  // nothing declares it; placement stops
}
```

`Option<Vec<World>>` could not say that — `None` read as both *anywhere* and
*I don't know*, and the deleted table collapsed them. `World::grants` returns
`Grant::{Yes, No, Blocked}` for the same reason, so every consumer had to say
what it does about the third case rather than inheriting a `bool`.

Seven consumers, all migrated, and `contract.rs`'s fallback deleted rather than
threaded. Two things fell out of the deletion:

- `Capability::resolve(effect, types)` became vacuous — it passed an empty
  ontology, and without a table underneath, every call could only answer
  `Undeclared`. It survived exactly as long as there was a table to answer for
  it, which is the argument against having had one. Deleted;
  `resolve(effect, types, ontology)` is the only form.
- `check_effect_rows`'s empty-ontology exemption is gone, on the ruling that
  single-file strictness is correct. What made the silence look harmless was
  the same table: an undeclared effect still had a placement and a capability,
  so nothing downstream noticed the row had never resolved.

**Results.** Every row in `placement_migration.rs` unchanged, and
`docs/evidence/E8/component-contracts.{json,txt}` reproduced byte for byte. One
row moved, in `effect_vocabulary.rs`: `post_paint` joined the
placement-constrained-but-authority-free list, because the old test asked the
table and the table had no `post_paint` entry to speak with. A statement of what
the declarations say, now that they are the only thing that says it.

**The negative control the ruling asked for by name** is
`an_undeclared_effect_gets_no_placement_answer`: `dom.thing` — a family the
deleted table DID restrict, with an operation nobody declares — must not acquire
a placement. It proves the deletion is semantic and not merely that the table
stopped being called in today's fixtures.

**And the structural gate**, `no_source_file_maps_an_effect_family_to_a_world`:
no file under `compiler/pw-core/src` may put a quoted family name on a line with
a `World` variant. The scan stops at each file's `#[cfg(test)]`, because two
test doubles live past it and exist precisely because the code under test no
longer carries a table.

**Step 5's shape**, settled:

```text
candidate nodes = contract.allowed_placements
                  ∩ nodes satisfying required_capabilities
```

`admit(component, node)` stays local; the planner composes those local facts and
never makes `admit` recursive.

**Nothing in the effect ontology is outstanding.** The chain is:

```text
EffectPath + type arguments
        ↓  ontology.rs, the only place a dotted effect name is split
EffectDefId + resolved TypeArguments
        ↓
EffectInstance + semantic facets
        ↓
placement (declared)   capability (declared)   phase legality (facets)
        ↓
CapabilityId → HostInterface → WIT
```

Three structural gates hold it: `last_segment.rs` (no second place splits an
effect name), `no_source_file_maps_a_phase_keyword_to_an_effect_spelling` (a
phase creates an execution context, never an effect), and
`no_effect_is_written_at_two_different_arities`.

### Step 7, as it landed

One boundary-transfer analysis, two policies over it, per the ruling:

```text
                  boundary-transfer analysis      pw-core/src/boundary.rs
                     /                 \
          resume-capture policy      remote-call policy
             resume.rs                  binding.rs
```

`TypeFacts` — is this type a resource, is it produced only by a scoped
declaration — MOVED out of `resume.rs` rather than being copied.
`can_cross(profile, context)` is `Proven | Violation | Blocked`, the same three
answers as `PlacementLookup` and for the same reason.

**Privacy is where the two policies differ, and the difference is principled.**
A session-scoped value may not enter a resume manifest, because the manifest
ships with the document and there is no destination to check. It may cross a
remote call, because there IS one and `World::may_hold` plus the topology
already decides — a second answer here would be the duplication the module
exists to prevent.

`BindingSupport { local, remote }` on each `Export`, not a seventh contract
field: remote capability is a property of an interface edge, so a component
with a remotable query and a handle-passing helper says so about each. Every
position is checked in both directions, results and errors separately —
`recover() -> Result<StoreId, OpenTransaction>` succeeds with a key and fails
with a handle, and a check reading only the success side calls it remotable.

`plan.rs` composes the two independent facts. Four combinations, one failure:

```text
co-locatable + transferable      bind it either way
co-locatable + NOT transferable  a same-process call passing a handle
necessarily remote + transferable   an RPC
necessarily remote + NOT           no binding exists — the only failure
```

**A defect the instrument did not catch and a probe did.** `binding_support`
looked its signature up through `Signatures`, which covers `fn`, `query`,
`command`, `subscription`, `resource` and `task` — and not `page`, `view` or
`component`. Every page export missed the lookup and took the permissive
default, coming out `Transferable`: the right answer for this corpus, from a
mechanism with nothing to do with its types. Every test passed. It reads the
declaration now, which every component kind has. `a_page_returns_nothing_and_
that_is_not_undetermined` is the guard, and it also fixes the second half —
`returns: None` means nothing crosses outbound, not that the build could not
tell.

Evidence: `docs/evidence/E8/binding-feasibility.txt`, via `just e8-binding`.

### Step 13 — deployment planning

`docs/NEXT.md`'s older E8 table item 4, and now unblocked: the compiler emits
`ComponentContract`s carrying declared capabilities and declared placements,
and `runtime/pw-host` admits against a `Topology`. What is missing is the thing
that PLANS — reads a program's contracts and a deployment's topology and says
which component goes where, refusing when nothing fits.

It consumes `allowed_placements` and `required_capabilities` as they now are;
both are declaration-driven, which they were not when the item was written, and
since step 6 there is no hard-coded input left on either side.

### The two rulings this sequence turns on

> **Effect declarations are ambient only because the selected platform package
> explicitly exports them into the Effect prelude. Their arguments are not
> ambient.**

> **`mutate` is a scheduling context, not shorthand for
> `style.mutate<LayoutAffect>`.**

The first is done. The second is step 7.

### Step 7, specified

`effects.rs::intrinsic_effect` maps four frame-phase keywords to effect
spellings, and the ruling is that all four should go:

```text
"measure"    → layout.measure
"mutate"     → style.mutate          (and this one now has the wrong arity)
"post_paint" → paint.post
"animate"    → animation.composite
```

> A frame-phase block says WHEN this body's work executes. An effect says WHAT
> that work actually does. Those are independent.

So `mutate { pure_computation() }` must not claim it mutated style, and
`measure { pure_computation() }` must not claim it read layout. The effect comes
from the operations inside the block.

**Placement still comes from the phase.** `post_paint { pure() }` is
browser-only because the execution CONTEXT has browser semantics, not because a
synthesized effect said so. That constraint wants an `ExecutionContext` concept:

```text
ExecutionContext   when/where scheduled work exists
EffectInstance     what the work does
CapabilityId       authority some effects require
```

The matrix the ruling requires:

```text
empty mutate                   no style effect
mutate + PaintOnly operation   style.mutate<PaintOnly>
mutate + layout write          style.mutate<LayoutAffect>
measure + pure                 no layout.measure
measure + geometry read        layout.measure
```

plus the existing phase-order checks, which must not move.

**Blast radius to expect.** `forbidden_in_phase`, `phase_at` and the corpus
fixtures that turn on phase effects — R-042 (`post_paint` may not measure),
A-022, R-034, R-035 — all read the synthesized spelling today. This is the
change most likely to open another corpus version, and the matrix should exist
before it lands, exactly as it did for step 7 of the previous sequence.

### Slice 2 — what remains### Slice 2 — what remains

1. **Resolve an effect row against the declarations.**

   ```text
   database.read<Stores>
     → EffectPath(["database","read"]) + TypeArg("Stores")
     → EffectDefId + TypeDefId
     → EffectInstance { effect, args }
   ```

   Everything downstream consumes `EffectInstance`. **No checker splits
   `"database.read"` at the dot.**

2. **Declare the vocabulary.** Enumerated below. No wildcards — the ruling is
   explicit: *"A wildcard is convenient documentation but terrible semantic
   identity."* `observe.*` and `durable.*` must become their real operations.

3. **The diagnostics.** Unknown family, unknown operation, wrong generic arity
   — all compile errors, in `PW52xx` beside `PW5200`.

4. **`interface_for` reads the declaration** instead of
   `format!("pw:host/{family}")`, which is the last hard-coded thing in the
   capability path.

### The tests the ruling requires before this is "landed"

```text
effect log                      parses/resolves          slice 1 ✓
effect database.read<T>         parses/resolves          slice 1 ✓
database.read<Stores>           resolves decl + argument
database.read<Stroes>           PW5200                   done
databse.read<Stores>            unknown effect
database.reed<Stores>           unknown effect
database.read                   wrong generic arity
database.read<A, B>             wrong generic arity
foo.read<T> vs database.read<T> never confused           slice 1 ✓
database.read vs database.write distinct EffectDefIds    slice 1 ✓
```

plus the structural one:

> No downstream semantic analysis discovers an effect by splitting or comparing
> its dotted spelling.

which is `last-segment-audit.txt` extended to cover effect spellings.

### The remaining grammar questions — ANSWERED

Three questions, each of which changes the parser, and none of which should be
answered in a hurry:

**1. Dotted effect names — flat, via `dotted_name` in the effect branch only.**
Done. `name()` is untouched.

**2. Type parameters — the SAME binder `opaque type Secret<C>` uses.** Done.
The ruling: *"Don't write an effect-specific generic binder. […] You've already
learned what happens when one representation retains generic arguments and
another quietly drops them."*

**3. Where they live.** `pw-std` for `log`/`trace`, `pw-platform-web` for the
rest. That splits the vocabulary across two packages, which is right — a
non-web host has no `dom.mutate` — but the corpus checks the platform packages
together, so the split is not exercised until something checks one alone.

### The vocabulary that must be declared

From the corpus as it stands, so nothing silently loses its meaning:

```text
database.read<T>   database.write<T>   database.transaction
secret.read<T>     secret<T>
dom.mutate         dom.read            style.mutate<T>
layout.measure     animation.composite paint.custom
observe.*          device.location
network.fetch      cache.read          cache.write
resource.acquire<T>  resource.release<T>
durable.*          log                 trace
```

Roughly twenty-five, and each one wrong is a corpus file that stops meaning what
it says — `LayoutAffect` (`docs/RISK_QUEUE.md` 37) is what one missing
declaration already cost.

### The order to work in

```text
1a  repair the two forbidden last-segment sites          DONE
3   placement consumes effective_effects                 DONE
    the effect/capability ontology above                 NEXT — grammar decision first
4   the remaining unknown-family / unknown-operation diagnostics
5   deployment planner
6   boundary-transfer-derived component binding
7   WIT generation
8   typed linking from Granted
9   add_to_cart through Wasmtime
10  fuel/memory limits (the declared topology replacing worlds_for is DONE)
```

Full E9 still comes after E8. The point of pulling the ontology forward is to
stop E8 freezing today's stringly effect vocabulary into the ABI.

### Step 5's design, ruled 2026-08-07

I asked whether `RemoteCapable` is the predicate the resume manifest already
computes. The answer:

> Remote-capable is not equivalent to resume-serializable. But they are two
> policies over the same underlying semantic fact: whether a typed value can
> safely cross a boundary. Don't build a second `is_remote_capable_type()`
> beside `is_serializable_capture()`.

```text
                  boundary-transfer analysis
                     /                 \
          resume-capture policy      remote-call policy
```

So `pw-core/boundary.rs` owns `transfer_profile(type)` and
`can_cross(profile, BoundaryContext)`, returning `Proven | Violation | Blocked`
— **not a bool**. `resume.rs` adds its version policy; a new `binding.rs` adds
the RPC/ABI policy.

The profile knows: wire schema/codec, privacy label, affine/resource status,
local-only handles, capabilities, schema identity. "Serializable" is too weak a
word for it — `OpenTransaction` could be encoded as a handle number and still
must not be reconstructed elsewhere. The concept is **transferability**.

`BindingMode` as an enum is rejected:

```rust
struct BindingSupport { local: LocalSupport, remote: RemoteSupport }
```

because `Either` is just both, and an enum forecloses "remote ✓ only through a
host-mediated handle proxy".

Remote capability is a property of an **interface edge**, not a component:
`get(StoreId) -> Result<Store, StoreError>` is probably remotable and
`with_transaction(fn(OpenTransaction) -> T)` is not, in the same component.
Arguments, results AND errors are all checked, in both directions.

### A permanent corpus rule, adopted 2026-08-07

After R-037 (`docs/RISK_QUEUE.md` 35):

> Any fixture whose claimed invariant requires propagation across one or more
> semantic edges must contain a discriminating control that breaks one edge and
> causes the target diagnostic to disappear.

R-037 produced *exactly the intended diagnostic for completely unrelated
reasons*, which is worse than an ordinary false positive. Controls to add:
rename the callback parameter (spelling independence); a collection whose
element type has no such member (receiver typing matters); a callback that does
not call the member (the path matters). The same shape applies to privacy
propagation, resource invalidation and placement.

Not yet written. This is corpus work, and it belongs with step 1's audit.

### Step 1, what is done and what is not

**Done.** A member resolves through its receiver's type. `Types::of_body` gives
a callback's parameter the element type of the collection it is applied to;
`effect_rows` consumes that environment instead of building a narrower one;
`param_pattern` binds `fn(el)` parameters, which it never did
(`docs/RISK_QUEUE.md` 35). The program-wide-unique fallback is deleted and the
corpus is 46/46 without it.

**The two items the ruling named, and where each stands.**

1. **The package-declared prelude — DEFERRED, not outstanding.** Architect
   ruling, 2026-08-07:

   > The prelude facility does not need to be invented just to satisfy an old
   > checklist item. Since receiver-directed resolution removed the reason the
   > platform needed ambient visibility, change the status to: prelude design
   > deferred until Pleris actually has a demonstrated need for implicit
   > imports. "The language must have a prelude" isn't itself a goal.

   If a need appears, package metadata defines it and it stays tiny — `Bool Int
   String Option Result List` or similar fundamentals. Implicitly importing a
   whole web-platform module for convenience is the wrong default.

2. **The structural test against global last-segment lookup — DONE.**

   `compiler/pw-core/last-segment-audit.txt` classifies all nine sites and
   `tests/last_segment.rs` enforces it: every site needs a category, a stale
   entry fails, a reason under 40 characters fails, and the scan has its own
   control.

   Categories used: three `display`, two `encoding`, two `scoped`, one
   `syntax`, and **zero `forbidden`**. The ratchet is at 0, which is now a
   ceiling: there is no stock of forbidden sites to hide a new one among.

   The two that were forbidden are repaired (item 1a): `check.rs`'s
   privacy-label lookup resolves through the workspace and `labels.rs`'s
   `declaration_named` is scoped to the module and its imports.

---

## E8's remaining half, in more detail

| # | task | acceptance |
|---|---|---|
| 1 | Generate a WIT world per `ComponentContract` | the world's imports are exactly `contract.imports`, and `wit-bindgen` accepts it |
| 2 | Typed linking from a `Granted` | `linkable()`'s list becomes real `Linker` entries; a component whose contract omits an import fails to instantiate, with the engine's own diagnostic |
| 3 | Run the store's `add_to_cart` as a component | the dev server's command path goes through the host instead of a Rust closure |
| 4 | Fuel and memory limits per instance | E0's `check:fuel` moved from the spike into `pw-host`, driven by policy rather than a constant |
| 5 | Retire `worlds_for` in favour of the declared topology | **done 2026-08-07.** The compiler keeps solving placement; where an effect is meaningful comes from its own `placement` clause, and which node grants which capability comes from the deployment's `Topology`. No table in between |

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
