# ADR-0351: a member read retains its type and effects through generic calls

Status: accepted under the integrator's generic-effects brief of 2026-10-16;
ruling 2026-10-15 (client date); worker implementation/evidence complete at7ec6a534; the assembled merge's 49
whole mutation scripts and full CI green on 2026-10-17 (the train-11 section below), its merge of master pending.
Date: 2026-10-16. Milestone: E14.

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
[Dataset-isolation output](../evidence/E14/generic-effects-kiokun-test-data.txt).

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

## Combined train integration, 2026-10-16

The integrator retains the expression-based each headers from ADR-0345,
the bounded callback Handed records and enclosing_callbacks walk, and D1
generated read pairs. Rooted member walks use callbacks(body, root).
The shared values solver remains the only generic call authority; the
canonical list expression types each row and preserves its members.
New joint regressions cover effect/privacy in generic each keys/bodies
and generated per-row/batch reads inside generic-result callbacks.
C28 extends C27 to43 accepted/93 rejected and63 challenged invariants;
these figures remain candidate expectations until the combined gates run.
Full assembled validation is pending; worker receipts retain their inputs.

The initial combined compile found the each-header fallback still calling
the removed mentions_parameter helper. That old unsolved-type filter is
removed: Types::of already defers callee-owned parameters to the shared
per-invocation solver and keeps enclosing rigid parameters intact. No
duplicate generic type authority is restored. The failed compile is retained
at /tmp/codex-generic-integration-focused.log before the next attempt.

## Canonical invocation and interface lexical scope review

Independent read-only review identified two concrete probes before this
merge can be committed: a generated read through an extension member may
perform an effect without entering the bounded checker's resolved call graph;
and a declaration-only interface term's lambda may borrow a same-spelled
global's effects because its lexical arena was not cached. Actual regressions
are added before changing production. Keep semantic member resolution as the
authority, distinguish receiver-only callable properties from structural fields
and function values, anchor an explicit call once, and arrange receiver
arguments using the existing signature authority. Cache lexical information
for every typed arena while inferring implementation effects only for bodies.
No successful reproduction or repair is claimed before the commands run.

Both probes actually fail assertions before repair (native Cargo101 each).
The ordinary helper and explicit row.rows() already produce PW0405, while
row.rows silently passes. The typed interface lambda is falsely PW0330 over
its complete optimistic expression. The full native output is retained at
`docs/evidence/E14/train10-generic-authority-before.txt`, SHA256
fbeb6c615b15e7b0e0c186684b4fb7015f37be03d3b4ce8f2fb716f88a10af00.

The repair adds a shared callable-property receiver classification, records
member invocation identities once through canonical member_read, supplies
the receiver to bounded argument propagation, and caches interface lexical
arenas. The member's declared and inferred effect union remains in Called.
Structural fields and function-valued members with additional arguments are
not invocation entries. A result is not claimed until real regressions run.

After caching interface effect lexicals, the original local-call witness is
clean, but renaming the unrelated global reveals PW0021 at the lambda's
stamp(). Lexical::build_in still returns None for a declaration with terms
and no implementation body. The integrator rules before repair: select the
canonical terms_body arena there, so name resolution, value typing and effect
inference share that arena; do not add a resolver-specific exception or
special name. Preserve ordinary enclosing scope and term-root boundaries.
Actual intermediate raw SHA256 cfd73456e679b9eadabeb9769badcb279bcfa5dfae602855f0caf88183272d48
is retained in train10-generic-interface-scope-before.txt.

## Shared Cargo result validation

Actual status injection after the D1 checkpoint found three positive unexpected
exits (137,67,1) could earn bounded kills, and five invalid exits could earn
generic kills. The earlier D1 eight-regression gate covered negative signals,
not every positive wrapper exit. Its47/12 actual campaigns remain historical
at that instrument; they do not certify this newly exposed boundary.
The raw probe is retained in train10-cargo-results-before.txt, SHA256
6e33e32763285b9be7932f05190ba1b6c6b1200f4ace51b04c421114b97d9a51. Before correction, the integrator rules:
share one pure Cargo/libtest parser for these consumers, require actual0/101
and consistent nonempty summaries from every binary, reject build/stop errors,
and retain failure names as diagnostics. Invalid outcomes earn no kills.
Guide will adopt the same pure authority after its frozen dependency review.
Execution, expected commands, restoration and provenance stay with consumers.
Run the existing8, strict status regressions and all affected wholes anew.

