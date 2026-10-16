# ADR-XXXX: check and build validate one executable program

Status: draft before implementation, 2026-10-16. The integrator approved the
ownership design and PW66xx allocation on 2026-10-16; results remain open.
Milestone: E14, developer feedback (check-is-build).

## Finding

At 17fade14, `query Count() -> Int { todo }` read by page Home passes check,
but build refuses Home's dependency on the placeholder. Changing todo to 3
checks and builds. The original binary's recorded proof is
`/tmp/codex-check-build-audit.txt`. The guide also holds open a signal in a
title, a value computed from two signals, and a computed each head.

ADR-0090 put declaration rules into one checker. The reverse implication is
still absent: successful semantic checking does not establish that the
compiler can execute the program it accepted.

## Inventory before change

All semantic exits in build.rs and the stages it calls are in scope:

| Source of refusal | Existing authority | Validation must retain |
| --- | --- | --- |
| syntax and semantic errors | check_units / lower::Checked::of | original codes and source files; short circuit support stages |
| handler lowering/JS encoding | backend::js | every non-Encoded handler, at its declaration |
| component lowering/encoding/audit | backend::component | Built::Refused, including derived functions and world audit failures |
| depended-on todo | Build::refusals, contract imports | dependency and placeholder identity; unrelated placeholders remain allowed |
| pure-query JS module encoding | backend::js_pure via component preparation | top-level Err, even though Build::refusals never sees it |
| WIT package and world preparation | wit::package / component::with_program | generation and parse/world failures before Build exists |
| blocked template or standalone layout | template_ir / build::build | all blocked parts, including an unused layout |
| speculation lowering/encoding | backend::speculation | every non-Encoded speculation |
| computed signal module | backend::computed | every module Err |
| graph dangling edges | graph::Graph | every unresolved edge |
| inert event parts | template manifest | every event with no handler code (a semantic backstop) |
| page value plans | page_values | every plan Err |
| signal initial values | backend::signals | Err folded into its owning page's plan |
| output directory creation/writes | Build::write | build-only IO failures, excluded from semantic validation |

These are categories, not claims that every backstop is reachable after a
successful semantic check. Tests will distinguish reachable witnesses from
backstops already refused by a narrower diagnostic.

## Decision and ownership

One validation authority in pw-core owns the sequencing. It prepares the
program's borrowed HIR list, resolution, signatures and contracts once,
runs the existing semantic checker once, and stops immediately on errors.
A private prerequisite proof binds the backend context to these exact units.
The constructor validates source/HIR identity, not just equal vector lengths;
a proof cannot be constructed externally or rebound to another program.

Backend stage routines accept this proof. Their public entry points keep a
checked-input gate; the shared validator calls the proof-consuming routines,
never those public entry points. In particular, check_units must not call
public build: the old lower::Checked::of called check_units, which would
recurse. No flag, global cache, thread-local bypass or unchecked constructor
is introduced.

The validator runs the existing lowering, encoders, audit, template and page
planning analyses in memory. It retains their exact outcomes in one plan.
Check renders diagnostics and drops the plan without filesystem artifacts.
Build consumes that plan and writes it. No approximate support traversal is
added: an independent list of supported syntax would become a second checker
and diverge as backends grow. Borrowed contexts and proofs remain scoped to
the authority's callback; the final artifact plan owns its data and can leave
the callback safely. A successful final proof/plan is private and only exists
when all semantic and support checks succeeded.

PW66xx belongs to Owner::CheckIsBuild. PW6601 names one invariant: a
semantically valid program is supported by the compiler's executable
backends. Stage/subreason metadata, the responsible file/declaration or part
span, original refusal wording, explanation and repair travel with each
finding. Existing narrower diagnostics remain authoritative and do not gain
PW6601 noise. Additional codes require genuinely different invariant meaning.

Build::refusals remains the inventory/backstop for partial build outcomes;
the implementation will choose its API adaptation explicitly and record it.
No refusal may be lost merely because a stage returned Err before a Build
could be made. Filesystem writes never occur on a refused plan.

## Cost and scope

Encoding and audit during check are a real cost. Record cold/edit checks and
stage wall time, process CPU and host load before/after. Preserve existing
latency ceilings. If the shared plan cannot meet them, report measured costs
and the design tradeoff before any ceiling change. The original untracked
zz_stage_timing.rs is preserved byte for byte and is not a new deliverable.

