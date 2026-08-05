# spike: koka-row-polymorphism (risk-retirement experiment RQ-2)

**Question:** does Koka propagate a callback's effects through higher-order
abstraction, or can an effect disappear behind a generic helper?

This was the highest-leverage remaining unknown. The project architect's own
framing: *"If Koka fails here, its usefulness as an effect oracle is much smaller
than assumed."* Because that is exactly the kind of result that gets explained
away after the fact, **the decision table was pre-registered before the
experiment was written.**

## Run it

```bash
just rq-row-polymorphism
```

Evidence: `docs/evidence/M0/spike-koka-row-polymorphism.txt`. Koka 3.2.3.

## Pre-registered decision table

Governing criterion: **Koka may overstate an effect, but must never silently
understate or erase one.**

| observed | E1 | Koka remains oracle? | pull `pw` effect checker forward? |
|---|---|---|---|
| 1. automatic propagation + selective narrowing | pass | yes | no |
| 2. definition-level annotation, conservative omission, narrowing works | conditional pass | yes | no |
| 2′. annotation omission silently understates, or call sites need manual rows | **fail** | no | yes |
| 3. higher-order effect disappears | **fail** | no | yes |
| 4. cannot discharge selectively while preserving the remainder | **fail** | no | yes |

## Result — Outcome 1, clean pass

### Case A: propagation through *unannotated* generic helpers

`kk/inferred.kk` declares `my-map` and `transform-items` with **no type
signature at all**, then routes a database-reading callback through both:

```text
my-map            !{polymorphic:e}
transform-items   !{polymorphic:e}
page-loader       !{database-read}     <- propagated through TWO generic layers
pure-loader       !{}                  <- stays total through the same helpers
```

Koka **inferred** `forall<a,b,(e::E)> (xs : list<a>, f : (a) -> e b) -> e list<b>`
without being asked. No annotation was required at the declaration or the call
site, so this is Outcome 1 rather than Outcome 2.

### Case B: selective discharge

```text
both-effects      !{database-read, trace-effect}
discharge-db      !{trace-effect}      <- db discharged, trace PRESERVED
discharge-trace   !{database-read}     <- trace discharged, db preserved
discharge-all     !{}                  <- both discharged
leaky             !{secret-read}       <- a generic helper cannot HIDE a capability
```

Narrowing works in both directions and reaches `{}` when everything is handled.
`leaky` is the important one: it pushes a secret-reading callback through the
same unannotated generic helper, and `secret-read` still surfaces in the row.

### Negative case

`kk/negative-total-helper.kk` annotates a helper's callback as **total** and then
passes it an effectful one:

```text
kk/negative-total-helper.kk(21,27): type error: effects do not match
check:total-helper-rejects-effectful-callback=pass
```

Without this, every positive result above would be worthless — the effect system
could be bypassed by declaring a helper pure.

## Findings

**F-1 — Outcome 1. Koka is a sound effects oracle through higher-order code.**
Automatic inference, propagation across two unannotated layers, correct
narrowing, and no capability hiding. E1's effect scope is on solid ground and the
`pw` effect checker is **not** pulled forward on these grounds.

**F-2 — my `.kki` effect reader had a defect that this experiment exposed, and it
was a dangerous one.** An effect-polymorphic result is written as a bare row
variable of kind `E`:

```text
-> (e :: E) list<b>
```

with no angle brackets. `effectNames()` returned `[]` for that shape — reporting
**`my-map` as a pure function**. A generic helper that faithfully propagates its
callback's effects was indistinguishable from a total one.

That matters because E1's gate item is *"every effectful example has a visible
inferred effect."* The tool the gate depends on would have passed while hiding
precisely the case the architect flagged as most important. Fixed: a bare row
variable now reports `polymorphic:<var>`. The existing 16 Koka spike tests still
pass.

This is the third measurement bug of exactly this shape in this project — the
layout benchmark that stopped invalidating layout (F-2 there), the mutation
counter that counted the browser's own parse (RQ-1 F-1), and now this. All three
produced *plausible, favourable* numbers. The pattern is worth naming: **a
measurement that cannot fail is not a measurement.** Every check in this
repository should be paired with a negative case proving it can go red.

**F-3 — the "conservative overstatement" property was not directly tested.**
The pre-registered criterion allows Koka to overstate an effect but never
understate one. Outcome 1 means the question of what happens when an annotation
is *omitted* never arose — inference simply produced the right answer. So the
project has evidence that Koka does not *understate*, but no evidence about how
it behaves in a case where it would have to choose between over- and
under-approximation. Recorded as untested rather than assumed safe.

## Limitations

- **Two layers of generic indirection, not arbitrary depth.** `page-loader →
  transform-items → my-map → callback`. Deeper or mutually recursive helper
  chains are untested.
- **No higher-order effects through data structures** — a callback stored in a
  record or list and invoked later is not covered.
- **No async or task effects.** `task.spawn` is not modelled by Koka at all in
  this project, so structured concurrency remains outside the oracle's reach
  (see the open E2A-S question).
- **Handlers here are all `fun` (tail-resumptive).** `ctl` handlers that capture
  or discard the continuation may behave differently and are untested.
- Nothing here is generated by `pw`; these are hand-written Koka modules. That
  `pw`'s lowering will *produce* correctly effect-polymorphic Koka is a separate,
  unproven claim.
