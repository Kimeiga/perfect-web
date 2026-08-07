# ADR-0020 — The `ComponentContract` is the compiler→host boundary, and it is frozen

**Status:** Accepted
**Date:** 2026-08-07
**Milestone:** E8-0
**Depends on:** ADR-0018 (the manifest is data), E5's placement solver, E2C's
capability→world table

## Context

E8 puts application code inside Wasmtime with no ambient authority. Two systems
have to agree about what a component is allowed to do, and they are written in
different places for different reasons:

- the **compiler** knows what the code needs, because it inferred the effect
  rows and solved the placement constraints;
- the **host** knows what this deployment has, because it holds the database
  handle, the secret store and the topology.

The architect's ruling of 2026-08-07 states the invariant:

> The compiler decides what authority code needs. The host decides whether that
> authority physically exists. Neither should reconstruct the other's answer.

The failure mode is not hypothetical. If the host re-derives what a component
needs — from its exports, from a naming convention, from a capability table of
its own — then two answers exist, and they diverge silently the first time one
of them changes. E2C's deletion gate exists because that had already happened
once at a smaller scale: a checker that knew `secrets.payments` yields a secret
was a second copy of a fact the effect row already carried.

## Decision

**One data artifact, six fields, frozen at E8-0.**

```text
component_id           which component, semantically
abi_schema             a hash over its INTERFACE
required_capabilities  what authority it needs
allowed_placements     where E5's solver says it can run
imports                the host functions it may call
exports                what it provides
```

`compiler/pw-core/src/contract.rs` derives it; `pw emit-contracts` writes it as
JSON; the host deserializes it and mirrors the types by field name. Neither
crate links the other — ADR-0018's boundary, applied a second time.

### The rule

```text
actual Wasm imports  ⊆  the contract's allowed imports
```

**Subset, never superset.** Fewer is fine: dead code, a branch never compiled
in. Even one import outside the set is not a small mistake — it is a component
whose authority the compiler never approved, and the host has no basis for
deciding whether it should have it.

The audit is membership in a set, so it is fail-closed by construction: there
is no parse to fail and no unknown-interface branch to forget.

### One contract per declaration

Not per module. A module holding a database query and a browser component would
otherwise give the component the query's `database.read` and the query the
component's `dom.mutate` — over-granting both, and then E5's solver finds
nowhere either can run, because the union of two satisfiable demands can be
unsatisfiable.

It also buys a property module-wide contracts cannot have: **a new declaration
is a new contract and every existing one is byte-identical**, so a host reloads
exactly what changed.

### A page does not inherit its handlers' authority

A page that renders `on:press={.. => add_to_cart(..)}` contains a call to a
command that writes the database. It does not perform that write — the handler
does, when someone presses the button, and E7-L already made the handler a
separately loaded unit with its own identity.

So contract derivation excludes lambda subtrees from a component's own effects
(`Inference::infer_excluding`). The authority is not lost: the command is itself
a component with its own contract, so `database.write` is recorded once, against
the thing that performs it. Without this the store page required
`database.write` **to render**.

### The capability keeps its type argument

`database.read<Stores>` and `database.read<Payments>` are different
capabilities, and `Capability` is a struct rather than a string so the host
decides on the parts. E2D recorded what dropping the argument costs: two
fixtures were reported for an effect their own row declared, and the coverage
number went *up* while detection got worse.

### What the compiler does NOT say

It does not say what a capability means. `interface_for` maps a family to a
name — `database` → `pw:host/database` — and stops. Whether such an interface
exists, what it connects to, and whether this deployment has one are the host's
questions. A compiler that answered them would be a second deployment topology.

### A component dependency is not authority

**Amendment, 2026-08-07.** Architect ruling:

> A component import isn't automatically "authority". It is a dependency on
> another component whose own authority is independently constrained.

So the three fields divide as:

```text
required_capabilities   things supplied by the host/environment
imports                 exports required from other application components,
                        AND the host interfaces those capabilities are served by
exports                 application interfaces this component offers
```

Each import carries an `ImportKind`. The store page depends on five components —
three queries it reads and two commands its handlers call — and requires no
capability of its own. Two of those components need `database.read` or
`database.write`; the page needs neither.

The audit CLASSIFIES rather than flattening, and each class is compared against
its own source. A flattened set would let a component interface fill a host
capability slot and the reverse, and then "this component may call the database"
and "this component may call the Store query" become one permission.

Classification is by **membership, not spelling**. The compiler emits
`pw:host/…` and `pw:app/…`, but a component built against a hand-written WIT
world uses that world's names — refusing those on a prefix would make the audit
a check on naming rather than on authority.

The one exception is `is_runtime`: a `wasi:*` import is refused whatever the
contract says. No effect row asks for it and no node grants it, so a contract
cannot authorise one by listing it, and that check runs before membership for
exactly that reason.

Finding a page's dependencies needs both a call walk and `graph::queried` —
`let menu = query Menu(id)` is a keyword form, not a call — and it must prefer
the COMPONENT namespace: `store.page` declares `query Store` and imports
`domain.Store`, and the general resolver tries types first. That is the E6
defect where every page dependency resolved to a record definition, arriving in
a second place.

### The capability mapping is versioned

**Amendment, 2026-08-07**, on the architect's ruling that WIT generation must
not hard-code today's effect syntax:

> Separate semantic identity from representation. […] E9 is free later to change
> the inference algorithm, source notation, internal effect-row representation
> and polymorphism machinery without changing `CapabilityId(DatabaseRead,
> Stores)` — unless E9 actually proves our semantic capability ontology itself
> was wrong.

So the contract carries `capability_mapping`, and a host that reads a version it
does not understand **refuses** rather than interpreting the capabilities under
its own. Two mappings can spell one capability the same way and mean different
authority; a host that guessed would be guessing about exactly the thing it
exists to decide.

`Capability::resolve` is the construction path, and it resolves the type
ARGUMENT against the program's declared types. `database.read<Stroes>` is
reported rather than accepted — otherwise it becomes a capability nothing will
ever grant and the failure appears at deployment.

An unresolvable argument **keeps** the capability. Over-stating is refused work;
under-stating is authority nobody approved, and silently dropping a capability
whose argument was misspelled is the second one.

What is NOT yet resolved: the family and operation are still read from the
row's spelling. A declared capability table — so `database.read` is a
declaration rather than a string — is E9's, and `CAPABILITY_MAPPING` is what
makes that change legible when it happens.

## Consequences

- E8 consumes six fields and may add none. A seventh would mean the compiler is
  answering a question it should be asking, or the reverse.
- `pw emit-contracts` joins `emit-graph`, `emit-manifest` and `emit-template` as
  compiler output the runtime reads as data.
- The effect inference now has a scoped form. `infer_excluding` is used only
  here today, but the distinction it draws — *this body performs* versus *this
  body mentions* — is the same one E9's effect rows will need.
- `worlds_for` remained in the compiler when this was written — a hard-coded
  family→world table, the one place the deployment shape lived, with
  `allowed_placements` as its output rather than a second copy. **Deleted
  2026-08-07.** Where an effect is meaningful is the `placement` clause on its
  own declaration; which node grants which capability is the deployment's
  `Topology`. `allowed_placements` is still the solver's output and still not a
  second copy, and there is no longer a third thing in between. An effect the
  program does not declare now gets no placement answer at all, rather than the
  table's `None` — which was read as *grants it*, and made `secret<Payments>`
  placeable in the browser for a milestone.

## What would falsify this

A host that cannot decide whether to instantiate a component from these six
fields alone. If E8 finds itself reading the template IR, the graph, or the
source to answer an authority question, the boundary is in the wrong place and
this ADR is wrong rather than merely incomplete.