Supported program behavior and artifacts remain unchanged. Unsupported
programs become check refusals. Held-open guide gaps that close become
explicit refusal examples, with the current diagnostic, rather than losing
their tests. Build-only IO errors remain build-only.

## Acceptance and results (pending)

- Red-before/green-after tests: depended placeholder, title signal, two-signal
  computation, computed each head, and supported neighbors.
- Inventory-driven tests cover every refusal category, with existing narrower
  check codes or an explicit backstop explanation where no clean witness exists.
- Rejected corpus parity, every repository program, artifact compatibility,
  proof safety, semantic short circuit, all backend entry points.
- Whole check_is_build mutation controls, every affected whole mutation script,
  fmt, anchors, case-check, evidence-gates, own evidence recipe and just ci.
- Measured latency/CPU/wall/load and any newly refused repository programs.

No equivalence, latency or completion claim is made by this draft.

## Implementation checkpoint (local WIP, not complete)

- Semantic-only `check_units` and `check_sources` remain available for partial
  and library HIR. `check_program_sources` is CLI check's executable promise;
  `program::build` consumes that same validation. The historical `build::build`
  API remains partial inspection compatibility, and `Build::write` now refuses
  incomplete inspection plans.
- A public `with_semantic_program` callback constructs its own context and
  lends the prerequisite proof. The old public constructor taking an arbitrary
  Context is removed: pointer identity alone cannot prove matching resolution
  and signatures. Private constructors retain exact HIR identity checking.
- The successful final plan is a private `Executable` variant, minted only by
  the validator. Public diagnostic report data cannot grant acceptance. A new
  regression was red before that separation (`/tmp/check-is-build-report-red.log`).
- Effects, page plans/members/derived functions, handler modules, component IR,
  parsed WIT and expected audit worlds are shared, including WIT's formerly
  repeated page planning. No encoder, audit or support check is approximated.
- Early WIT/template failures retain independent stage refusals. WIT generation
  and pure-JS module errors carry typed declaration ownership. Tests cover a
  two-file collision plus independent title refusal and an unmappable export.
- The guide's semantic build gaps are PW6601 refusal examples. Its verdict tests
  retain actual build failure coverage through an unwritable output destination;
  semantic held-open gaps are explicitly tested as closed. Guide15/15 passed.

Initial full-path latency after sharing: cold1788.36ms/edit1729.24ms, clean
1992.95ms/rejected926.76ms (55 files), unchanged2s gate,2/2 tests. The final
source requires a fresh measurement. The integrator's larger56-file train10
program must be measured separately. No ceiling change is authorized or made.


## Accepted language and current backend support

The census exposed four existing build-only gaps in the exact whole accepted
corpus program (85 files). The integrator ruled that the accepted corpus stays
as the semantic language specification; no accepted source is edited or moved,
and this track does not implement unrelated resource or layout features.
`test-compile` now asserts semantic corpus conformance through its existing
Rust suite, and separately pins complete CLI check/build to exactly these
four source/stage/message/span refusals and no artifacts:

| Source | Stage/subreason | Existing executable limitation |
| --- | --- | --- |
| A-007, StoreLocation | backend_template | mounting a resource element is not compiled |
| A-008, Recommendations | backend_wit | duplicate generated API item store-recommendations-recommendations-api |
| A-013, Terms | backend_page | a computed part performs build.input.read<WorkspaceFile> |
| A-018, ResponsiveMenu | backend_template | style:grid-template-columns directive is not compiled |

These are exposed backend gaps/ABI collision, not invalid language or completed
features. The complete gate pins the exact set, not any PW6601 or a broad
allowlist. Executing the real recipe with failing stand-in producers proves
both gates run and propagate failure; mutation controls remove each gate.
All supported reference program roots remain positive complete check/build
controls. The final train must rerun the census after generic and D1 effects.
The pinned local wit-parser 0.257.1's typed ParseError/ResolveError spans map
the refused emitted world/API name through existing declaration identities;
this is location metadata after the same parser refuses, not a second check.

The first census detected two uncovered nested robustness regressions and
failed its coverage assertion (561/563 tracked .pw files). The glob is corrected
to include nested regressions; the completed run covers 563/563 tracked files.


