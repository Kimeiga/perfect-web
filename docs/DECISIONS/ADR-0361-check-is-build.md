# ADR-0361: check and build validate one executable program

Status: implemented; on train-11's base as `track/check-is-build-train11`
(2026-10-19, the next section). The integrator approved the ownership design
and PW66xx allocation on 2026-10-16. Open for the integrator: the check's
cost against train-10's latency ceiling (below).
Milestone: E14, developer feedback (check-is-build).

## On train-11's base: `track/check-is-build-train11` (2026-10-19)

This section is the decision's current state. The sections after it are its
history: the check-is-build line to e82b0d6c on its own base 17fade14, then
Codex's assembly of that line onto 9902c62b beside a generic-effects overlay.
Their results were measured at those commits, not at this branch's.

**The branch** is `track/train-10-land` 587657ff (train-10's head, train-11's
base) and:

- 33f44bce, the merge of `track/check-is-build` e82b0d6c. Its conflicts are
  resolved as the assembly resolved them (its first overlay: manifest
  f471b0c7, 2,861 paths), without the assembly's generic-effects overlay:
  each path is a 3-way merge of 587657ff and the assembly over
  `wip/train10-generic-merge` 600ebf19, which equals that overlay's generic
  part byte for byte. So D1 tables (ADR-0338) are composed in:
  `program::validate_with`/`validate_sources_with` take the released schema,
  `pw check` honours `--schema-lock` as `pw build` does, PW0366 refuses before
  any backend runs, and `build::prepare` is handed the accepted tables and
  schema. By hand: refusal's mutation script (train-10's `--part i/2` with
  this decision's refused/killed counts and its gate on the restored browser
  suite), evidence-integrity's latency mutant (it repeats
  `validate_sources_with` ten times, where checking is now), the justfile's
  import, and the codes reference regenerated (205 codes, 92 cite an ADR).
