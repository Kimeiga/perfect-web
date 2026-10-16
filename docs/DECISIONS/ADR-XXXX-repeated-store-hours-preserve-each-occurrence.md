# ADR-XXXX: a repeated store keeps all of its opening hours

Status: accepted under the owner's delegation of 2026-10-02, by the
integrator before the production correction. Date: 2026-10-16. Builds on
ADR-0328's hours contract and ADR-0336's existing set read.

The legacy `hours#for-stores` contract preserves the caller's order and
duplicates and omits missing stores. This is distinct from generated
table primitives' canonical deduplicated ordering. No cost admission or
source promise changes follow from this correctness repair.

## Actual defect and causal evidence

On committed train-9 base `e2a4acd960c88144e3b40e7a6e9d9b8bd774ee1a`, a
new test uses the production `layer_for_tests` factory, `DataLayer::begin`,
the actual host hours write/commit/publication, and the actual public
`store:data/hours#for-stores` primitive. Both stores carry weekly and
special periods, with distinct cut-offs. Requests include reordered keys,
duplicates and a miss. The same expected full values pass in memory and
fail on PostgreSQL with native Cargo101 and the named equality assertion.
The second and later occurrence of a store has empty weekly/special lists.

The exact PostgreSQL raw SHA256 is
`05714504fb49e54e0c809cea7a55a1748eda4b38215fb99454801802f573df22`;
memory raw is
`2b5cda644797c9fd1cc1a24e77d4d9b2e5882ba5cd123af23c15a21e69c00ee0`.
The complete causal record, actual inventory/native statuses and all input
hashes are retained at `/tmp/legacy-hours-duplicates-causal-v1` before this
edit. Cargo results were validated by unchanged canonical parser a177209c.
The expected result includes the existing empty `name` field from `hours::of`;
that fixture assembly was corrected by source inspection before any test ran.
No failed fixture-assembly run is presented as a product failure.

## Decision

`pg_hours_of` gathers each store's periods in local maps, then reconstructs
the requested output. Its `remove` calls consume those periods for the first
occurrence. Replace only those two destructive reads with cloned lookups.
Keep the three existing set queries, their SQL/order, all snapshot/read-only
boundaries, output order and duplicate/missing behavior. No extra database
requests, cache, deduplication or app-name branch is introduced.

The isolated branch changes no running assembly or frozen D2 source.
Request prefixes of3/17 plus duplicate/miss tails are a correctness witness,
not a full two-data-size cost-conformance gate. Memory/PG provider-wide
OneCall admission, strict wire costs and the shared D2 suite remain separate.

## Required validation, still pending

Rerun this exact regression on memory and real PostgreSQL, then existing
hours suites on both, the applicable complete mutation suites, formatting,
global mutation anchors and fresh default CI. Require real named assertion
failures for controls and exact restored inputs. Preserve the original RED.
No push, merge, full-CI or milestone-completion claim is made by this record.

## Mutation evidence correction, accepted before consumer changes

After the production correction, the exact regression and complete14-test
hours suite pass on each real provider (30 total focused test executions),
with every input byte/path unchanged. Raw evidence is retained in
`/tmp/legacy-hours-duplicates-focused-v1`.

The existing hours mutation consumer still converts a timed-out command
to a fabricated test failure and can count contradictory/empty/build-error
outputs. A private direct-function probe of that exact source uses only
mocked results/stopped flags and temporary fixture files; no Cargo child,
project mutation, signal or process stop occurs. Four unittest methods
produce seven assertion failures in invalid/native-consistency and
restoration/no-later-control checks; the valid-count check passes. Raw
SHA256d34ea2371be831a218b52605c83f83f2cf35ac9998ac40afc8e0141599dbea27
is retained in `/tmp/legacy-hours-consumer-causal-v1` with the exact source.
This establishes an actual consumer classification defect under controlled
inputs, not a claim that a historical real run experienced a timeout/stop.

Use the existing unchanged canonical Cargo parser a177209c, as the bounded
statements consumer already does. Observe the existing watcher.stopped flag
at baseline/control boundaries. Invalid/build/stopped results are REFUSED,
restore the current input and return nonzero before scheduling another
control. Leave command arguments, process construction, timeout, memory
limits, signaling and cleanup mechanics unchanged. Keep all17 original
literal mutation tuples intact and add two operative controls reverting
the weekly and special non-destructive lookups separately. The real new
PostgreSQL regression must detect each through its original assertion.

Adopt the same four source-authoritative consumer tests into the existing
Python test suite. Then run complete updated hours19 and complete bounded
statements12 (including their existing payment/write-order controls),
capturing actual native results and assertion provenance, with exact source
restoration; follow with fresh default CI. No earlier partial/failed result
is upgraded by these pending gates. No cost/provider promise changes.

## Current-source validation closed (2026-10-16)

The production correction, current19 hours controls, complete12 bounded
statements controls, and recipe failure-propagation repair now have fresh
closed evidence. `just e14-store-hours` passed36 actual browser tests over
Chromium, Firefox and WebKit; every current19 control failed at a checked
original assertion. The separate full12 bounded-control run also passed
with original named assertion witnesses and its reviewed helper call chains.
The sources were restored after each whole, with all2745 source paths and
staged semantic bindings unchanged. Ordinary errors/stops earned no kills.

Fresh `just ci` returned native0/ci:OK on execution snapshot85c5c3c3:
2955 Rust tests passed,25 ignored; Python190 run,189 passed,1 skipped.
`docs/evidence/E14/repeated-store-hours-ci.txt` is its unchanged raw output;
`repeated-store-hours-gates.json` binds the source manifest, exact closed
proof/recovery hashes and qualifications. Reproduce with the three commands
listed in STATUS using provisioned PostgreSQL and the pinned local tools.

These result records were added after that exact execution. They change
documentation/evidence only; the checked executable, test and recipe bytes
remain identical. This closes the local repair gates, not remote verification,
final train assembly, D2/provider-wide OneCall admission or wire-cost claims.
Earlier incomplete/failed and51984-source evidence remains historical.