## Verification checkpoint before broad campaigns

- Original whole16:16/16 killed, baseline18, zero bound stops. Expanded
  whole20:20/20 killed, baseline22, zero bound stops; source restored. Its
  Python failed-command counts were conservative (both tests counted failed
  when one failed); reporting is corrected for the final recipe's whole rerun.
- Core complete-program tests10/10; CLI witness tests7/7; rejected corpus78/78
  actual check/build pairs agree (156 commands),46.97s. Exact accepted support
  classification1/1,1.76s;9 supported roots check+build (18 commands),20.40s.
- Guarded fmt and core-library Clippy passed; mutation anchors3064/267,
  case-check2704 paths, Python evidence-gates170 tests1skip. Original probe
  restored byte-identically to the recorded SHA256.
- Committed source387bc12e census:407 program assemblies cover563/563 tracked
  .pw files;11 clean,323 semantic/syntax refusals,73 new complete-program
  refusals. Those73 are not73 mechanisms: semantic-clean generality suites
  include the whole accepted corpus as their existing library, inheriting its
  four backend gaps. There are16 distinct refusal messages. Raw classifications
  are committed in docs/evidence/E14/check-is-build-programs.txt, with source
  and immutable binary identity. A parent train must rerun after new effects.

Whole affected mutation campaigns, full CI, own final recipe and final cost
measurements are still pending; this checkpoint is not a ready claim.


## A missed standalone computed-module path

The first whole computed-signals campaign was RED: 15/16 controls killed,
with browser baseline66/66 and cargo13/13, all three browser mutations killed,
and no bound stops. The omission control still targeted standalone
`computed::compile_checked`, while complete builds now reuse the exact IR
through `compile_prepared`. Existing program tests exercised only the latter;
the missed path was not retired or treated as a passing control.

The original raw report is retained externally at
`/tmp/check-is-build-whole16-computed-signals-red.log`, SHA256
`1b6cf8ccfce5ed8e2931175baa569a3b70056a4fa835e0f250017796fa1aa9a5`.
A new independent program test compares the complete emitted module sequence
and bytes from standalone and prepared compilation of the real feed. It is
green, as are the existing computed-holes/signals tests (8/8+4/4). The old
standalone omission control remains, and a seventeenth independently removes
the actual prepared-plan module output. No backend behavior or assertion was
relaxed. The whole17 rerun and other held browser campaigns remain pending.

The first earlier browser baseline ran no assertions (0/0): its configured
kiokun-server prerequisite was missing, exit127. That infrastructure refusal
is retained separately, and both native hosts were built explicitly before
the fresh green baseline. This configuration starts65 webServer entries
(64 pw-dev-server+1 kiokun-server); inspected parents/arguments matched the
registry. The dispatcher was held and reaped only after normal whole-suite
source restoration; no live mutation/browser child was interrupted.


## Scoped browser evidence repair (design before adoption)

Whole computed_signals17/17 completed after the earlier RED15/16. The old
build_id campaign observed12/12 kills and exit0, but inspection found its
final restored browser tuple was discarded. The same defect exists in refusal.
That refusal campaign was cancelled as a partial run, not a whole result;
its raw report is retained externally at
`/tmp/check-is-build-whole-refusal-cancelled-partial.log`, SHA256
`588164807100d285af6bd36f42bc976c20e0bf8135378ffa62ad6872327c764e`.

After all owned children exited and source/probe restoration was proven, these
two scripts will adopt the exact existing `scripts/run_suite.sh` authority
from immutable integrator commit `c67bf6ed`, together with its exact existing
`test_run_suite.py` regressions. Provenance is recorded before adoption. This
self-contained helper owns producer status and executed-test presence; the
legacy numerical summaries remain reporting only. Both actual browser commands
will invoke it, and each final restored browser tuple must also be green.
Deterministic stand-in producers exercise each real script's main cleanup and
command wiring/status, red before the correction and green afterward. Independent
controls remove each cleanup gate, helper wiring and nonzero-status requirement.

Refusal's four Chromium invocations may be batched only if actual before/after
Playwright --list inventories have the identical selected-test multiset; no
assertion, grep meaning, retries or thresholds may be weakened. Whole corrected
build_id12/refusal30 plus guide74 and expanded own controls remain required.