The full generic suite then exposed legitimate additional PW0405 diagnostics
at six old exact-single-code expectations and R-091: those witnesses perform
database reads once per row, now visible through properties. Before fixture
changes, the integrator rules: preserve every effect assertion and its pure
neighbour, state the additional bounded refusal explicitly, and keep R-091
single-defect by taking one reversed-list element via Option matching. Do not
add a MULTI_DEFECT exemption. The independent standard-package probe generator
will record each member read it intentionally generates inside a loop and
hold PW0405 to that exact span as well as the original effect messages.
Its native full negative output is retained in
train10-generic-call-composition-before.txt, SHA256
d4347a6d383098ef141d649244aba1238386e77c862c706a6b78480588b7fda1.

The direct callgraph positive also exposed the value target resolver treating
an actual local receiver List as the module List. Both typing and invocation
resolution must honor the existing lexical head before consulting module
paths. This is corrected in the same canonical Typer target/path decisions,
with direct callee identity/count tests; no special spelling is introduced.
Property diagnostics must retain the written member name, not an empty callee.

The actual fast semantic suite is now28 generic tests plus8 bounded-table
tests, all green. The generator holds each loop-read span independently;
the R-091 fixture retains PW0400 through one selected generic result.
Nine new operative controls bring the generic whole from17 to26, covering
property invocation, exact member identity, one evaluation, structural-field
exclusion, receiver arguments, interface scopes and both local receiver forms.
These are additions to the worker's immutable history, not a claim that its
earlier17-control result proves the assembled implementation. Full combined
focused/whole/CI results remain to be recorded after natural completion.

## Receiver identity also owns result types and qualified cases

Actual further probes find two remaining mismatches before this source is
committed. A local Shape receiver's Empty property borrows a sum constructor
and is falsely PW0606. More seriously, a local List.length() returns Detail
through the value solver, but Types::callee borrows standard List.length's
nongeneric Int result; its following database-backed caption is accepted by a
pure function. The complete actual31-test run is retained in
train10-generic-callee-before.txt, SHA256
c9b2a9a66d99a01b5b489e0605b96be8f0b074c15fcdff4116e5fb4885b5b493. Computed receiver suffix calls and
make(i).decode.list() already pass; those are positive regressions, not claimed
before-repair defects.

Before repair, the integrator rules: a Field callee uses canonical member_read
first, a genuine Path uses by_path, a refused/unknown receiver has no signature;
a bare lexical function callee defers its result to the shared value solver.
The two Typer qualified-case consumers share one local-head-aware case_path,
preserving the existing case_named type/constructor authority for genuine
qualified constructors. Keep direct constructor positives and wrong-result
negatives. Backend consistency and full check/build integration are explicit
remaining assembled proofs; no new backend support is claimed here.


## Executed backend identity and handle-construction review

The actual checked-and-built Wasm witness serves HTTP200 with Local=false,
while its genuine Shape.Empty constructor control serves true. The local
Shape:Item getter returns Shape.Circle, but the backend emits the namesake
enum constructor instead. Two additional actual backend IR probes compile
local Shape.Circle(7) and List.map([7], Shape.Circle) as the unrelated enum
constructor. Both fail their identity assertions. Separately, calling a
local Session:fn(String)->String is falsely PW5037, because the handle scan
treats the callback as the platform's opaque constructor. Fixture syntax and
query-binding mistakes in earlier backend drafts are not semantic evidence.
The complete actual negative outputs are preserved below.
- `train10-generic-backend-before.txt` SHA256 `3bfc7483899fbee3847564ead036132da2f005afacba88fdade89c06ebfeecae`.
- `train10-generic-backend-call-before.txt` SHA256 `27f511e97f4dfd2f46be36e9b5e38db25a60de4c11a53c64a6a6e2cbdc08b31d`.
- `train10-generic-handle-before.txt` SHA256 `0639a0e08f4d2c52898975274175e2d6cb72622432dda9da7498ce75812e63c5`.

