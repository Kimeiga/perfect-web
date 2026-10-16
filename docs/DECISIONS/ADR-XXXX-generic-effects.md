# ADR-XXXX: a member read retains its type and effects through generic calls

Status: accepted under the integrator's generic-effects brief of 2026-10-16;
ruling 2026-10-15 (client date); implementation and evidence in progress. Date: 2026-10-16. Milestone: E14.

## Context

At master 22a0508e, a view declared pure could reverse a list and read a
database-backed member of each element without PW0401. The value relations
already solved generic calls, but infer.rs answered the callee's declared
List<T> with T unsolved. Other analyses read that type and found no member.

The reproductions also exposed ordinary for-loop and expression-match
binders with no type, chained receivers skipped by the effect walk, generic
record fields retaining their declared T, and members with inferred rows
charged only their empty declared rows. Privacy and capability contracts
read the same incomplete information.

## Decision

Reuse values.rs's call solver for missing or unsolved types in infer.rs.
A callee's type parameter is never a type of a value in the caller; a rigid
parameter of the enclosing declaration remains rigid. Resolve generic
record fields using the same substitution as the value relations. Cache
owned solved expression and binding types per body on Signatures so the
many analyses share one solve, without a second generic-call solver.

Read members through the receiver's type for every receiver expression,
not only bare names. Charge both a member's declared effects and its body's
inferred effects. Rooted effect walks use the same member and value
inference as ordinary bodies. Labels share the member-reading decision.

A member read on a rigid type parameter is PW0610: no constraint declares
that member. A member read whose receiver neither declarations nor the
solver type is PW0408, rather than an unchecked effect/privacy hole. This
tightens accepted programs, ruled by the integrator on 2026-10-15.
Any-valued unreachable expressions (todo and empty-list elements) retain
their existing treatment. Function-type effect rows remain ADR-0078's
separate limitation.

## Acceptance and evidence

In progress: generic_effects integration tests, whole-workspace tests,
corpus R-090+ / A-039+, generality witnesses, standard-package non-vacuity,
mutation controls and just ci. Exact commands and counts will replace this
paragraph when measured.

The standard-package test enumerates generic functions independently from
HIR and source text, then generates one call and member-read probe for
each. It checks solved result types and expected diagnostics, including
functions whose results do not contain elements and constructors whose
arguments cannot solve a type parameter.

## Scope and consequences

No dependency, runtime representation or backend specialization is added.
A checker's solved type is not proof that every generic program compiles
through a backend. Corpus conformance, generality and language soundness
remain distinct claims. Adding PW0408 moves the published invariant and
generality denominators; the integrator updates authoritative status and
corpus version records on integration.

## Findings

The original hole crossed effect rows, pure views, privacy labels and
component capabilities. A by-spelling fallback also attributed a computed
receiver's member to an unrelated declaration; the receiver's type now
owns that read. Broader-suite results and repository-wide check comparison
will be recorded before completion.

The accepted painter A-021 used ctx.rect with no declaration of either its
context or operation anywhere. The platform now declares browser.PaintContext
and rect with four Float dimensions and paint.custom; the draw binder reads
that declared type through the same language-type lookup as self. The
integrator approved this declaration-only repair on 2026-10-15. No broader
canvas API or runtime implementation is claimed.

R-028's subscribe expression has no type in the existing value typer; its
progress.stage_label consequently introduced an unrelated PW0408 beside its
scope error. The fixture now states progress: OrderProgress, retaining its
application-scope defect. No subscription inference or runtime guarantee is
claimed by that annotation. Its historical C0 text remains scope-rejected.

A further failing reproduction put an untyped member in an interface's
optimistic term (no implementation body). The refusal now walks its term
arena too; the matching typed lambda remains valid. The old body-only row
entry returned early and silently accepted that case.

The whole callback control script initially left three infer-only mutants
alive: the shared solver correctly recovered their removed approximation.
The obsolete list-only callback approximation is deleted; the four controls
now disable the actual solver's parameter mapping, temporary/persistent
binding and pipe argument instead. No test is weakened to manufacture a
kill, and a generic-field control likewise targets the canonical shared
member substitution rather than a redundant caller-side branch.

A further adversarial probe at d00d2e12 still accepted stamp.apply in a pure
view, where stamp reads the database and apply invokes its function-valued
receiver. The value walk discarded every field base as a path prefix. It
now discards only canonical module/type paths; a real function receiver is
charged where named under ADR-0078. A module path whose head shares a
function's spelling stays a path. This addition was ruled by the integrator
on 2026-10-15; its baseline miss is recorded separately. R-094 and a
module-qualified generality witness retain the case.

The function-receiver audit distinguishes analyses: component capabilities
use the same performed-effects walk and did lose the database requirement;
the query contract regression now requires it. Labels do not share the
field-base exclusion: they already join the receiver's label. A separate
privacy regression retains a secret-derived String through the function
receiver, where neither the function's declared result nor the applying
member's signature declares a secret. No privacy propagation is inferred
from the effect refusal.

The recursive invocation audit at 2d2c5f05 exposed a second distinction:
callee and caller may share the same DefId, yet the invocation substitutes
its own arguments. A recursive first<T> called with Item, or a nested
function calling its enclosing first<T>, could retain the enclosing rigid
T and silently lose Item.label's effects. Any result mentioning the
callee's own parameter now goes through the shared per-call solver,
including beneath List. A recursive call given the caller's actual T stays
rigid and its undeclared member remains PW0610. This was ruled by the
integrator on 2026-10-15; before-correction misses are recorded separately.

The per-invocation guard made the former fallback-filter mutation
redundant: removing only Types::of's unsolved filter no longer admits a
callee's generic result, because declared() already defers it to the
solver. The whole script exposed this survivor. Its control is replaced
by removal of that authoritative per-call guard, which builds and fails
both new invocation regressions. The 17 controls each target a distinct
operative behavior; no assertion or valid control was weakened.

The whole component-contract script likewise exposed two obsolete
controls: performed() replaced by infer_at() no longer removes value
effects, because infer_at now completes the shared root walk. Both
consumer-specific controls remain, now substituting calls_from directly
at effective_effects and effective_effects_excluding. Each removes exactly
the intended member/value behavior at that consumer; all 12 controls and
all performed-function-value assertions remain. The whole script reruns.
No production source changes accompany this harness correction.
