# ADR-0026 — a capability authorizes an operation; it does not identify one

**Status:** Accepted

**Date:** 2026-08-20

**Refines:** [ADR-0020](ADR-0020-component-contract-is-the-compiler-host-boundary.md),
which fixed the `ComponentContract` as the compiler→host boundary without
saying what a host import *is*.

## The decision

A component's dependency on something outside itself is a **callable import**:

```text
ImportId              interface + operation      store:data/carts#add
Signature             the ABI                    (SessionId, MenuItemId,
                                                  PositiveInt)
                                                   -> Result<Cart, CartError>
Vec<Capability>       what invoking it requires   [database.write<Carts>]
Ownership             who DEFINED it              External | Platform
```

Four separate facts, and none of them is derivable from another.

The relation between capabilities and operations is **many-to-many in both
directions.** An operation may require zero capabilities; a capability may
authorize many operations.

A host implementation is **explicit declaration metadata**:

```pleris
fn add(session: SessionId, item: MenuItemId, quantity: PositiveInt)
    -> Result<Cart, CartError> !{ database.write<Carts> }
    host "store:data/carts#add"
```

It is never inferred from a missing or `todo` body, and never from an effect
row. A `fn` with a privileged row and no `host` policy is ordinary compiled
Pleris, however privileged it is.

## What the encoder proved

`Instr::HostCall { capability, args }` keyed a core Wasm import on a
**capability**. The store's `add_to_cart` and `clear_cart` both call
`database.write<Carts>` — through `Carts.add(s, item, qty)` and
`Carts.clear(s)` — so the IR carried one capability with three arguments at one
site and one at the other.

A core import has **one** signature. There was no honest number to put in the
import's type:

```text
wasmparser: type mismatch: expected i32 but nothing on stack
```

The second attempt dropped the ambiguous import and produced *"exported
function index out of bounds"*, because the function index was computed from
the declared import count rather than the emitted one. Both mechanical bugs
were real; neither was the defect.

Architect ruling, 2026-08-20:

> **A capability authorizes an operation. It does not identify the operation.**
> So `database.write<Carts>` must never be used as the callable import identity.

## The ownership field, and why it is not decoration

> `pw:host/carts#add` wrongly implies Pleris defines a universal carts API.
> […] Do **not** let `pw:host/carts` become the permanent standard-library
> design merely because it was the first thing that made the demo executable.

So who *defines* an operation is recorded separately from who *implements* it
today:

```text
pw:host/session#read      Platform    a Pleris facility, ABI-stable
store:data/carts#add      External    the application's own, externally
                                      implemented for now
```

"The host process currently provides the implementation" is a deployment fact
and does not collapse their semantic ownership.

This field earned its place within a day. It decides who owns a host
operation's **ABI** — see the open question below — and the answer differs by
ownership, which is why the distinction had to land before the Canonical ABI
work rather than after it.

## The audit is three layers, never one

```text
artifact audit        did you import only the operations your contract names?
contract consistency  does your contract possess every authority those
                      operations require?
admission             does this node actually grant that authority?
```

> Do not combine those into one check.

`contract::consistent` is the middle one, and the relation is **⊆, never
equality**:

> because a component can have required authority whose use isn't represented
> by a particular callable import shape, and we shouldn't force the ABI to
> become the definition of semantic effects.

A component may require more than its imports do; it may never require less. An
operation requiring nothing is satisfied by a component holding nothing, which
the browser-semantic effects already need.

The artifact-audit layer asks **two** questions, kept apart:
`contract::audit` decides identity, and `contract::abi` decides shape — *same
`interface#operation`, wrong ABI → rejected*. The second is not folded into the
first because the first is satisfied by an artifact importing the right name
with any type at all.

## What was deleted, which is the substance

**The effect-row classification rule.** It read *"the callee's declared row
contains a capability this component requires, therefore host call."* That is
true of `Carts.add` and does not make `Carts.add` a host function.
`an_effectful_function_without_a_host_binding_is_an_ordinary_call` is what says
the rule was removed rather than a field added: one declaration, one line of
difference, two outcomes.