Before production repair, the integrator rules: expression namespace lookup
requires a genuine Name-rooted path whose head has no canonical lexical
binder. Share that decision through Lexical and contextual values named_at /
case_named_at readers, including the typer, privacy scan and backend. Cache
the canonical per-body Lexical in the lowerer, respecting the currently
inlined unit and body owner; never use a second spelling-only locals rule.
Pattern case lookup keeps its separate type/constructor namespace. Existing
getter lowering should execute the actual receiver. Local member call and
callback lowering that the backend does not implement must explicitly refuse
before any global/type fallback; no new callable support is claimed. Genuine
constructors, actual forged-handle refusals and existing local function calls
remain positive controls. Fresh controls, focused suites and full assembled
gates are required before this source can be committed.


## Assembled candidate before fresh whole campaigns

The actual focused backend/semantic gate now passes31 generic,8 bounded,
12 backend,4 handle and1 real HTTP/Wasm tests. Both constructor and module
collisions have refusal controls; genuine constructors/modules and actual
forged-handle refusals remain valid controls. The full combined focused
gate passes checking20, each9, generality6, platform4, view-positions2,
CLI guide15 and14 strict Cargo-result Python regressions, with3292 anchors
in274 scripts. Raw outputs are train10-generic-backend-focused.txt and
train10-generic-focused.txt. These are focused results, not full CI.

The own whole now has33 operative controls: original17 plus16 assembled
controls. Three existing controls moved to the actual shared lexical
authority after consolidation; four further controls cover the privacy
consumer, executed backend property and backend call/callback module
fallbacks. The runner now includes backend, handles and the executed server
witness beside generic and bounded tests. No whole33 success is claimed
before natural completion and byte-restoration checks.

The fresh regression closure includes the original21 worker scripts,
current changed-anchor/test consumers selected by ci_plan, canonical lexical
and each consumers, direct performed-effect consumers, and backend
case/function/opaque/privacy consumers. Every selected control must have an
actual unequal source substitution before launch. Complete script output
and independent Cargo test command outputs/statuses will be retained;
invalid builds/stops earn no successful campaign result. Existing browser
consumers retain their own spec assertions and raw producer qualifications.
Final whole and fullCI receipts will state the exact frozen input hashes.


## Fresh each-control campaign exposes an obsolete fallback control

The frozen assembled v1 own whole33 completes with170 independently retained
Cargo commands, all valid actual0/101,56 baseline passes,33 built-test kills,
no stopped process and all2392 source/configuration byte hashes restored.
Independent replay of every raw command confirms those outcomes. The next
whole each campaign naturally fails19/20: disabling only infer's computed-list
fallback survives because the canonical value solver has already typed its
Each binding. Its actual command exits0 with19 passes, not a hidden failure.
All2392 bytes restore again; this is not a20/20 result.
Its complete raw output is retained in history, not in committed evidence,
which records no survivor (`scripts/evidence_survivors.py`, a gate of
`just ci`): `git show 600ebf19:docs/evidence/E14/train10-each-computed-control-before.txt`,
SHA256 0ef88dce83ddc2e47aa246689d559ac2674dfc98fb387b7b25ce5193a7fd2453.

Before control correction, the integrator rules: keep every existing semantic
assertion and valid neighbour. Retarget that one control to the shared
Lexical each-block enumeration used by both value solving and inferred-type
projection, withholding lists whose head is not a local path binder. That
removes the actual common input for computed/declaration list row typing;
it does not manufacture a kill by weakening tests or changing production.
Do not delete the general inference fallback based on one equivalent control.