A read-only AST/callee audit found26 discarded final browser tuples in this
base. Those are static failure-path witnesses, not26 observed failed runs. The
integrator separately repairs descriptions; this scoped correction repairs
build_id/refusal. The other23 remain explicit integration work: block_parts,
canonical,cart_lines,command_answers,command_order,command_retry,connection_faults,
documents,holds,instance_changes,keepalive,materializer_failure,navigate,not_found,
one_entry,page_policy,query_params,slots,soft_navigation,store_accounts,stores,
stream_boot,test_controls. There is no claim that all scripts propagate failure.

Cancellation itself exposed an ownership defect: managed Ctrl-C left Linux
children alive and an early runner selector missed @playwright/test/cli.js;
15 shell/server pairs spawned during its grace interval. Exact stable pidfds
and the original WSL relay/environment/cwd/argv/start identities allowed scoped
cleanup. All122 pinned instances exited/reaped and three listener samples were
empty before editing. Source HEAD30f7e670 and the original untracked timing probe
were byte-identical. This records a process-owner regression target, not a claim
of general cancellation correctness.

The actual pre-batch Playwright1.58 inventories select7 identity+1 recovery+
1 lazy-handler+2 accessibility tests. Replaying the exact list selects the
identical11-test multiset. Before/after raw inventories and proof are retained
externally as `/tmp/check-is-build-refusal-list-before-{1..4}.txt`,
`/tmp/check-is-build-refusal-list-after.txt` and
`/tmp/check-is-build-refusal-inventory-proof.json`. The implementation derives
its replay list from the unchanged original selectors on every run and compares
the replayed --list multiset before one actual assertion invocation. Failed
inventory producers, zero selections and drift refuse the browser phase.
It changes no grep meaning, retry or threshold. This guards future test changes
instead of freezing a current title list.

Initial deterministic cleanup/wiring/status regressions:12 methods,10 red
failures with2 green neighbours before correction;12/12 afterward. Two added
inventory-boundary methods give14/14 green. The exact canonical helper's8/8
regressions pass. Eight independent controls extend this track's whole20 to28;
the full28 rerun and whole build_id12/refusal30/guide74 remain pending. Canonical
helper SHA256 `6aaf7b298aec45b1658455fd0f35256b109896a1dea15dec620002e992bd8161`,
canonical test SHA256 `12939e2b3bce1a18473b6170028b50f9de72059ea9811ce5f66fa9173cdbe683`.
Old observed build_id12 raw retained as
`/tmp/check-is-build-whole-build-id-historical12.log`, SHA256
`aee8b70eca4360bf6a848abedf3ece7e756f922c63976e2638f1b7a283293895`.

## Diagnostic reservation integration ruling, 2026-10-16

The integrator found that integrated ADR0339 had already reserved66/PW6601..6612
for an unimplemented collaborative-docs design. This track's initial parent
allocation therefore conflicted with an existing design reservation;66 was not
historically free. After a read-only116-ref/98-head audit found no PW67 use and
ADR0339 was reread, the integrator formally ruled to retain implemented/tested
Owner::CheckIsBuild66/PW6601 and move only the unimplemented collaborative-docs
design to67/PW6701..6712, preserving suffix meanings. This grants no documents
implementation or package permission. The parent owns the corresponding ADR0339
and authoritative reservation/status amendments before integration. No unrelated
owner is silently reassigned by this worker.


## Producer validity and interrupted own28 (before final whole35)

The first own28 dispatcher-only stop was intended to leave its regular-file
output child running. The child instead exited immediately after10 controls,
leaving the11th mutation in build.rs. WSL managed-session teardown is an
unresolved causal hypothesis, not a proven mechanism. The full partial report
is retained at `/tmp/check-is-build-own28-dispatch-interrupted-partial.log`,
SHA256 `b7cf75485edd34762289e387c86d155236a6328db3cfbf7c9c9b74a720ca7763`.
It earns no whole28 result. All known session entries were gone and three
worktree process samples found no owned descendants before restoration. The
sole residue matched the exact11th replacement; immutable30f7e670 build.rs
bytes matched its saved pre-run manifest hash and only that file was restored.
The entire frozen source/probe manifest and3082/268 final anchors are verified
after the subsequent hardening. Future campaigns run to their natural end.
This interrupted28 and the earlier cancelled refusal/122-process cleanup are
distinct histories.

