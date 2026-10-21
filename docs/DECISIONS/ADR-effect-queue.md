# Effect inference revisits readers when their dependencies grow

Status: isolated candidate; numbering, exact hosted verification and integration
remain pending. This does not change the verified master or the frozen train.

ADR-0375 removed an unsound eight-sweep limit. A caller-first chain of 250
helpers then required repeated scans of every body: the original bounded local
diagnosis measured 572ms in inference, versus 10ms for the same callee-first
chain. Correct convergence is required in both orders; the answer cannot depend
on a depth cap or declaration order.

The existing invocation graph cannot schedule this work: a named function value
and a non-invoked function member carry effects without being invocation edges.
Instead, the existing three canonical known-row reads record their resolved
declaration identities. The direct-call fallback, named-value fallback and typed
member row retain their existing resolution, declared-row authority and generic
type substitutions. Recording an absent or empty row is necessary: it can grow
after its reader was visited.

Every implemented body starts in a deduplicated queue. Each visit records its
reverse dependencies before publishing row growth, so self-recursion can revisit
itself. Only growth enqueues readers; a reader already waiting is not added again.
Every visit replaces the complete performed metadata even if its permitted row
was already sufficient. A declaration's own row permits effects; it does not
perform them. Sources, call charges and provenance must therefore refresh when
a late dependency changes, including when another dependency already contributed
the same effect.

Rows still grow only by union with the finite set of source/signature effects.
Only row growth schedules new work, so recursive components converge without a
fixed iteration limit. The queue adds a reverse-reader map and a pending bit per
body; it adds no persistent cache and changes no public `Inferred` contract.
Worst-case work still depends on the number of distinct row-growth events and
dependency edges. This is not a claim that every compiler workload is linear.

Five queue witnesses compare both declaration orders across canonical rows,
performed effects, source text/reasons, charged calls and provenance. They cover
empty/absent late rows, direct/member/named-value chains, a self-cycle with a pure
neighbor, declared-row metadata, a non-invoked member with extra parameters,
100-way fan-in, and a staggered read/write diamond. Deterministic visit assertions
bound the one-effect chains of 15/100/250 bodies by twice their body count and
require just one pending-reader refresh for the fan-in. Six targeted controls
must fail their named assertions. Existing convergence tests additionally check
valid program diagnostics and exact query capabilities, including value cycles
and non-invoked members.

Visit counting uses a private generic callback on the same inference path. The
public `run` passes a zero-capture no-op; tests pass a local counting closure.
There is no stored counter, dynamic dispatch or public instrumentation API.
The first candidate's exact CI (39528431883) correctly refused mid-source
`cfg(test)` fields and blocks: the diagnostic census permits that boundary only
before test modules. The census is unchanged. Its twelve tests, the five queue
witnesses and six convergence tests independently pass with the callback repair.
The [repair measurement record](../evidence/tooling/effect-queue-instrumentation.txt)
retains that failure and repeats the same alternating benchmark and native
controls against the repaired source.

The existing eight-sweep and first-sweep controls now cap queue waves: each wave
processes the pending set present at its start. They still truncate actual deep
propagation. The value-only and rooted-member/value controls retain their faults
with the new optional dependency collector. The touched value campaign no longer
credits failed builds. Both it and convergence use the existing native Cargo
exit/summary authority plus a complete named inventory and exact assertion source
line/column. Native tests hold custom-message assertion failures as valid and
refuse compile failures, runtime panics, assertion-operand panics, missing tests,
ignored tests and contradictory native exits. All producers use the shared
process owner and restore source after each control.

The [bounded local measurement record](../evidence/tooling/effect-queue-measurements.txt)
alternates old/queue/old/queue on the same Linux/WSL machine and pinned Rust debug
build. Each entry is a median of five samples after one warmup. The isolated
inference phase reuses parsed HIR/signatures and their warm type-solving cache;
the full checker measurements rebuild the normal checker pipeline.

| Workload | Old medians (ms) | Queue medians (ms) |
| --- | ---: | ---: |
| 250 caller-first helpers, inference | 554.74 / 558.87 | 9.53 / 9.59 |
| 250 caller-first helpers, full checker | 750.36 / 737.82 | 199.66 / 198.04 |
| 250 callee-first helpers, full checker | 196.10 / 198.93 | 187.99 / 199.87 |
| Reference feed, full checker | 227.23 / 225.49 | 217.22 / 226.97 |

All 23 canonical snapshots are identical across all four runs, covering rows,
performed effects, source text/reasons and charged calls. Valid-program and exact
capability assertions also pass. Callable identities use qualified paths;
unchanged non-callable declarations retain their IDs, so repeated clause names
cannot overwrite snapshots. The table uses the callback repair's repeated run;
the earlier record remains historical evidence. This bounded debug measurement establishes the deep
chain improvement; it does not imply a whole-framework speedup or replace exact
hosted verification. The new recipe is `just e14-effect-queue`; existing reached
recipes include `e14-effect-convergence`, `e10-effects-through-values` and
`e14-generic-effects`.
