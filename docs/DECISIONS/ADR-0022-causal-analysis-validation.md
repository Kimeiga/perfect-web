# ADR-0022 — Causal-analysis validation: test the reason, not the answer

**Status:** Accepted
**Date:** 2026-08-07
**Milestone:** E8 (methodology; applies from here on)
**Depends on:** the admissibility rule in `docs/RISK_QUEUE.md`

## Context

Nine risk-queue entries landed in one session, 28 through 39. Five are the same
failure, and it now has a name.

> **Coincidental correctness:** the compiler emits the right verdict, but the
> semantic information that is supposed to justify that verdict never actually
> flowed through the program.

`R-037` is the clearest instance. The fixture claimed that `layout.measure`
propagates through `List.map`'s callback. Its callback parameter was never
bound, so `el` had no type and no receiver-directed member lookup happened. The
effect was found by matching the spelling `getBoundingClientRect` against every
declaration in the program. **The diagnostic was correct, the code was correct,
the invariant was correct, and the mechanism had no connection to the claim.**
The fixture would have passed identically had it written `fn(anything_at_all)`.

The other four: a materializer race written off as flakiness; `LayoutAffect`
naming nothing across five files and three milestones; a generality witness
declaring an effect its body never performed; three historical corpus texts
catchable only through ambient resolution.

**None was found by a test going red.** Every one surfaced when a change removed
the mechanism propping it up. Which ones were found depended on what happened to
be changed next.

The existing admissibility rule — positive control, negative control,
plausibility, raw evidence — does not catch this. Nor does the broken-path
control rule adopted after R-037: that catches a fixture whose chain is broken,
and R-037's chain was intact in the sense that every link existed. The link that
mattered was carrying no information.

## Decision

**Important compiler conclusions must be able to explain which resolved semantic
facts caused them.**

Not the diagnostic text — the compiler must be *capable* of producing the chain,
and tests must be able to inspect it.

```text
Parameter el
  ↓ bound by            PatternId 81
  ↓ inferred as         ElementRef
  ↓ resolves member     ElementRef.getBoundingClientRect : DefId 203
  ↓ introduces          EffectDefId(layout.measure)
  ↓ through callback    BodyId 74
  ↓ through helper      DefId(List.map)
  ↓ reaches             View BodyId 18
```

The broken implementation could still emit `layout.measure`. It could not
produce that chain — its explanation would show a program-wide spelling match
and no receiver-type edge. **The test goes red while the diagnostic stays
green.**

### One vocabulary, not one per checker

```rust
Fact {
    id: FactId,
    kind: FactKind,
    origin: Origin,
    depends_on: Vec<FactId>,
}
```

`ResolvedMember`, `Type`, `Effect`, `PrivacyLabel`, `Capability`,
`PlacementDemand`, `ResourceDependency`, `MaterializationInvalidation`. The
facts form a DAG; a diagnostic points at one or more terminal facts.

The graph need not be retained in optimized builds. It is for diagnostics,
`pleris explain`, tests, semantic PR reports and dev mode.

### Tests assert edges, not whole proofs

Asserting an exact proof is brittle and would be rewritten on every refactor.
Assert **required semantic edges** and **forbidden evidence sources**:

```text
R-037 must contain          R-037 must NOT contain
  callback parameter binding  program-wide name lookup
  receiver type ElementRef    unresolved receiver
  resolved member DefId       spelling-only member match
  EffectDefId(layout.measure)
  propagation through callback
  propagation through generic helper
```

### The fixture rule grows from three checks to five

```text
1  the bad program is rejected
2  a valid neighbour is accepted
3  breaking a propagation path makes the diagnostic disappear
4  SEMANTIC PROVENANCE — the result is justified through the path the
   invariant claims
5  IRRELEVANT PERTURBATION INVARIANCE — changing what should not matter
   leaves the conclusion identical
```

Rule 5 attacks accidental dependence on spelling, ordering, uniqueness and
repository composition — the exact substrate of every failure above.

### Two mutation classes, automated

Not five hand-written files per invariant. A metamorphic harness generating:

```text
semantics-PRESERVING          expected: conclusion unchanged
  alpha-rename a local          exposes spelling dependence
  reorder independent modules   exposes ordering dependence
  insert a same-spelled decoy   exposes uniqueness dependence
  add an unrelated import       exposes composition dependence
  wrap a value in an identity helper
  rebind through a `let`

semantics-BREAKING            expected: conclusion changes
  ElementRef → a type without that member
  remove `invalidates_on`
  Secret → Public
  effectful callee → pure callee
```

Together: **the compiler depends on what matters and does not depend on what
does not.**

### Acceptance needs provenance too

Several false positives were found only through programs that must be *accepted*.
For headline safety invariants, an accepted neighbour should establish *why* it
is acceptable:

```text
fragment partition: Public
dependency label:   Public
Public can_flow_to Public
→ edge permitted
```

so that "accepted" cannot mean "the checker failed to run". That was precisely
the defect in the E6F witnesses.

### Risk-weighted, not universal

**Tier 1** — the analyses that have already produced coincidental correctness:
name/member resolution, effect inference, privacy labels, placement and
capabilities, resource invalidation.

**Tier 2** — type inference, affine/resource checking, resumability,
transferability, deployment planning.

A local syntax rule like "duplicate attribute" needs no proof DAG.

### The gate

```text
semantic evidence
    headline analyses: n / n

To close one:
  ✓ the violating program has the expected provenance
  ✓ the accepted neighbour has the expected PERMITTING provenance
  ✓ breaking the causal path changes provenance or verdict
  ✓ semantics-preserving perturbations do not change the conclusion
```

A **new hardening gate**, created by evidence the implementation produced. It
does not retroactively reopen closed milestones.

## Consequences

- Effect ontology slice 2 is the first consumer, and is **not** delayed for
  this: the provenance mechanism is built into it. Resolution from
  `EffectPath` → `EffectDefId` → `EffectInstance` produces evidence at each
  transition, and a test asserts no later analysis splits `"database.read"`.
- `pleris explain` gains real answers to *why*: why this page is private, why
  this component is origin-only, why this render may not measure layout. That
  is not test scaffolding — it is what a semantic compiler owes its users, and
  it is unusually valuable to an agent reading the compiler's output.
- The admissibility rule in `docs/RISK_QUEUE.md` gains a fifth requirement for
  Tier 1 analyses: **provenance**.

## What would falsify this

A Tier 1 analysis whose provenance is expensive to produce and adds nothing a
targeted test could not assert directly. If the DAG becomes a parallel
implementation of the analysis — a second answer to the same question — it is
the pattern this project exists to delete, and the ADR is wrong.

The distinguishing question: does the provenance *record* what the analysis did,
or *recompute* it? Only the first is admissible.