Review then caught a second validity gap: rejecting a producer as not-built
would still have earned the legacy main a KILLED verdict. The two scoped
scripts now accept only consistent nonempty test results: Cargo exit0 with
positive passes/no failures, or exit101 with actual failures; the canonical
browser helper exit0 with positive passes/no failures, or exit1 with actual
failures. Other exits, missing summaries, zero tests, contradictory statuses
and time/memory stops are REFUSED. They earn no kill and fail the campaign.
The main has independent refusal/kill counters and verifies source restoration
through its real finally in deterministic tests. No other script gains this
stronger classification merely by association.

The sole already-declared compiler exception remains exact: the named feed
composer mutant may be killed by PW5048 from the actual feed CLI check exiting1.
A typed CompilerRefusal retains its complete code set; the mutant name and
exact predeclared set must agree. An unlisted name, additional PW6601, or exit67
cannot use that exception. There are no broad build-failure kills in these
two scripts.

With valid fixtures, the new producer regressions were RED24/42 (the earlier
fixture-name error attempt is retained separately), then GREEN45/45 including
additional actual diagnostic/exception-boundary cases. Valid assertion-failure
neighbours remain kills; invalid main states explicitly contain no KILLED
verdict and restore their temporary source bytes. Seven independent producer
controls extend own28 to35. Fresh complete35/2/12/30/74 and fullCI are pending;
no source/harness commit is made before these required gates pass.


## Natural results and guide inventory refusal

Fresh own35/35 passed with baseline75 and zero bound stops. The exact shared
Kiokun sample-data controls passed2/2 assertion kills. Corrected build_id12/12
and refusal30/30 passed naturally with0refused/no memory stops; their final
restored browser runs were explicitly green1/1 and11/11. Refusal's30 comprise
18 Rust assertion cases,11 browser assertion cases and the actual exactly named
PW5048 checker refusal. Every completed phase restored the entire frozen source
and original timing-probe byte manifest, with3082 anchors/268 scripts.

The following guide74 invocation refused before baseline or mutation because
two controls renamed when closing guide build gaps were absent from the manual
MUST inventory. The raw pre-baseline refusal is retained externally at
`/tmp/check-is-build-guide74-inventory-refusal.log`, SHA256
`cfa989d7d3b00961dad1dcdfb26767454b1db572ccf4778759e64760cc589fa9`.
This is no whole74 result. Only the two stale inventory keys are corrected to
match the existing control names, with the same GUIDE_TEST witnesses and all
uniqueness/exact-set/CHECKED assertions retained. A fresh entire74 is required.
No source commit precedes the required whole/CI gates.

All2712 frozen inputs, including uncommitted source, original probe and reports,
have byte copies in `/tmp/check-is-build-frozen35-byte-copies`; individual source
hashes are retained separately. After the natural coordinator ended, its worker
and coordinator entries were gone, every frozen source/probe hash matched, and
three configured/provider listener samples were empty before the inventory edit.

## Ordinary browser startup environment qualification

The completed35/2/12/30 natural campaign inherited nonempty KIOKUN_DATA and
KIOKUN_APP from setup. Exact selected values were recorded privately from its
coordinator/worker environments, without inspecting external data contents.
Earlier broad assumptions about no external startup read were incorrect.
Code tracing narrows the path to the unconditional legacy kiokun-server at
PORT+40: main calls load/data_dir and reads the configured KIOKUN_DATA shard.
This campaign's65 hosts were49dist+15dist-feed+1legacy, with no Kiokun-site host.
Do not attribute this exposure to all store/feed Server instances or claim
external labels/glosses were loaded here; those constructors choose Kiokun only
when their contracts import it. The legacy loader uses DATA, while APP was also
present in the inherited environment. These historical browser results retain
that environment qualification; there is no hermetic ordinary-host claim.

Subsequent guide/fullCI/final-recipe launches explicitly unset both variables
after setup so the legacy host selects the committed sample. The hermetic unit
negative child still sets its own missing private paths; production deployment
configuration remains supported. Later host-selection work owns ordinary browser
configuration isolation; this track does not merge or implement that feature.

## CI3 exposed the stream test's stage boundary