The each runner's native injected-result regression separately proves it
credits -9/137/1 with a failed summary, and101 without any built tests.
These are injected classifier outcomes, not stopped real Cargo processes.
Raw train10-each-producer-before.txt retains all four assertion failures.
Before the harness change, the integrator rules: adopt the already shared
pure Cargo parser for this third consumer, require native0/101 consistent
results, and refuse invalid producers without kill credit. The two native
consumer regressions preserve source restoration and a named101 test-failure
positive. Existing shared parser bytes/API remain unchanged. Fresh whole20
must pass; assembled fullCI remains pending.


### Cart field-row control and aggregate Cargo results

The frozen v2 closure ended naturally on cart-lines at40/41, with the sole
survivor `a loop over a field is typed as its binding`. Raw /tmp/codex-generic-assembled-v2/cart_lines_mutations.txt, SHA256
d11c5ab15686a95c6270df5e304f17bb2244a58c8094144d73e8ea4016d1a885; all2393 source/config hashes restored and
no process stopped. Its old infer::element_at field loop is a fallback:
canonical value typing already solved these rows. This is an equivalent
control on this assembly, not a demonstrated production escape.

Before this amendment the independent cleanup-tree candidate changed the
shared lexical each-list identity from a Field to its base. Existing exact
four-target argv baseline35/0; the candidate built and failed three named
handler assertions at actual101 after6+18 passes. Cargo stopped before the
later two targets. RawSHA0046afac595a215ca7677d963631711ccb485dd7f8f15cb6e6097c04026ae1e8;
all2822 candidate input bytes restored. Ruling: re-anchor only this control
to that common identity, preserve production/tests/fallbacks and rerun whole41.

Separately, actual `cargo test --quiet --locked -p pw-render` returned0 with
19 consistent summaries,117 passed,0 failed,1 ignored and two empty sibling
binaries. Canonical acb698a7 incorrectly refused it. Ruling before this source
change: require a nonempty command aggregate and consistent individual
verdicts; successful empty siblings may coexist with real tests. All-empty,
unknown/mismatched exits, build/process-stop errors and contradictions refuse.
New canonical pure API SHAa177209cff90064cd45e734bb913c8557cb7a03e0ab7920305e6f9aa196df317
passes8 methods and replays340 previously valid command outputs identically.
Native counterexample rawSHA0a6ac5f5b1ba114f72a473d2466504371d9746793f3bf94e744fb0eeb8f43402.

The old external observer saw subprocess.run only. Direct Popen consumers
(including bare-cases and cart-lines) retain existing-runner evidence only;
their previous printed verdicts are not per-command native Cargo audits.
Future observer captures communicate for both paths without changing argv,
bounds, results or cleanup. A private actual Cargo fixture proved both real
named assertion101 outcomes and refusal of actual compiler101 failures through
both paths, restoring source; no cancellation experiment was performed.
These amendments require a new frozen run and fresh applicable suites/CI;
the previous incomplete closure is not a final green gate.


## C8 R-035 historical receiver classification (integrator ruling, 2026-10-16)

The host-composition default CI exposed a real RED in corpus_history: C8's original R-035 did not report wrong_frame_phase. Its ElementRef annotation has no browser import. The canonical receiver solver refuses this unresolved type and cannot invent style effects by a global member spelling. This is FixtureDidNotExpressIt under the existing Why taxonomy, not a phase-semantics regression and not acceptance of the old program.

An external immutable-input public check_sources probe, linked to already-built exact-hash pw_core/serde_json rlibs without Cargo or project changes, pins the causal distinction. Original: PW0026 unresolved_type528..551; PW0408 member_receiver_untyped736..744,736..756,779..787; PW0605 argument_type857..881. Adding ONLY import browser.{ ElementRef } restores PW0402 wrong_frame_phase766..792 at exactly it.style.set_padding(4.px), beside the old width/maximum defects PW0610/PW0605. The current typed fixture reports ONLY that PW0402; deleting ONLY the illegal setter makes its typed neighbour clean. Every other library unit is clean. The existing causal_evidence tests forbid ProgramWideName routing and effects invented for a receiver with no such member.