- 281747fd, the assembly's later work on 14 paths, merged two independent
  ways that agree: privacy facts derived once (below); producer validity
  (`mutation_source.py`, and `unittest_producer.py`/`unittest_protocol.py`,
  byte copies of guide-producer-validity 6abc09f3's); this decision's
  controls from 35 to 50 and held-labels' memory watch; the CLI corpus pins
  (A-052's PW0365 warning; R-106/R-107 with their schema locks). The C8/R-035
  classification, the `calls_with_types` audit entry and E2D's R-090..R-095
  rows (sections below) are generic-effects' own: its branch carries the
  same bytes, and this one does not.
- 19ab8166: refusal's script now runs in parts and reads its arguments; the
  restored-browser gate's tests ask it for every mutant and mutate as a
  browser mutant does (under `unittest discover` 13 of them had exited on the
  runner's arguments), and the control on that gate anchors at its new depth.
- 5b740407: the program census covers train-10's `examples/tables` app (the
  census refused: four tracked sources uncovered).
- 6fa7c616: the evidence below.

HEAD equals 587657ff plus the assembly's check-is-build delta on all 61 of
its paths, except exactly the four hand resolutions, and 587657ff everywhere
else.

**Privacy facts once.** Complete validation derived the privacy summaries
and `Reads` in the semantic check and again for the contracts (profiled
2026-10-16: about 57 ms of CPU a check). `Checked` owns the exact
`PrivacyFacts` of its own preparation and lends them to `contracts_checked`;
standalone wrappers derive their own.

**Results** (lane worktrees at this branch's commits; load 20-115 from other
workers throughout):

- `just e14-check-is-build` at 5b740407 (docs/evidence/E14/check-is-build.txt):
  pw-core check_is_build 12/12, pw-cli check_is_build 7/7, check_build_corpus
  3/3, test_check_build_gate 15/15, this decision's controls 50 of 50 killed
  (baseline 90, no bound stop), check_latency 2/2.
- Census (check-is-build-programs.txt): 432 program assemblies, 598 of 598
  tracked sources; 14 clean, 337 semantic or syntax refusals, 81 support
  refusals. Against e82b0d6c's 407: 25 new programs, no changed verdict and
  the same 16 unique PW6601 messages, which are the four accepted-corpus
  backend gaps (A-007, A-008, A-013, A-018) as every assembly that includes
  the accepted corpus inherits them. Nothing is newly refused; every
  reference app (store, feed, kiokun, kiokun-site, demo, tables, guestbook,
  hello-static, koka, render, the benchmark's store) checks clean.
- The planner's reach (`ci_plan.py --changed 587657ff HEAD`): 54 recipes, 53
  mutation scripts, 792 mutants. 52 scripts ran whole and killed all their
  762 mutants, none surviving and none refused: the 43 cargo-only ones (544
  mutants), eight browser ones (computed_signals 17, provide 29,
  computed_conditions 12, computed_rows 12, speculated_arms 8, proof 4,
  build_id 12, guide 74) and this decision's 50. refusal's 30 ran as
  train-10 splits them, each clean: part 2 (`--part 2/2`) 7 of 7 and part 1
  (`--part 1/2`, 2026-10-20 08:28-09:21Z, load 13-61) 23 of 23, both with the
  restored browser suite green; earlier attempts at load 90-130 ended refused
  (Findings in the track file). So all 53 scripts, 792 of 792.
- `cargo test --workspace`: 350 binaries, 3,090 passed, 25 ignored; four
  pw-dev-server timing tests failed at load 50-67 and passed on re-run.
  mutant-programs 301 judged, 16 refused by their named rule, 0 failing;
  evidence-gates 326 OK; clippy, fmt, case-check, anchors (3,330/274) green.

**The check's cost (ruled 2026-10-20).** Complete validation runs
the backends a build runs, so a check costs about twice a semantic check.
check_latency's binary built at 587657ff and at 5b740407, run alternately on
the same program (the store, 56 files, 6,568 lines, which this branch does
not change): 245-416 ms of CPU a thousand lines at 587657ff, 542-837 at
5b740407, a ratio of 1.9-2.9 that moves with the load and with the hybrid
CPU's core (an E-core run costs about twice); the record before agrees
(17fade14 756 ms cold, d74e29ff 1,406: 1.86x). Train-10's ceiling is 800 ms a
thousand lines, two and a half times ARM's 321.67 for 587657ff: at about
twice, ARM would measure some 640-740, inside the ceiling with 10-25% to
spare instead of 150%; on this PC under load runs pass at 542-761 and fail at
809-967. The same binaries with dependencies optimized (`--config
'profile.dev.package."*".opt-level=2'`, measurement only): 213-423 at
587657ff, 341-586 at 5b740407: wasm-encoder, wit-component and wasmparser
run unoptimized in a debug check. Ruled 2026-10-20: the 800 ms ceiling does
not move; ci-diagnostics bounds the fastest of 7+ checks. Measured: CI x86
517.48 and ARM 640.94 ms a thousand lines (ci 39386818639), about 1.97x
train-10's 262.04/321.67; here at load < 50, fastest of 9 checks
462.5-520.6, median 562.2-566.5 (semantic-only 223-259). Per phase (median
ms): components 905-919, handlers 279-290, contracts 134-140, templates
96-100, speculations 74-77, a syntax re-parse 62-66, pages 58-64; semantic
analyses 1,455-1,469, lowering 189-191, inference 149-176.

**Claimed:** `pw check` and `pw build` share one complete validation on
train-11's base, D1 tables composed in: what checks, builds, but for
build-only IO. **Not claimed:** that the four accepted-corpus backend gaps
are closed (both commands refuse them now, with PW6601); that the evidence
in the sections below was produced at this branch's commits.

**For the integrator.** Merge this branch before guide-producer-validity
6abc09f3: that branch is built on e82b0d6c, so merged first it would bring
this decision's old line in unresolved; after this branch its merge base is
e82b0d6c and only its own 13 paths come in, conflicting only in
`scripts/cargo_test_results.py` (its version against train-10's, add/add;
its `unittest_producer.py` and `unittest_protocol.py` are byte-identical
here). A dry-run merge of generic-effects 618abfbe onto this branch
conflicts only in docs/STATUS.md, NEXT.md and DECISIONS.md, the codes
reference (regenerate it: `python3 scripts/guide_diagnostics.py`) and the
justfile's imports (keep both).

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


## Integrator composition ruling before isolated implementation

This candidate is separate from the active frozen root campaign.
Feature evidence above remains its original-source history; no assembled gate is yet claimed.

Integrator content ruling for the later D1 + Check-Is-Build assembly
2026-10-16. NOT IMPLEMENTED; active root source remains frozen.

The pre-feature CIB base is17fade14e5f5567d3bfcc0849019885cfbf63fb2;
a84c064a is the first implementation, e82b0d6c the ready checkpoint.
Content-only rehearsal against frozen generic-v2 has four textual conflicts
(CLI main, core build/check, latency test). Actual Git ancestry still has
master22a as merge-base. Use the feature delta to distinguish duplicated
history, never to discard other root fixes. Ownership classification is
/tmp/check-build-cumulative-ownership.json; content report is
/tmp/check-build-pending-merge-review-v2/report.json. No candidate was built.

Preserve CIB's sole complete executable validation, private final plan,
sealed Checked proof, callback-only semantic borrowing, syntax-clean sibling
diagnostics and native backend refusals. Preserve D1's --schema-lock CLI,
PW0366 migration diagnostics and tables.json/schema.lock artifacts. Preserve
generic-v2 contextual lexical/callee identity and its backend caches. Raw
pattern case lookup remains pattern authority. No public Checked::of returns.

Concrete composition direction, to record in the feature ADR BEFORE coding:

1. Add schema-aware full validation entry points in program (validate_with /
   validate_sources_with or equally explicit names); current names wrap None.
   Keep semantic-only APIs distinct. CLI check and build consume the same
   full program authority with the same optional released schema.
2. After clean semantic preparation, derive tables_json once in full program
   composition using the exact checked context and optional release. Append
   native PW0366 diagnostics with their original unit/span/code/reason. If
   migration errors exist, refuse before executable preparation. Do not turn
   PW0366 into a generic PW6601 backend refusal.
3. Lend/move that exact accepted TablesJson+Schema into build::prepare. The
   builder writes these accepted artifacts and does not independently derive
   or revalidate a second migration plan. Preserve CIB's early and support
   refusals and partial-inspection-only API boundaries.
4. Retain D1's public semantic check_sources_with(files,released). Rename CIB's
   private parsing/callback helper to avoid colliding with that public API.
   Add a full schema-aware check_program_sources wrapper for CLI. Syntax
   errors still retain diagnostics of clean siblings, without executing a
   backend over recovered HIR.
5. Combine the existing CPU/wall measurement with full executable-program
   checking in check_latency. Keep the existing2-second bound; measure real
   final assembly and repair duplicate work if it exceeds the contract.

Required causal integration tests (beyond preserved feature tests/controls):
same released lock accepted/refused by both CLI check and build; unstated
migration retains exact PW0366 and writes no executable output; stated
migration writes matching tables.json/schema.lock; a backend-unsupported
source still gives PW6601 with a schema lock; mixed syntax/semantic siblings
retain independent findings; generic lexical/property/callback regressions
and the actual receiver-identity HTTP witness stay green. Run the applicable
full CIB35 and affected full D1/generic/guide controls on final anchors, native
program census, full default CI, fmt and source restoration before commit.

This is an integration direction from source review, not a new result or a
claim that the assembled cost/backend gaps are closed. Revise it if an actual
causal test exposes a defect, recording the evidence before implementation.


### Isolated D1 composition: measured guide merge and moved controls

The isolated composition compiles. First focused core command passed109
tests across7 binaries, including native migration unit/span/reason identity,
generic/callback/lexical/bounded effects and backend lowering. Exact input
pathset/bytes remained unchanged. RawSHA48900a84d3e632a55f90d9ea3e06140b858a5b5265d266c3b5c98282f40e8e01.
The next CLI command passed check-is-build7 and guide14, then failed one
guide example: the content merge combined PW0401 prose with the incoming
pure function body, which actually receivesPW6601. The table CLI binary and
latency command were not reached. RawSHA93f0cfd9b31fa13a396178ed3b423981dc58b82ee18471639c87ef4f2ac6e7e2.
Ruling before correction: restore exactly the assembled root's effectful
host function underPW0401, retain a separate pure computed-head example
underPW6601, and regenerate the registry-derived diagnostic reference.
This is a guide content merge error, not evidence of a semantic escape.

Global anchor preflight refused5 of3328 controls because the composition
moved their authorities. Before re-anchoring: CLI check must still restore
semantic-only checking while preserving the same released lock; CLI build's
complete validation failure must still become an incorrect success. D1's
build lock control moves to program's single tables_json derivation; its
semantic source lock control retains the same held_to call after diagnostic
variable renaming. The latency control must now repeat full executable
validation five times, as the measured CLI promise changed from semantic
checking. Preserve every control name/count/test and require complete affected
whole suites with actual assertion failures before any assembled gate claim.


## Integrator privacy-sharing ruling before correction

The assembled unchanged CPU2s latency gate is RED1/2: cold2030.62ms and
edit1929.98ms; an earlier clean measurement was2217.40ms. The original raw
`/tmp/train10-cib-assembly/second-focused/1.txt` remains failed evidence,
SHA256266e16f27f51e7ff4a5a6362ed82fb604b488bcd59089340d876aa7f3930a29c.
No ceiling, parser proof, solver behavior, component encoding or audit may
change as a response to that result.

Three actual stage profiles retained the complete executable checker. Every
profile restored the same2861 source paths and hashes, and the pristine CLI
was rebuilt afterward (SHA256f03f0baf81185a4dbe826caafb86bc58f594b692eed07da2e78bbd6d81a90c04).
Raw/restoration/measurements remain outside the repository under
`/tmp/train10-cib-profile-20261016`, `-v4` and `-v5`. The two intervening
instrumentation preflights refused ambiguous timer anchors before running
Cargo; both restored every byte and earn no test or performance result.
The final profile raw SHA256 is
19ba2da919fc29d5a24f4869ad7e3bde9a4555b5768d8e92a9657f2db041a8a8.
These instrumentation runs are measurements, not a replacement green latency
gate. Their raw numbers remain separate from the original RED.

The profiles identify one narrow repeated computation. The semantic checker
builds `summaries(hirs,sigs)`, then `Reads::of(...).answering(&summaries)` and
its labels. Contract generation immediately repeats the exact derivation for
the same HIR/signatures/inference. The repeated contract step costs57.41ms
CPU median in v4 and59.32ms in v5 (its summary alone48.59/50.41ms). Tables and
cache cloning are below0.1ms; resume typing is only8–10ms per site. The55
component encodings and55artifact audits are actual required work and stay.

Integrator approved the following before permanent edits,2026-10-16:

1. A crate-private immutable `check::PrivacyFacts` owns the existing summary
   and label `BTreeMap<DefId, privacy::Label>` values and existing `Reads`.
   Reads contains owned maps of labels, strings and DefIds; no inference or
   HIR borrow is retained. One private derivation factory serves both paths.
2. The semantic checker retains every analysis and its order, and returns
   these exact already-derived facts through an internal prepared result.
   Its existing semantic-only wrapper keeps its signature and diagnostics.
3. `Checked::diagnose_inferred` retains exact unit/HIR identity and syntax
   checks, and stores the facts in a private `Rc` only after clean semantic
   diagnostics. `with_contracts` clones only this Rc for identical source
   context. No public Context field, cache-input API or proof constructor is
   added, and no cross-program/global cache exists.
4. Complete composition calls `contracts_checked`, which borrows the sealed
   Checked's exact context, pages and labels. The contract implementation
   retains all native labels, answered-secret/placement/capability rules.
   Existing standalone contract wrappers derive their own facts once and
   feed the same private contract body.
5. Behavioral tests compare standalone and complete contracts, distinguish
   cross-module namesakes, and repeat separate programs with the same DefId
   positions and different privacy. A genuine empty/foreign-label control
   must fail a named behavioral assertion. No committed derivation-count
   test or control merely repeating semantically equivalent work is added.
   Profiling counts remain profiling evidence only. Existing proof-forgery
   control must remain operative with the new private field, rather than
   earning a build-failure kill from an obsolete struct initializer.

Run focused privacy/generic/backend/schema tests, the unchanged actual2s
latency gate, full source/pathset receipts and global mutation anchors before
reporting this candidate. Full relevant whole controls/CI remain required for
eventual integration; no source commit or optimization completion is claimed.
The measured57–59ms elimination is not a guarantee that the2s bound passes.


The first focused compile caught a required native consumer before any test
ran: contract Demand also joins `Reads::holds(def)`, including answered
secrets. A labels-only parameter does not represent the complete facts. The
private contract body therefore borrows the entire immutable bundle, using
its labels and unchanged holds projection. The full path receives it only
from the sealed Checked; standalone derives one complete bundle. The genuine
control supplies an empty bundle only to the complete path, because removing
only labels can be absorbed by holds. Initial compile raw remains external
`/tmp/train10-cib-profile-20261016-v5/privacy-focused.txt`; it is not a test pass.


Narrow implementation checkpoint, before final focused gates: the checker
retains its exact summary/Reads/label derivation in an owned private bundle.
Checked stores an Rc only after its unchanged identity, syntax and semantic
gates; with_contracts clones that Rc. Contracts borrow both native labels
and unchanged Reads::holds through the same bundle. Public Context/proof
APIs, parser/solver behavior, every component/audit and the2s bound are unchanged.

The focused CIB suite passed12/12, including behavioral contract equality,
qualified module namesakes, three alternating same-DefId programs and native
cross-module PW5001 early-refusal parity. A genuine empty-facts control failed
the named helper-hidden-session assertion0/1 with nativeCargo101; rawSHA256
f062eea4c7f41503f3f292dac868f29240a756fe36bfce4c198215a742367197.
The existing public-proof-forgery control now initializes the added private
field but still makes the compile_fail doctest compile and fail0/1, actual
nativeCargo101; rawSHA256be44a21c81cfbae1020630b3673282cf3e922b172f0c70e5a741c1844957f409.
Both full outputs and exact byte-restoration receipts are external under
`/tmp/train10-cib-privacy-final`. No derived-work-count test/control is added.

Global anchor preflight found one moved contract holds projection. Its
existing held-label control now removes the same `.join(&privacy.holds(def))`
instead of `.join(&reads.holds(def))`; it is not dropped or credited here.
The CIB roster is36 (originally35) and held-label roster remains8. Global3329/275 anchors
match once. Full current-source wholes and CI remain pending; earlier feature
campaigns retain their original source qualifications and are not relabeled.


## Result-producer repair ruling before consumer edits

The privacy-only compiler candidate passed the unchanged latency gate2/2:
coldCPU1886.52ms, editCPU1793.51ms; the separate clean comparisonCPU1995.86ms
remains close to its unchanged2s ceiling. Focused core18binaries223/0,
CLI26/0, Checkedunit1/0 and public-proof doctest1/0 passed. The eight-command
receipt preserved all2861 paths/bytes; raw and native exits remain external
under `/tmp/train10-cib-privacy-focused-final`. This does not credit a whole
mutation campaign or finalCI, and original latency RED remains history.

Integrator review then found a separate evidence defect in the two affected
whole scripts. Mocked children only, without signals or launched processes,
reproduced Cargo failed summaries accepted at native0,67,-15 andNone;
build-error text plus a failed summary was also accepted. Both mains called
a build failure KILLED and returned0. CIB additionally trusted Python count
text at invalid exits and fixture errors without any executed body. Actual
regression red is4methods/17failing subcases, nativePython1; first3/12rawSHA256
12e5f2df0d40863ed21c67f206236840768c613eecc6289237686b48253e4e1b.
Full raw histories are `/tmp/train10-cib-privacy-review/producer-before.txt`
and `producer-python-before.txt`; they provide no real process-stop evidence.

Parent approved the bounded repair before code on2026-10-16:
- Preserve all36 CIB and8 held-label tuples, original Cargo commands and
  Python module selections, bounds, groups, timeout and process cleanup.
- Consume exact canonical Cargo validatora177209c. Unknown/build/stopped
  results are campaign failures after restoration, never kills. Native
  doctest assertion text is retained; no second Cargo count parser is added.
- Copy exact immutable synchronous unittest producer902dfef3 and pure
  validator91e941dc from reviewedv17. Freeze actual method IDs for each of
  CIB's three existing Python selections before changing source. Every run
  gets fresh dedicated expected/report files; validate native exit, complete
  report and exact frozen IDs. Fixture, stale/missing/invalid and ordinary
  body-error outcomes are refusals; only body assertions earn kills.
- An invalid outcome aborts after exact source restoration with no next
  control. Unexpected exceptions retain their original identity through
  secondary restoration/diagnostic failure. This does not expand process
  ownership, change cancellation/reaper policy, or alter compiler facts.
- Add behavioral mock classifier/main restoration/no-next regressions and
  operative wrong-result controls. Global anchors and whole scripts must
  run on the final refrozen source before any integration/CI readiness.

The earlier64/63 roster prose was a counting error: AST and loaded inventory
show36 current CIB tuples and35 in the frozen pre-sharing baseline. The
corrected metadata does not add or claim any unperformed control.


Before the test-loader correction, a private synthetic cached module witness
was RED: both existing cleanup-test and new consumer-test loaders sawVALUE1
from preseeded bytecode while same-size/same-mtime source readVALUE2. Actual
nativePython1, one test/two subcase failures, no project source mutation or
project process; raw `/tmp/train10-cib-privacy-review/loader-before.txt`.
Parent approved before edit: retain module_from_spec identity/globals but
execute compile(path.read_bytes(),str(path),'exec') into that dictionary.
This changes only the two test loaders, not producer902d/protocol91e9, test
selection or process policy. Parent also approved two actual loader-reversion
controls, bringing proposed final CIB roster46 and held-label8 (existing
44+8 retained). Final revised source/consumer evidence still requires review
before any whole launch; no new whole result is claimed here.


Before whole release, parent found one retained memory-stop accounting gap:
held-label main imports the existing mutation_baseline Watch but did not
consult its stopped list. A fully mocked flag-only witness, without child
launch or signal, is RED2/2: baseline and mutant both accepted a known stop
with apparently valid output, credited a kill and returned0. Raw retained
`/tmp/train10-cib-privacy-review/watch-before.txt`, nativePython1.
Parent ruling before code: obtain the existing watcher through
mutation_baseline.mutation_bound.start() (it reuses that active watcher),
and make both baseline/mutant validity include an empty stopped list. Abort
after source restoration with no next control. Keep bounds, signals, groups,
reaping, original primary identity and held8 tuples unchanged. Add one real
guard-removal control to CIB; proposed final roster47+8. No new watcher or
process ownership is introduced, and this is no evidence of an actual stop.


Before launching any whole, a fully mocked private report cleanup probe found
one new unwind defect: TemporaryDirectory.__exit__ could replace an existing
SystemExit37 with a secondary cleanupOSError. RED1/1/nativePython1, raw
`/tmp/train10-cib-privacy-review/report-cleanup-before.txt`, SHA256
5414e09cbebc308ce4d516ec964168b028733434b04ab636e226d8ab71e90fab.
Parent approved explicit owned report cleanup and the same cancellation
precedence as the external observer. A second mocked source-release probe
then measured two failures: an ordinary primary swallowed a first SystemExit
from release, or a first KeyboardInterrupt from diagnostic output. Existing
primary cancellation already retained exact identity. Rawsource-unwind-before
is3methods,1green/2red, nativePython1; no process/signal was involved.

Ruling before correction: share only this unwind rule between source/report
cleanup. Existing SystemExit/KeyboardInterrupt primary retains exact identity;
otherwise a first cancellation during release/diagnostic outranks an ordinary
primary. An ordinary secondary cannot replace an existing primary, and an
ordinary cleanup failure without a primary remains visible. Report cleanup
is explicit and owned. Preserve every existing test selector, process group,
reaper, signal and bound policy, and unchanged compiler/semantic inputs.
Add report-caller reversion plus separate proven release/diagnostic-cancel
controls; retain the existing source-primary control at its same authority.
Proposed final roster50CIB+8held; source/focused review still precedes wholes.


## Accepted D1 warning distinguished from backend errors

First released whole50 attempt failed its unmutated baseline89passed/1failed,
actualCargo101 in accepted_corpus_has_exactly_its_known_backend_refusals;
zero mutants were applied and held8 was not started. All2864 source paths
were exact afterward. OwnerPID3879781 naturally reaped at13:59:35UTC,
raw `/tmp/train10-cib-consumers-whole-v1/check_is_build_mutations.py.txt`
SHA2563b2c855fd9bc193ac11bab9127945ef2053a445db3622b165c801de8e9726a9a.
Observererrors0/stops0; this is failed baseline history, not mutation evidence.

Causal evidence shows no new semantic refusal. D1 accepted A052 explicitly
expects warningPW0365: a reference across databases may name no row. Native
structured validation still reports exactlyfourPW6601 errors plusone
Warning/DeclarationRule/refers_across_sources at A052 bytes628..652
(`refers author to Members`), with related A051Members bytes479..486,
label`Members` is held by `MemberData`, canonical targetunit88. The CLI check
renders this warning alongsidefourerrors; build's errorDisplay includes
onlyfourerrors and writes no artifact. The test's generic codes(text) helper
incorrectly treated the warning as an additional error. Immutable copiedCLI
SHA256d0d7a21637fd1cc144c9859cba7963d74a8397af557852d0582bdfebb04d2ca3
and private actual structured witness/raw/argv receipts are retained under
`/tmp/train10-cib-privacy-review/corpus-red`.

Parent ruling before test-only amendment: preserve production, corpus
programs/@rule/diagnostics and exactfourbackend refusals. Pin exactlyone
structured D1 warning separately (severity/reason/message/source/primary
and related target unit/file/span/label). Check output must retainfourerror
headers,oneexactwarningheader/source and4/1summary; build retainsfourPW6601,
noPW0365 and no artifacts. The related unit is pinned through its resolved
public file identity, without exposing the private Related.unit field.
Leave the generic codes helper and rejected-corpus parity unchanged. The
original50CIB+8held tuples/count remain fixed; refreeze and rerun complete
wholes after the exact focused command passes. No failed history is relabeled.

The reviewed test-only patch also pins both retained native CLI refusal exits
as code1, replacing generic non-success only within this accepted-corpus
command. Parent approved this explicit status assertion before its amendment;
all other corpus tests and original50+8 mutation tuples remain unchanged.


## C8 R-035 historical receiver classification (integrator ruling, 2026-10-16)

The host-composition default CI exposed a real RED in corpus_history: C8's original R-035 did not report wrong_frame_phase. Its ElementRef annotation has no browser import. The canonical receiver solver refuses this unresolved type and cannot invent style effects by a global member spelling. This is FixtureDidNotExpressIt under the existing Why taxonomy, not a phase-semantics regression and not acceptance of the old program.

An external immutable-input public check_sources probe, linked to already-built exact-hash pw_core/serde_json rlibs without Cargo or project changes, pins the causal distinction. Original: PW0026 unresolved_type528..551; PW0408 member_receiver_untyped736..744,736..756,779..787; PW0605 argument_type857..881. Adding ONLY import browser.{ ElementRef } restores PW0402 wrong_frame_phase766..792 at exactly it.style.set_padding(4.px), beside the old width/maximum defects PW0610/PW0605. The current typed fixture reports ONLY that PW0402; deleting ONLY the illegal setter makes its typed neighbour clean. Every other library unit is clean. The existing causal_evidence tests forbid ProgramWideName routing and effects invented for a receiver with no such member.

The TEST-ONLY correction explicitly classifies this one C8 original and requires its five exact original code/symbol/span/source-text/message refusals, import-only causal control, current typed violation and clean typed neighbour. All25 historical files and the25-file floor remain unchanged; the other24 retain the original rejection assertions, including the existing R-019 parser boundary. Actual counters report23 declared-invariant catches +1 existing R-019 syntax refusal +1 classified outof25, never24 or25 original target-invariant catches. The existing R-019 assertion is unchanged. A mismatched required rejection fails before any success measurement is printed. Production scope, ambient resolution and expected-code allowlists are unchanged.

Actual immutable receipt: /tmp/browser-c8-readonly-2mr9np78/receipt.json SHA256b4a04bc47e8a1ba04c435ab7ce9e0b52f8624b0af004b8e9a19354f0403e2c44; probe46ad30c3811043e0604333aea2c3f3f2f7b17aeb201892adb05d2aed5fb1cc4f, binary29e294ae82bad61d3227ed07989a22ff78b9bef775af6761a70d62db4bf8964d; pw_corec19ff415c3663da2794dabcc84ee93b9839233670702ec2b970a7f5d4056eb4d, serde_jsonf37867b2cfe65dd2cd73fe1cb915593ba7dfb685e22e2fb3d8bab8667e78a613. Complete structured native diagnostics, invocation argv/exits,49 exact library entries,57app inputs and source hashes are retained. Two initial external fixture/toolchain setup refusals are preserved separately and earn no semantic control credit.

This amendment precedes canonical test correction. Proposed patch is NOT applied in the active host CI tree; integrator review and quiescent canonical validation precede adoption.

This isolated CIB tree is now quiescent and parent reviewed canonicalv2 before
adoption: patch24043cb7, proposedtestf143e622 overf4ad037f. Own post-whole
v4currency7/7 is green, so no generated fixture refresh is indicated here.
Ownhistory7/8red remains rawSHAe67993c628727202ac305470018b4aa02b75dbb5fc16735b5c14a5f025ba45af.
All25 C8 file hashes match the immutable receipt before this test-only edit.
The v4whole50+8/417children proof remains historical/source-qualified; final
amended inputs require fresh focused gates/full50+8 and defaultCI.

Before metadata reanchor, unchanged last_segment is actualRED3/4/native101:
effects.rs:calls_with_types is unclassified, while stale display entry still
namescalls_from. RawSHA256a39b840675df12660f61b85512660cdd2130a0027021d3f278c3e29ff6de10d7.
Parent approved metadata-only move of that EXISTING entry to calls_with_types,
keeping display category, exact existing prose and forbiddenzero. Typed member
identity and fullpath/DefId/effects resolve before shortening; the short name
feeds only Via::Helper.helper for describe(), while callee retains fullpath.
This does not alter compiler code, whitelist membership count or category.
Ruling and actualred precede the one-entry metadata change.


### Released-schema context in the rejected CLI corpus (2026-10-16)

The final-v5 full CI exposed a test-assembly omission: the generic rejected-program
CLI loop passed the 50 program files for R-106 but omitted the fixture's existing
@schema-lock companion, so check correctly exited 0 with no diagnostics.
checking_source::check_fixture, the generality harness and the D1 table tests
already supply that released schema. A row declaration alone cannot express the
comparison with a released version.

The immutable executable d0d7a21637fd1cc144c9859cba7963d74a8397af557852d0582bdfebb04d2ca3
and copied, hashed inputs reproduce all eight command observations at
/tmp/train10-cib-schema-context-v5-v2/matrix.json (SHA256
a3712234e96e8072fa6027ae381cc0477425ce8f85b610f06e7da263ddf06267).
For both R-106 and R-107, unlocked check/build exit 0; each build emits eight
artifacts. Supplying the declared sibling lock makes both commands exit 1 with
only PW0366 and no artifacts: one diagnostic for the lost Notes.body, two for
the lost Memos.body and the new Memos.text. The first private-copy attempt
refused an overwrite of its own read-only common input after the four R-106
observations; its partial raw is retained separately and earns no completed
matrix credit.

The integrator rules this a **test-only missing-fixture-context correction**.
Forward the existing declared lock symmetrically to both public commands and
require that companion to exist. Pin native exit 1 for rejected semantic
programs; for these two lock fixtures pin PW0366 alone, the exact one/two
diagnostic counts and their existing header reasons. Keep source/library/fixture
bytes, code parity and no-artifact assertions unchanged. This amends neither
compiler behavior nor a refusal allowlist. The prior final-v5 50+8 whole results
and failed CI remain source-qualified history; refreeze and rerun the complete
50+8 controls and fresh CI after this test amendment.


### Corpus enforcement evidence after the generic integration (2026-10-16)

The complete final-v5 default CI naturally ended at 15:05:46 UTC with native
status 101 and exact original 2,864-path source restoration. The complete raw
output SHA256 is 85f458e18da32f5d613f877fb5366fb11174794cdb1971bdbccfb3b1a6fc6d48
under /tmp/train10-cib-full-ci-v5. Its full failure census is exactly the missing
released-schema CLI fixture context above and stale E2D enforcement evidence:
the report records 87/87, while the canonical current test measures 93/93. The
remaining Rust results are 3,126 passes and 25 ignored, including the unchanged
two-second latency gate 2/2; Python ran 304 passes and two skips. This failed CI
remains failed history and is not substituted with focused passes.

Read-only comparison finds the parent's validated report differs by exactly the
87-to-93 total and the six R-090 through R-095 rows. The complete
checking_source.rs authority is byte-identical in both trees (SHA256
2369ec22b14334f09199bc0a4e4de1d3a1c6d3a99eae79d40a278e5e4b363703), as are those
six fixture files. The actual evidence-corpus recipe already selects the valid
the_committed_enforcement_table prefix; no recipe edit is warranted.

The integrator approves regeneration through this candidate's existing exact
writer, not adoption by assumed compiler equivalence. Run cargo test --locked
-p pw-core --test checking_source
the_committed_enforcement_table_is_what_the_corpus_checks_now -- --exact
--nocapture with PW_WRITE_EVIDENCE=1 in that child only. Require native 0 and
exactly one executed test, generated report SHA256
04178f0ffa9e2cdf09b95165d07e7b715354aaee53078ef8b63bc1d25303e127 and the exact
six-row/header-only diff, then run the same test without write permission.
This ruling precedes the evidence write. Refreeze the amended inputs and run
the complete unchanged 50+8 rosters and fresh default CI before any checkpoint.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0361** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after the train's first seven, in train
  order), merged from `0c8d0c1e` (`track/check-is-build-train11`).
  `ADR-XXXX` reads `ADR-0361` in its validation index's scope
  (`docs/evidence/E14/check-is-build-validation-index.json`), as in this
  file's name and title.

## Amended on train-12 (2026-10-20)

- **The two-signal witness builds now.** The witness that a part cannot
  compute a value from two signals (`{a + b}`), in the CLI test and the
  guide's tour, was a gap track `device-store` closed (ADR-0366, decision
  7: the browser computes from several values it holds). The witness is a
  signal and a query's value together, still refused in the same words
  ("computes a value from `a` and `count`, and a host computes one from one
  value (ruling 0073-a)"):
  `a_part_cannot_compute_from_a_signal_and_a_query`
  (`compiler/pw-cli/tests/check_is_build.rs`). Two signals build, as `pw
  check` and `pw build` both say: `a_part_may_compute_from_two_signals`,
  held by a mutant of `scripts/check_is_build_mutations.py` ("a part
  computed from two signals is refused again"), killed by that test alone.
  The guide's tour (`shop.sum`) is a plain `pw` block, not `fails=PW6601`,
  and `docs/guide/why.md`'s list of what `pw build` refuses no longer names
  it.

## The build's refusals, one list (the coordinator's ruling, 2026-10-20)

Train-11's verify 39442025240 (at 320c52a4) failed five recipes on one
conflict the merge did not show: they ran `pw check` on
`examples/accepted/*.pw` and required it clean, and the complete check
refuses four accepted programs with PW6601 ("pw check: 90 file(s), 4
error(s)"). This corrects the census's "nothing newly refused" for the
accepted corpus. A-007 (an element that mounts a resource), A-013 (a page
value computed at build time from a workspace file) and A-018 (`style:`
directives) use features the build does not support yet; A-008 is a bug:
the WIT the build generates names an item twice,
`store-recommendations-recommendations-api` (NEXT queues it). The programs
stay as they are. The build's refusals are stated in one list,
`examples/accepted/build-refusals.json`: each program, its exact message,
its stage, the text its span names, and why. Every corpus check reads it:
`compiler/pw-cli/tests/check_build_corpus.rs`, and
`scripts/accepted_corpus.py`, which every recipe that checked the accepted
corpus now runs (twelve places in the justfile and
`just/bounded-checker.just`). A corpus check passes only where every
program not listed reports no error and each listed one is refused with
exactly its message, and it prints the list's size
(`scripts/tests/test_accepted_corpus.py` holds the comparison, with the
controls of an unlisted error, a missing refusal and a message changed by
one character).
