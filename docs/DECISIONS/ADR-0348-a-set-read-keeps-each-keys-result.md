# ADR-0348: a set read keeps each key's result

Status: accepted under the owner's delegation of 2026-10-02, by the
integrator, 2026-10-15. Milestone: E14. Amends ADR-0340's pair-shape
implementation, preserving ADR-0337's list of keyed results.

## Finding

Combining scheduled orders with the bounded-read checker refuses
`lead_of`: an estimate was read per store. A set read repairs it, but the
checker rejects a keyed record holding the exact `Result<Estimate, Error>`
of its paired read because it only compares the success payload. Dropping
each error would lose the source operation's meaning.

## Decision

A set result record with its key may hold either the keyed operation's
exact result or the existing successful-value projection. Exact equality
includes both Result type arguments: a different error type still fails.
This does not change key matching or allow results of another type.

Scheduled estimates read the reader's chosen address, all store rows and
the session's estimate once. They return one keyed result per requested
store, preserving order, duplicates, coverage and declared failures. One
host failure fails the set operation. No estimate is read for an empty
list. The estimator's artificial latency is paid once per set operation.

## Acceptance

Regression controls require exact success/error types and a key, and retain
the previous successful-value form. The store's common counted-operation
suite checks the new operation at seed and fifty additional stores. Focused
memory/PostgreSQL tests compare keyed/single results and failure paths.
Core bounded tests passed 31/31; whole bounded controls killed 41/41,
including the exact-result control. PostgreSQL's four bounded tests passed;
the set-estimate count is three at both sizes. Whole statement controls
killed 8/8, including the hidden per-store coverage read. See the raw
[integration evidence](../evidence/E14/train10-scheduled-integration.txt).
Full assembled CI remains pending; train-10 is not landed.