The TEST-ONLY correction explicitly classifies this one C8 original and requires its five exact original code/symbol/span/source-text/message refusals, import-only causal control, current typed violation and clean typed neighbour. All25 historical files and the25-file floor remain unchanged; the other24 retain the original rejection assertions, including the existing R-019 parser boundary. Actual counters report23 declared-invariant catches +1 existing R-019 syntax refusal +1 classified outof25, never24 or25 original target-invariant catches. The existing R-019 assertion is unchanged. A mismatched required rejection fails before any success measurement is printed. Production scope, ambient resolution and expected-code allowlists are unchanged.

Actual immutable receipt: /tmp/browser-c8-readonly-2mr9np78/receipt.json SHA256b4a04bc47e8a1ba04c435ab7ce9e0b52f8624b0af004b8e9a19354f0403e2c44; probe46ad30c3811043e0604333aea2c3f3f2f7b17aeb201892adb05d2aed5fb1cc4f, binary29e294ae82bad61d3227ed07989a22ff78b9bef775af6761a70d62db4bf8964d; pw_corec19ff415c3663da2794dabcc84ee93b9839233670702ec2b970a7f5d4056eb4d, serde_jsonf37867b2cfe65dd2cd73fe1cb915593ba7dfb685e22e2fb3d8bab8667e78a613. Complete structured native diagnostics, invocation argv/exits,49 exact library entries,57app inputs and source hashes are retained. Two initial external fixture/toolchain setup refusals are preserved separately and earn no semantic control credit.

This amendment precedes canonical test correction. Proposed patch is NOT applied in the active host CI tree; integrator review and quiescent canonical validation precede adoption.


Integrator root-assembly ruling before this amendment: v3's first13 complete
wholes naturally passed, then computed_holes stopped its unmutated baseline
49passed/1failed at this same historical R035 assertion. No computed-hole
mutant ran and the remaining35 scripts never started. RawSHA256
6eb380df13d1c0c89a0281f06daf1517cc11f39b5e81bb8d0e1718c9d2132111.
All2393 frozen source inputs were restored. Generated evidence and store-ir
were explicitly outside that original manifest; no all-repository-byte claim.
Keep v3's partial evidence source-qualified and do not call the49-script gate
complete. The canonical C8 v2 correction already passed all8 history tests
in the isolated CIB composition, with23 actual invariant catches,1 existing
syntax refusal and1 explicit missing-import classification among25 originals.

The last-segment audit's existing effects.rs:calls_from display entry moved
with its code into calls_with_types. The short name is computed only after
fullpath/DefId/effects resolution and feeds Via::Helper.helper for diagnostic
describe(); callee() preserves the full path. Reanchor only that existing
audit key, retaining its category/prose and forbiddenzero. CIB's unchanged
audit3/4 RED and corrected4/4 GREEN are retained separately. No compiler,
library, historical fixture bytes or mutation tuple changes are authorized.
Freeze the amended snapshot after baseline gates, then rerun all49 complete
mutation scripts and fresh CI before the generic merge may be committed.


## Train-11 completion: the lexical survivor, the unreached scripts and CI (2026-10-17)

