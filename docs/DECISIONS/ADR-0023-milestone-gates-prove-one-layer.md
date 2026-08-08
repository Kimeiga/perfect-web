# ADR-0023 — A milestone gate proves one layer's contract, not two

**Status:** Accepted
**Date:** 2026-08-08
**Strategy:** build (this is a rule about evidence, not about code)

## Context

E8's fifth gate item read:

> Run the store's `add_to_cart` as a component, so the dev server's command path
> goes through the host instead of a Rust closure.

It cannot be met at E8. Executing a *Pleris-compiled* component requires a
Pleris→Wasm-component backend, and there is none: `pw emit-koka` lowers a pure
subset (ADR-0015) with no code generator behind it. That backend is **E10**.

```text
E8 host
    supposedly must execute Pleris-generated Wasm
                    ↑
E10 Pleris→Wasm backend
```

A dependency inversion in the plan, not in the code. The charter's rule — *do
not start the next milestone until the current one's gate passes* — made it
blocking rather than cosmetic.

Three ways out were put to the architect:

```text
1  close E8 on the items it can meet, and move the fifth to E10
2  hold E8 open until E10 lands, and work E9 in parallel
3  build a minimal backend inside E8 for one command
```

## Decision

**(1), with the gate amended explicitly rather than the item quietly dropped.**

Architect ruling, 2026-08-08:

> E8 cannot honestly require an artifact that only E10 knows how to create.
> Building a temporary backend inside E8 would be exactly the sort of duplicated
> mechanism this project has repeatedly had to delete.

and the general rule that follows from it:

> **Milestones should prove one layer's contract independently. Cross-layer
> end-to-end proofs belong at the milestone where both sides of the boundary
> actually exist.**

So E8's claim becomes a claim about one layer:

> Given a component contract and a Wasm component artifact, the host can
> determine what authority it may receive, verify what the final artifact
> actually imports, link only granted interfaces, constrain its resources, and
> execute or refuse it accordingly.

which does not require the component to have been generated from Pleris. A
hand-written Rust/WIT fixture is the right positive control, because what is
under test is the host.

The end-to-end proof becomes a **deferred integration obligation**, recorded in
two places so neither can close without it:

```text
docs/EVIDENCE_LEDGER.md   "Deferred integration obligations", E10-I
docs/MILESTONES.md        E10's own row
```

> **E10-I:** compile `add_to_cart` through the production Pleris→component
> backend and execute it through the E8 host, **with no alternate Rust closure
> path**.

## What this does NOT change

**The "gate must pass" rule stands.** What moved is one gate, and only because
it was asking one milestone to demonstrate two layers at once. A gate item that
is merely *hard* is not a candidate for this; a gate item whose dependency is
owned by a later milestone is.

**Option (3) is rejected on the record.** A temporary backend built to make old
wording green is how a temporary backend becomes the backend — the same shape as
Koka (ADR-0001) and Marko (ADR-0002), both of which needed explicit deletion
conditions to stay temporary, and neither of which was built under a gate
deadline.

## Consequences

- E8 closes with eleven controls, including one the ruling added: **a permitted
  guest calling a granted host interface and receiving the host's answer.**
  Everything else in E8 shows authority being refused or linked; without this,
  the capability system had only ever been observed saying no.
- E9 may begin. The effect ontology was pulled forward into E8 precisely so E9
  can change the inference algorithm, row representation and syntax without
  changing `CapabilityId`, `ComponentContract` or host semantics — so E8 no
  longer needs to be held hostage to it.
- The dev server keeps a Rust closure body. Its *authority* is decided by
  `admit`, which is the half that needed no backend, and E10-I's closing
  condition is explicit that "both work" is the obligation avoided rather than
  met: the closure path must be **deleted**, not left beside the component one.

## What would falsify this

A second gate item deferred for the same reason, which would mean the milestone
sequence has a structural inversion rather than one mistake. Two deferrals is a
signal to re-derive the milestone order from the dependency graph instead of
patching it one obligation at a time.
