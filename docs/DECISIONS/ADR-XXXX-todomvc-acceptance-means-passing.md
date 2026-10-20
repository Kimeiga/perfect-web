# ADR-XXXX: TodoMVC acceptance requires every official test to pass

Status: accepted by the integrator under the owner's delegation of
2026-10-02, on the explicit takeover request, 2026-10-15. Milestone: E14.

## Finding

The TodoMVC track ran the official Cypress files unchanged but its custom
judge returned success for a known failing run (3/29 passing, failed tests
and hooks, most tests not run). It even rejected a test passing if it asked
localStorage. Explaining an observed mismatch does not satisfy acceptance.

## Decision

Acceptance requires all 29 official tests to execute and pass, with zero
pending, failed or skipped tests and Cypress exit status zero. Missing
status or output fails closed. Passing a storage requirement is welcome,
not an error. The separate diagnostic remains explicitly non-acceptance;
its known persistence skip and race explain observations only.

The existing server-backed application does not satisfy the unchanged
suite's storage assumptions, and its stream timing race remains open.
These are unmet obligations; no adapter-specific expected-failure rule
turns them into a passing application witness. No upstream file is edited.

## Validation

Recorded historical failing runs must now exit 1. Synthetic complete runs
are positive parser/gate controls only, not evidence the application passed.
The corrected gate passes 41 unit tests and kills all 42 mutation controls,
with no memory bound stops. Evidence harness: 201 passed and one
intentional skip (202 total). Full local `just ci` passed: 2913 Rust tests passed and
25 were ignored across 337 result sections; mutant-programs 279, 13 named refusals,
zero failing.

An actual run of all seven unchanged upstream files on Cypress 15.21.1
reports 3/29 passing, two failed tests, one failed hook, and 23 not run.
Cypress exits 3 and the acceptance gate now exits 1. Missing localStorage
semantics and the subsequent hook failure remain application obligations.
Raw gate verdict, whole mutation output, commands and input hashes are in
`docs/evidence/E14/todomvc-acceptance-gate.txt`. The application remains
unfinished until its unchanged suite passes.