The merge commit 600ebf19 (train-10's 9902c62b with 7ec6a534) holds
Codex's resolution and the repairs below as Codex left them. Three things
kept it from green: a survivor in the assembled `lexical_mutations.py`,
18 of the 49 mutation scripts the merge reaches that its driver never
ran, and no `just ci`. The driver ran 31 whole at a frozen source of 2393
files (evidence and store-ir excluded) whose every byte equals
600ebf19's, checked file by file: 30 green, and lexical 18/19.

**The merge's repairs**, each where a local binding meets a module's or a
type's path of its spelling, each held by a test and by controls of
`generic_effects_mutations.py` (33/33 whole in that assembled run):
- an effect read through a local `List.length()` was missed: the callee
  borrowed the standard `List.length`'s `Int` where the value relations
  give the local function's `Detail`
  (`member_and_local_function_callees_keep_their_actual_result_type`);
- `Shape.Empty` through a local `Shape` took the global sum constructor:
  a false PW0606 (`a_local_receiver_never_borrows_a_type_qualified_constructor`),
  and the backend emitted the namesake enum constructor, served over HTTP
  from Wasm (`a_local_receiver_getter_runs_instead_of_a_namesake_type_constructor`,
  with the lowering refusals in `backend_lowering.rs`);
- a false PW5037: a called local `Session: fn(String) -> String` was read
  as the platform's opaque handle constructor
  (`a_local_callback_is_not_a_handle_constructor`);
- `row.rows`, a receiver-only property making a generated read per row,
  escaped the PW0405 that `rows(row)` and `row.rows()` get
  (`a_generated_read_through_a_member_stays_a_read_per_row`);
- a false PW0330 over an interface's typed optimistic lambda, whose term
  arena had no lexical scope
  (`an_interface_term_local_call_never_calls_a_namesake_global`).

**The lexical survivor is killed, not removed.** It was `the declared-type
environment reads no local's binding`: in `Types::declared`, a name's
binding read replaced by `None`. The mutant is not equivalent. Since this
ADR, `Types::of` answers what the value relations' typer solves wherever
`declared` gives nothing, and that typer reads every binding source
`Types` reads (parameters, annotations, a query's `let`, lambda and
handler-event parameters, the bindings around a nested declaration,
loops, arms, stream parts and release clauses). For a name whose type is
known whole it answers what the binding says, so the one test that killed
the mutant in its last recorded run before this ADR (1 of 31,
`docs/evidence/E10/lexical.txt`) passes under it, through the typer. What
only the binding answers is a type naming the body's own type parameter:
`ResolvedType::inferred`, which makes the typer's answers resolved types,
makes none of a `Parameter`. Under the mutant every reader of such a name
lost its type, and a rule went silent: `fn f<T>(r: Result<T, String>,
n: Int) -> Int { r; n }` drops a `Result`, PW0618 with the binding and
nothing without it. `a_names_type_is_its_bindings_where_nothing_else_types_it`
(`lexical_scope.rs`, d3daa5bb; its comment corrected at f773d429) states
it, with its `let _ = r` control. `lexical_mutations.py` whole at
f773d429: 19/19, the survivor killed by 1 of 32 tests.
`each_heads_mutations.py`, whose tests include `lexical_scope`, whole at
f773d429: 12/12. No mutant was removed or changed.

**The 18 unreached scripts**, whole at 600ebf19 in three private clones
run side by side (the one source file changed since, `lexical_scope.rs`,
is a test none of them builds): member 10/10, member_resolution 9/9,
name_binding 14/14, nested_scope 12/12, opaque_value 2/2, page_policy
44/44, policy_value 14/14, privacy_resolution 3/3, query_read 3/3,
record_field 5/5, recursion 6/6, scheduled 37/37 and store_postgres 12/12
(on PostgreSQL), shared_cache 5/5, tables 44/44, value_labels 12/12,
what_a_component_does 12/12 and write_invalidation 13/13: 257/257, each
kill a built test failing, no build failure counted and no process
stopped. Each script restored its sources (`git status` empty after it).
The first `page_policy` run's browser baseline was red because its
suite's web servers need `kiokun-server`, which a fresh clone had not
built (the script builds only `pw-dev-server`): an infrastructure refusal
in which no mutant ran. After `cargo build --locked -p pw-dev-server -p
kiokun-server` the whole script ran again. All 49 scripts: whole and
green, 665/665 mutants killed.
[Every run](../evidence/E14/generic-train11-wholes.txt): this session's
logs, and the driver's outputs, each checked against the SHA-256 it
recorded; the one that records the survivor is named by its hash, not
copied.

**A gate the merge failed.** The merge committed a raw whole run that
records a survivor (`train10-each-computed-control-before.txt`, above),
and `just evidence-gates` refuses committed evidence that records one
(`scripts/evidence_survivors.py`). It stays in history at 600ebf19, named
above with its SHA-256; its control was re-anchored, and
`each_head_effects_mutations.py` ran 20/20 in the assembled run.

**A flake, not loosened.** The first `just ci` at f773d429 (2026-10-17,
15:02 to 16:06 UTC, load average 40 to 75 from other workers' runs and
these) failed one test of 3,100: `check_latency`'s cold check of 57 files
used 2.77 s of its thread's CPU time against the 2 s ceiling (3.76 s on
the wall clock). Its comment says why that is the machine's: a thread on
a busy core takes more CPU time for the same work, and the wall clock it
bounded before failed at loads of 40 to 110. It passed in the whole
`pw-core` suite run at 600ebf19 at a lower load (233 binaries, 1,727
passed, 0 failed). `just test-compile` and `just
mutant-programs`, which that run did not reach, pass on their own: 302
source mutants judged, 16 refused by their named rule, 0 failing.

