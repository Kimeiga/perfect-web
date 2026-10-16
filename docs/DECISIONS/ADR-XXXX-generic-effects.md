# ADR-XXXX: a member read retains its type and effects through generic calls

Status: accepted under the integrator's generic-effects brief of 2026-10-16;
ruling 2026-10-15 (client date); implementation and evidence complete. Date: 2026-10-16. Milestone: E14.

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

The compiler/platform implementation is frozen at 7fb47665. Fixture and
existing-test setup repairs are a2848bcf; the integrator's shared hermetic
server-test patch is 1e8b1ccd. The final corpus taxonomy registry was tested
before committing, SHA256
3fdaf87e5f177ca11ed162ab78f7dcdacacf49add8e53c1103f29153b97d2eca.
No compiler, platform or fixture program changed after the final source proof.

`just e14-generic-effects` at 1e8b1ccd passes 24 generic-effects tests,
18 corpus-checking tests and six generality tests, with 17/17 mutation
controls that build and fail assertions. The branch's generality score is
57/57 invariants, separate from fixture and category counts.
[Own recipe output](../evidence/E14/generic-effects.txt).

Twenty whole affected regression scripts completed at the frozen compiler
source: 178/178 controls build and fail assertions. After the form helper
repair, its complete 12-control suite also passes: 21 whole scripts,
190/190 built assertion kills altogether. No control is credited for a
build failure or stopped process. The computed-row suite has 66 browser
and 24 Cargo baseline assertions, and 12/12 controls; each of its two
browser controls fails three actual assertions. Its reproducible browser
prerequisite is `cargo build --quiet --locked -p pw-dev-server -p kiokun-server`.
[Complete regression output](../evidence/E14/generic-effects-regressions.txt).

The shared Kiokun test-factory patch separately passes both whole controls:
a missing private deployment path still makes the production loader refuse,
while the test server serves the committed sample. Both controls build and
fail an assertion; no process is stopped. The final full CI inherits both
KIOKUN_DATA and KIOKUN_APP, whose values are never logged.
[Dataset-isolation output](../evidence/E14/kiokun-test-data.txt).

The complete command-line preflight passes `just test-compile`, formatting,
2,913 mutation anchors in 264 scripts, all 237 source-mutant admissibility
checks (11 intentionally refused under their named rule, zero failures),
and 62 exact corpus tests. The executable taxonomy covers 37 accepted
categories in 37 files and 80 rejected categories in 80 files. The
corpus-check crate has zero unit tests; that is not assertion evidence.
[CLI preflight](../evidence/E14/generic-effects-cli.txt).

The final default-profile `just ci` passes the whole workspace, command-line
corpus compilation and source-mutant admissibility. Its full log was scanned
for failures before recording success. The source registry hash above ties
the uncommitted candidate to its subsequent commit.
[Full CI](../evidence/E14/generic-effects-ci.txt).

The final program comparison covers 562 .pw files in 441 program sets using
immutable executable copies: 99 clean after, 31 diagnostic sets changed and
42 added diagnostics. Zero pre-existing clean programs are newly refused;
all 12 new refusals are branch-added R-090..R-095 and six generality
witnesses. Nineteen already-invalid sets gain diagnostics. Every changed
source, code and message is recorded; historical source text is retained.
The current executable SHA256 is
89658aa864861c956ee22bcbbba1d521d94095b2b5f582257e3b1dbf06be2a84;
the 22a0508e baseline SHA256 is
d52d080db1b63c578e4cdb666e7af0449fb3223607dbc2730eb86e43681908ef.
[All-program comparison](../evidence/E14/generic-effects-programs.txt).

The standard-package test enumerates generic functions independently from
HIR and source text, then generates one call and member-read probe for
each. It checks solved result types and expected diagnostics, including
functions whose results do not contain elements and constructors whose
arguments cannot solve a type parameter. It independently enumerates 39
generic declarations: 28 element/member-reading probes, nine non-carriers
and two constructors whose type parameters remain unsolved.

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
owns that read. Whole-suite and repository-wide comparison results are
recorded above.

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
all performed-function-value assertions remain. The whole script finishes
12/12 built assertion kills.
No production source changes accompany this harness correction.

The whole nested-scope script exposed the same duplication at an outer
binding: removing only its declared map is recovered by the shared solved
parent environment. The inference-consumer control now removes enclosing
Types construction, so both its declared bindings and the shared solver's
inherited types are absent. The values-checker's separate outer-table
control remains independent. All 12 nested-scope controls and all existing
privacy/type assertions remain. The whole script finishes 12/12 built
assertion kills; production source is unchanged.

## Corpus taxonomy ruling, 2026-10-16

The first complete command-line corpus gate exposed seven new canonical
fixtures whose truthful category headers were not registered in
`corpus-check`. Relabeling ordinary generic/member cases as browser, null
or layout cases would misstate their subject. The integrator ruled that
all seven existing headers be registered in the accepted/rejected category
arrays, under this ADR, before changing any fixture program or diagnostic
expectation. The tool already records the architect's 2026-08-06 ruling
that canonical coverage must grow when a new invariant is exposed.

This extends the executable corpus taxonomy; PROJECT_CHARTER.md is not
changed. The integrator owns the authoritative version/history update:
generic-effects will be C28 on the assembled train, following D1's C27.
File counts, category counts and generality invariant counts remain
separate measures. The seven fixture programs, @rule codes and diagnostic
expectations remain unchanged. Complete `just test-compile`, source-mutant
admissibility and the relevant corpus/anchor checks precede a new full CI.

## Failed runs retained separately

The first full CI exited 101 on 2026-10-16 at 05:54:34 UTC: four core
fixture/setup/pin failures and 16 server failures inheriting external
Kiokun deployment data. It is not passing evidence. Its raw response dumps
remain outside the repository's committed evidence, in
`work/generic-effects-ci-before-fixture-repairs.txt`, SHA256
afdd9434eb3c4653e1e4387458b34c1027fde7813abe6ab719fe85d8921809c6.

The second full CI exited 1 at 06:30:37 UTC after the complete workspace
passed, including 566 server tests (nine intentionally ignored); the later
corpus gate refused seven unregistered category headers. The raw log is
`work/generic-effects-ci-1e8b1ccd-failed.txt`, SHA256
cb080bf615d1b26d2be129e0d2077f9f116adffb9da52a09295e7bf3ee4e0696.
The taxonomy ruling above corrects that gate without changing fixture
programs, rule codes or comparisons. The final canonical CI artifact is
only the actual green run.

Earlier absorbed mutation runs remain historical: a per-call guard made a
fallback filter redundant, and shared inference absorbed old component
and nested-scope mutations. Their surviving controls were reanchored at
the current authority and each complete affected script reran. No failed
intermediate run is relabeled as final proof. An initial computed-row
0/0 browser baseline lacked kiokun-server: an infrastructure refusal,
not an assertion kill. After building both prerequisite servers, the
whole suite completed 12/12. All paused coordinators were terminated and
reaped at restored boundaries; no mutator was interrupted.
