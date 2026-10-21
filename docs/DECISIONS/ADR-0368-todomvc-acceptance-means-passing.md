# ADR-0368: TodoMVC acceptance requires every official test to pass

Status: accepted by the integrator under the owner's delegation of
2026-10-02, on the explicit takeover request, 2026-10-15. Milestone: E14.

## Amendment: producer transcripts and campaign verdicts, 2026-10-21

The Cypress producer controls intentionally test words such as `SURVIVED`.
Their native assertion transcripts therefore contain those words even when
the control was killed by a valid assertion. Verify 39538607183 recorded all
54 producer controls killed but the generic evidence guard correctly refused
the resulting ambiguous evidence stream. No summary or fetch guard is relaxed.

The campaign now retains each native run's stdout, stderr, invocation,
expected inventory, structured receipt and validation record under the
already-uploaded `spikes/own-renderer/kept-traces` tree, including invalid
runs. It also retains the combined diagnostic transcript. Copies must match
byte-for-byte and fit the existing shared artifact budget; missing or skipped
retention refuses the campaign. The evidence stream prints the complete
inventory, native and validated status, structured outcomes, actual retained
path and SHA-256 per file. Its own `KILLED`, `SURVIVED` and `REFUSED` verdicts
remain plain and retain their existing authority.

This is an explicit bounded exception to the older retention comments that
exclude intentional mutation output: these small native producer receipts
are required diagnostic evidence, rather than browser traces. The existing
32 MiB shared retention budget and uploader are unchanged. Raw assertion
operands are not themselves campaign verdicts and are never rewritten.

All prior 54 controls remain. A 55th reintroduces raw transcript printing:
the full 51-test native producer then reports exactly two body assertion
failures, no body errors or incomplete results; restored 51/51 passes. The
regression verifies copied raw bytes and hashes, proves an incidental
assertion operand does not flag canonical `ci_summary`, and proves a real
plain `SURVIVED` verdict still does. It also covers invalid-run retention
and refusal when retention is incomplete. Whole 55-control and full official
29-case acceptance reruns remain hosted obligations. Historical and focused
proof are [retained separately](../diagnostics/train-12/todomvc-witnesses-and-report.txt)
from current E14 evidence.

## Amendment: explicit Cypress compatibility execution, 2026-10-20

The integrator accepts a versioned execution copy under the owner's current
instruction to continue the work. The seven vendored upstream blobs remain
byte-for-byte unchanged. Execution on the pinned Cypress 15.21.1 applies
exactly two identified compatibility edits: invoke the spy's `resetHistory`
method, and finish `createTodo` with a fresh query after retaining its
existing logging callback. The latter lets the persistence alias query the
new document after reload. No application/runtime behavior, test case,
storage assertion, persistence assertion or known-issue table is changed.

Every report must identify this as **official TodoMVC cases with pinned
Cypress compatibility patch v1**. It is not unchanged upstream execution.
The source hashes, runner pin, unique patch anchors, output hashes and
staged manifest are checked before launch, as specified by
[ADR-0377](ADR-0377-todomvc-cypress-compatibility-copy.md). The runner must own and reap
its host; another process answering the chosen port is not readiness proof.

Acceptance still requires all 29 cases to execute and pass, no pending,
failed or skipped cases, and native Cypress status zero. Crashes, timeouts,
missing results and source drift fail the gate. Historical runs remain
failed, including the unchanged 24/29 run in verify 39526960864. A passing
compatibility diagnostic does not replace the complete recipe or its
negative controls. The server-backed version and old unchanged-suite
requirements described below are the historical decision; this amendment
supersedes their execution method and does not retroactively change results.

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

## Numbered on train-12 (2026-10-20)

- **Numbered ADR-0368** (the integrator's ruling of 2026-10-20: a train is
  numbered before it is pushed, after master's ADR-0363, in train order),
  merged from `a8bab6a2` (`track/todomvc`, merged inside
  `track/device-store`). `ADR-XXXX` reads `ADR-0368` in this file alone, as
  in this file's name and title.
