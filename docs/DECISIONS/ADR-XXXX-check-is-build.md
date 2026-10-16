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
