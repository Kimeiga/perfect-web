# ADR-0379: speculation absence names an existing page and controls require native assertions

Status: integrator-approved repair, 2026-10-21. Milestone: E14.

## Finding

Bulk verify39526960864 at f15117de passed the six compiler/parser and66 feed
browser cases, but e14-speculated-arms killed only7 of8 injected faults.
The empty-module fault survived because its negative test still queried
`feed.app.Likes`, while ADR-0373 emits `feed.app.Likes~ui`. Absence of the
obsolete name proved nothing about the actual page. Compiler behavior was
correct; this was a missed migration in the regression witness.

The campaign also treated failed builds and timeouts as kills and counted
browser prose without requiring every selected case to finish. Those results
cannot establish assertion causality.

## Decision

The absence witness first finds exactly one Page declaration named Likes in
the fixture, derives its identity through contract::component_id, and asserts
that exact identity before examining successfully compiled speculation output.
Existing positive Home and PostPage witnesses remain. Meaningful compiler
Result/Option expectations use direct assertions before extracting values, so
the same fault is attributable to the expected contract rather than unwrap.
No production compiler rule or original mutation replacement changes.

The eight-operator campaign retains all seven Cargo faults and one actual feed
fault. It delegates native exit/summary, complete5+1 identity inventories and
assertion-site line/column validation to cargo_assertion_results. Browser
results use the existing nonce-bound assertion reporter and browser_test_results
with a frozen complete inventory. Build/runtime/import errors, timeouts,
incomplete/skipped/retried cases and memory stops earn no mutation credit.
Process ownership is producer_process; source ownership is mutation_source.
Restored source must build before a campaign can succeed. Twelve producer
controls run through guide_python's complete fresh native unittest protocol,
requiring the intended actual assertion and no body error.

The feed imports the existing assertion-expect wrapper. All ordinary scalar
and locator matchers preserve native behavior, including .not. Its one poll
using toBeGreaterThan(20) becomes the equivalent completed boolean observation
`(await rows.count()) > 20` with positive toBe(true), retaining the existing
bound. The other polls already use positive toBe. No shared matcher is added.

The feed changes serial mode to default: cases still execute in declaration
order on each engine's own shared host, while failure no longer skips later
independent cases. Cases use fresh pages/contexts and their own session data;
no beforeAll or cross-case JavaScript/page state supplies later cases. Shared
host data persists in the same order. Playwright documents this distinction in
[test.describe.configure](https://playwright.dev/docs/api/class-test#test-describe-configure).
All setup, checks and cleanup are retained. The complete feed baseline and
actual browser fault must finish all66 cases; a shorter failed run is refused.

## Validation and limits

The original failed hosted record remains failed. Local raw proof records the
same empty-module mutant failing the corrected assertion and source restoration;
the six-case baseline and all seven Cargo controls pass strict validation.
The twelve focused producer tests and twelve controls pass. The full feed native
baseline passes66 cases; the unchanged browser fault yields63 passes and3 actual
assertion failures over the same complete66 identities, one failure per engine.
The whole campaign passes8/8 original faults plus12/12 producer controls, with
zero refusals, survivors or memory stops. Source and generated builds are
restored. Final hosted whole-recipe and
reached-consumer evidence remain pending.
The feed import and mode change reach existing feed campaigns; the canonical
planner determines their required whole reruns. These tests are bounded
regressions, not a guarantee against all compiler or application bugs.
Raw local commands, input hashes, the original fault's red output and the whole
campaign output are retained in [the diagnostic tooling record](../evidence/tooling/speculated-arms-proof.txt).
It explicitly predates the source commit and does not replace hosted evidence.

## Recipe ownership correction

The recipe executes the twelve producer controls explicitly, once, before the
unchanged eight semantic controls. The semantic runner no longer invokes a
nested campaign. This makes both evidence owners visible to the canonical
recipe parser. Producer verdicts and counts follow the existing evidence
format, and class-qualified witness identifiers remain metadata. The parser
and its authority are unchanged. A thirteenth producer regression checks the
actual recipe's two literal invocations. Fresh twelve-control output combined
with the retained eight-control stream checks report ownership; this structural
check is not a new run of the semantic campaign or hosted evidence.


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.