**Full CI is green.** At 7ab0e7a3, after the gates unpiped in one chain
(`cargo fmt --all` clean, 3,292 mutation anchors in 274 scripts each
matching once, `case-check` over 2,870 paths, `just evidence-gates`' 246
tests), `just ci` (2026-10-17, 16:31 to 17:04 UTC, the load falling from
54 to 10): `ci: OK`, 347 result groups, 3,100 passed, 0 failed, 25
ignored, `check_latency` among the passed; 302 source mutants judged, 16
refused by their named rule, 0 failing. KIOKUN_DATA and KIOKUN_APP were
inherited, their values not logged.
[Full CI](../evidence/E14/generic-train11-ci.txt).

**Not claimed.** Which other test suites fail under the lexical mutant:
that census was started and stopped before any test ran, at a load
average of 63. The integrator's status documents, which the merge commit
edits, still say the combined gates are pending.

**Next.** Merge `origin/master` when train-10 lands on it (never rebase),
run the gates and `just ci` again, and push `track/generic-train11` on
green.

## Train-13 survivor repair (2026-10-20)

Master 115895de is merged without rebasing, preserving its check/build and
producer-validity work. Six stale controls remained on the generic track:
two query-value controls and four discard-inference controls. The shared
value solver recovers concrete types when the older declaration-only
approximation is removed; deleting that approximation alone does not prove
that a consumer loses its type.

The four discard controls retain their existing production anchors. A new
semantic regression carries the enclosing function's rigid T through `?`,
`if`, `match` and a block into a named `Result<T, String>`, then drops it.
Each must produce PW0618 at that name, while explicit discard is accepted.
The solver's concrete resolved-type projection cannot replace that declared
parameter, so each control now fails this assertion.

The loop/arm iteration control also keeps its production anchor. A generic
function's template matches an `Option<List<Row<T>>>` and then loops over it.
The phantom T does not affect its concrete record fields' capture schema,
but its binding must retain T. A resumable capture verifies that declaration
propagation reached the loop, and a missing Err arm still produces PW0305.
This is a frontend witness, not a claim of generic UI/backend specialization.

The deleted query-binding approximation is redundant for these concrete
query declarations. Its control instead erases the canonical value solver's
query success type. A new function-argument witness accepts Option<Status>
and refuses passing that query value to Option<Int> with exact PW0605.
The existing incorrect-Result control and all original query tests remain.
No production implementation or language semantics changed in this repair.

Both campaign consumers now delegate Cargo exit/result validation to the
existing `cargo_test_results` authority. Build failures, missing/empty output,
contradictory results and unexpected exits refuse the campaign after source
restoration; none earns a kill. Five executable injection/restoration tests
cover both consumers. Four additional controls undo actual-exit validation
or the refusal status, and are judged through the shared frozen-inventory
unittest producer/protocol (`guide_python` launches it); only completed body
assertions count as kills.

Local focused validation: query_values 6/6 and let_discard 12/12; all six
formerly surviving semantic controls build and fail their new assertions;
producer controls 4/4, five tests in their green baseline. Full query/discard
and generic regression campaigns plus exact-commit CI remain pending on
GitHub. Raw local logs are /tmp/generic-finish-focused-final.log,
/tmp/generic-finish-six.log and /tmp/generic-finish-producers.log; these local
counts do not replace whole campaign evidence.