Fresh post-style whole35 passed35/35 (baseline75,0bound stops), then CI3
naturally failed only `streams_and_resources::a_stream_builds_as_a_part`.
Its complete accepted A-008 fixture hit the already documented generated-WIT
collision, not a new language error. Raw CI3 is retained externally as
`/tmp/check-is-build-ci3-streams-red.log`, SHA256
`b7dfc2290b6f7e090f11216097e84db890a5cabe9ca95bd48126fc4634cafe79`;
launch SHA256 `3558e999357e5a44b84047fbdd534bc6c1c022e77efdf5db4dbe30fcaf6a905b`.
CI3 stopped at test-unit (exit101), wall10:11.90,user2073.98,sys151.38,
363%CPU,load4.91→9.37. No full-CI success is claimed. All2289 frozen source/probe
paths matched the launch manifest after exit, the original probe was restored
untracked, its backup was absent, and three68-host/provider listener samples
were empty before editing.

The integrator approved a test-only correction before its implementation:
keep A-008 untouched, retain its exact complete-program PW6601/Stage::Wit
reason, responsible query declaration and full refusal wording, and exercise
stream template lowering through the existing semantic-program callback and
`build::templates`. The original test inspected only the stream part manifest;
it did not establish executable component support. A separate explicit complete
negative now prevents that stage positive from being read as whole-program
acceptance. Public CLI validation, artifact proof and backend behavior do not
change. Mutation TESTS selections and full applicable campaigns must be audited
and run before a source checkpoint; the fresh full CI remains required.

The integrator also independently proved an invalid producer-exit gap in the
current guide harness. The completed guide74 is historical evidence for its
current controls, not proof of guide producer validity. A separate reviewed
follow-up after the current source/recipe checkpoint will address it; neither
this correction nor the two repaired browser scripts closes that gap.


## Final check/build checkpoint: source d74e29ff

The tested source checkpoint is `d74e29ffe11cdbc382fcf99f4bcd892f28bc4bec`.
All applicable whole campaigns and full CI preceded that source commit. The
original user timing probe remains untracked at its original path and SHA256
`230eab1aea7b2dadc5b9477fc046c73d5a6d72215831be0e7cf70415e6c1cb6f`.
It was temporarily relocated only under the approved fmt/CI restoration guard;
no probe or backup is committed. No diagnostic ceiling or accepted source changed.

After the test-only stream correction, `streams_and_resources` passed 4/4 and
its sole selected whole mutation script passed 3/3 actual assertion kills
(baseline 4, no memory stops). The exact complete accepted-corpus CLI refusal
gate remained 1/1 green. The original stage-positive manifest assertion and
complete A-008 Stage::Wit refusal are both explicit. Fresh own35 passed 35/35,
baseline 75, zero bound stops; the entire frozen source/probe manifest matched
before fresh full CI4.

CI4 naturally passed lint, all workspace unit/doc tests, semantic conformance,
the exact four complete-program accepted refusals, reference applications and
248 program mutants (11 exact named checker refusals, zero failing). Raw CI4
SHA256 `64e85b9e91bc2c9e766140f6418456ef32e9579c82bab28134f6e72a1fc91455`;
launch SHA256 `b794ceff082e2383daa98c247d80163a124bbba3b28de925a5aa000350f5d20a`.
CI4 ran against the frozen staged inputs at prior HEAD30f7e670, manifest SHA256
`8d6f39d60798565e7065cdc875dd8475007ddf653829484b2e4e2df8f7f73ae8`;
the source checkpoint commits those tested changes. Its wall time was 10:25.41,
user 2387.97 s, system 121.97 s, CPU 401%, with load 3.47 to 9.38. All 2,289
source/probe paths matched after CI, the probe backup was absent, and three
samples of all 68 configured hosts plus provider ports were empty. Earlier
Clippy/stream CI failures remain separate retained RED records.

The actual `just e14-check-is-build` then ran from clean committed d74e29ff,
with both deployment variables absent after setup. It passed core 10/10,
CLI 7/7, corpus 3/3, corpus-producer regressions 2/2, full own35/35 (baseline75,
zero bound stops), and latency 2/2. Its complete raw output is committed as
`docs/evidence/E14/check-is-build.txt`; SHA256
`5bd73722efcf7e1bd5223caf133d0f8d026a2e37f1a608435318194db688afa6`.
The full recipe took wall 6:29.35, user 360.40 s, system 50.83 s, CPU 105%.
Source and original probe bytes matched after natural completion; the coordinator
and child were gone and three configured/provider listener samples were empty.
The receipt index preserves exact source, raw-report and campaign hashes.