**`interface_for(capability, ontology)`.** It turned a capability into an
interface name — the conflation living one layer above the encoder. Its absence
is why the contract now has to be *told* the operation.

**`todo` meaning "the host implements this."** Two materially different
semantics that shared a spelling.

Residue: an `effect` declaration still carries a `host` clause, and the ontology
still reads it into a field nothing consumes. `backend::host_binding` refuses a
`DeclKind::Effect` so the two cannot be confused, but removing the clause from
the effect vocabulary is not done.

## Consequences

A deployment must publish a WIT package for the **operations** it supplies, not
for the capability families it grants. Those are different lists, and the WIT
stand-ins were rewritten accordingly — into two packages, because a deployment
publishes the platform's operations and the application's.

## Open: who owns a host operation's ABI

Recorded here because this ADR is what made the question askable.

A host operation's ABI is now derived **twice** — by the contract, from the
Pleris `fn` carrying the `host` binding, and by the deployment, in the WIT it
publishes — and nothing compares them. All six of the store's operations
disagree while every gate stays green.

The Canonical ABI does not expose this; it hides it. The two flatten to the
*same* core signature, so the disagreement lives entirely in the component
types. `docs/RISK_QUEUE.md` carries the classification and
`tests/canonical_abi.rs` pins it.

The answer appears to differ by `Ownership` — an operation the application
declared is one the compiler should arguably publish; a platform facility is
one the compiler must conform to — and it is with the architect.

## Amended, 2026-10-11: the artifact audit builds the guests it reads

`just e8-host` (`spikes/wasmtime-component/audit.sh`) refused where the two
WebAssembly guests were not built, and pointed to `just spike-wasmtime`.
That spike also rewrites E0's evidence. A verification runner never had the
guests, so every nightly verification failed the recipe without running it.
The audit now builds each guest itself
(`cargo build --locked --release --target wasm32-wasip2`, each into its own
`target/`, where the tests read it). `rust-toolchain.toml` already installs
the target.

Its first run since then failed, and so did the recipe: a test the rule of
2026-10-01 (`e20caefc`, a call reaches only an export whose place in the
component the contract names) had made a refusal. `tests/artifact.rs`'s
contract declared `lookup` with no place, so its granted guest's call was
refused as "not an export declared by component contract `spike.Store`".
Every test that reads a guest is ignored where the guests are not built, and
none was run for ten days. The test's contract now names the export's place.
The evidence names its commit, and so does `e9-parity`'s, which named none.

The E10 benchmark that the audit runs with them (`tests/bench.rs`, which
`just e10-bench` runs in release) had rotted the same way. Its node granted
the two capabilities the command required when the benchmark was written, so
the command was refused admission: it has since read the menu (ADR-0178) and
handed the outbox its event (ADR-0246). The benchmark also passed an id where
the command now takes the item (ADR-0178), and a cart line without its name
(ADR-0172). It now grants what the contract requires, read from the contract.
It answers each import the contract declares, or names the one it does not.
It passes the item and the line as their types declare them. All of
`pw-host`'s tests pass with the guests built, ignored ones included.

## Amended, 2026-10-13: a failed audit prints why

The nightly verify of 2026-10-12 (38271162931) failed `e8-host` with exit
code 101, and its log held one line: just's, saying the recipe failed. The
audit captured its tests' output in a variable and printed it afterwards; a
failing `cargo test` ended the script at the capture (`set -e`), and nothing
was printed. That failure was the benchmark's rot this ADR's amendment of
2026-10-11 names, fixed the same evening (`4813aea0`), and was found by
running the tests again locally. The audit now prints the tests' output
whatever they answer, then exits with their status and writes no evidence
(`scripts/tests/test_evidence_gates.py`: a failure at the tests shows their
output and writes nothing; tests that pass write the evidence with their
lines).
