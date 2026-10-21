# ADR-0374: only a current keyed read retries after newer frames

Status: proposed for integration. Date: 2026-10-20. Milestone: E14.

## Context

A keyed read samples its document's sent-frame count before fetching. Under
ADR-0224 it reads again if that count changed before it takes the session's
hold, so it cannot overwrite a committed change with an older value.

A newer keyed request also sends frames. In the shared-flight cancellation
witness, session a's Hot request begins a flight, session b's command joins,
and a moves to Cold. Cold replaces Hot's subscription and sends its frames.
Hot's shared flight completes for b. The superseded Hot request then sees
the changed frame count and retries before checking whether it is still
current. Its hold is gone; freshness zero starts an unnecessary flight that
may be cancelled before its source finishes.

The earlier test comment attributed that second flight to b's page. The
trace instead follows a's obsolete retry. Render merging and component-name
migration do not change this path.

## Decision

After fetching and taking the session's hold, first check whether the request
is still the one the document awaits. A superseded or released request
returns `Superseded` before frame-count revalidation or further binding reads.
A current request keeps ADR-0224's existing retry and bounded last attempt.

## Evidence

The existing gated shared-flight witness asserts exactly one new Hot flight
while preserving the shared flight's cancellation protection, both pages'
final values and absence of a reload. The existing longer-read witness
requires a commit between fetch and apply to appear in the final page.

`just e14-keyed-retry` runs both witnesses and two source controls: omit the
superseded check; omit current-read revalidation. A kill requires a complete
Cargo result with both witnesses accounted for and a named equality assertion.
Build errors, stops, missing tests, gate timeouts and setup panics are invalid.
Before the guard, the gated witness completes and fails its flight-count
assertion: two Hot flights instead of one. With the guard, six targeted tests
pass: shared flight, current-read revalidation, cancel, supersede, keep and
browser departure. Whole mutation and CI results are pending.

## Limits

This changes only the keyed read's apply/retry boundary. Query freshness,
shared-flight cancellation, command ordering and cross-session invalidation
retain their existing policies. It does not claim general race freedom.


## Landing (2026-10-21)

Landed on `master` with train-12 at `e7664d2eb01aa8c6e91df594bd7270a84a8bf8de`
after exact-commit CI ([39584219053](https://github.com/studiokimeiga/perfect-web/actions/runs/39584219053))
and automatic track verification ([39575516650](https://github.com/studiokimeiga/perfect-web/actions/runs/39575516650)).
Dated statements above that call landing or combined gates pending describe
the candidate before it landed and are kept as written. The
[acceptance record](../diagnostics/train-12/acceptance-record-2026-10-21.md)
states what the landing's evidence does and does not prove.