The fresh census is 407 program assemblies covering all 563/563 tracked `.pw`
files: 11 clean, 323 semantic/syntax refusals, and 73 support-refused assemblies.
An exact extraction found 16 unique PW6601 messages. The 73 assemblies inherit
accepted-corpus gaps through generality compositions; they are not 73 distinct
mechanisms. The four accepted-language backend gaps remain exposed, with exact
source/stage/subreason/message and no-artifact gates: mounted resources, the
A-008 generated API collision, build-input page computation, and a style directive.
Semantic language conformance remains clean. These are not invalid-language or
completed-feature claims. The committed census now comes from the final compiler.

For the unchanged 55-file store program, three complete CLI samples were:

| Sample | Wall s | User s | System s | CPU |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1.49 | 1.48 | 0.01 | 99% |
| 2 | 1.48 | 1.47 | 0.00 | 100% |
| 3 | 1.45 | 1.45 | 0.00 | 100% |

Program-source SHA256 is
`1ff8f9ebb52c8a16ffca9f5b268878c0ad53277294447cb8aab2821755b2d303`;
compiler SHA256 is
`012359a0d8dfb7654a1d27380d8f8f004efef409e46cd1a5ae675fd0a86516f9`.
The latency gate measured cold 1405.93 ms, edit 1320.89 ms, clean 1390.39 ms,
and reject 696.81 ms. Its full command took wall 34.48 s, user 34.27 s,
system 0.26 s, CPU 100%; sampling load was about 5.15 to 5.00. The two-second
ceiling is unchanged. These measurements describe this exact source and host
condition, not a guaranteed speedup over earlier conditions. Integration adds a
larger store and generic solving; its full-program cost must be measured again.
No e9-latency campaign was run while other workers were active.

The distinct completed whole inventory is 21 scripts / 287 controls; repeated
own35 runs are counted once. The table indexes actual retained full reports,
not a claim that every historical campaign ran at d74e29ff:

| Script | Complete controls |
| --- | ---: |
| one_checker | 5/5 |
| pure | 16/16 |
| computed_signals | 17/17 |
| derived | 3/3 |
| handler_placement | 4/4 |
| block_clauses | 13/13 |
| policy_value | 14/14 |
| rendered_children | 8/8 |
| view_element | 6/6 |
| template_value | 11/11 |
| signals | 8/8 |
| speculation | 5/5 |
| page_speculation | 4/4 |
| what_a_component_does | 12/12 |
| wit_name | 5/5 |
| build_id | 12/12 |
| refusal | 30/30 |
| guide | 74/74 |
| check_is_build | 35/35 |
| kiokun_test_data | 2/2 |
| stream | 3/3 |

The prior-head qualifications remain: the combined35/2/12/30 campaign inherited
deployment variables; its browser launches had the narrowed legacy shard-loader
exposure described above. Isolated guide74, CI4 and the final recipe unset both.
Corrected build_id/refusal final restored browser runs were explicitly green
1/1 and 11/11. Guide74's final cleanup gated APP_BUILD, not a second browser run.
Its 74 current controls passed, but the integrator's independent invalid-exit
probe proves a separate guide producer-validity gap. That is an assembled-train
blocker and a separately reviewed follow-up, not a check/build source failure.
No all-suite propagation or strict guide producer-validity claim is made here.

The remaining 23 other-script cleanup gaps also remain explicit and parent-owned:
block_parts, canonical, cart_lines, command_answers, command_order, command_retry,
connection_faults, documents, holds, instance_changes, keepalive,
materializer_failure, navigate, not_found, one_entry, page_policy, query_params,
slots, soft_navigation, store_accounts, stores, stream_boot and test_controls.
The integrator's mocked-main outcomes are unit-level harness evidence, not
browser failures or completed whole campaigns. This checkpoint neither repairs
nor silently credits those other scripts. Both interrupted campaigns remain
partial/RED and earn no whole pass. The reservation ruling above remains an
integration prerequisite owned by the parent. No merge, push, authoritative
status edit, ADR numbering or larger-train landing is performed by this worker.
