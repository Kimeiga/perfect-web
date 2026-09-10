# ADR-0027: Failure census evidence and local resource completion

Status: Accepted for the bounded repair and research-record structure below.
Date: 2026-09-10.
Authorization: The project owner explicitly requested implementation, commit, push and merge in the research conversation.
Scope: Correct existing E4 runtime behavior and preserve research obligations. Do not advance E9/E10 gates or merge the incomplete resolved-type migration.

## Context

The Web Failure Census reviewed master at `0c2579d2bda9edb91a1ccb817b93f2ba0ab24f45`. Its findings are historical assessments of that snapshot, not executable proofs or a promise that every proposed language rule is accepted.

Source inspection of `runtime/pw-resource/src/lib.rs` found that a concurrent fetch returned an old cache value or an empty string rather than waiting for the active request. Command admission checked the completed-result map, released the lock, executed the callback and only then recorded the result. Concurrent duplicates could both execute. Invalidation and final-subscriber release did not prevent the old callback from publishing afterward. A panicking callback could leave state marked in flight.

`runtime/pw-resource/tests/gate.rs` checked execution counts but discarded concurrent fetch results, used sequential command duplicates, and released subscriptions after the request completed. The new regression corpus must observe results and control schedules, not merely count bookkeeping events.

## Decision: two connected research views

Keep one record per violated invariant, with stable census IDs. Attach separate evidence records identifying the affected system, mechanism, version or date scope, conditions, source and evidence strength. Distinguish an observed incident, a documented hazardous pattern, an independently reproduced defect, a bounded mitigation and an unevaluated system. Never turn an empty cell into a claim of immunity or treat a patched historical defect as a claim about every current release.

Cloudflare's September 12, 2025 dashboard/API incident belongs to G06 as a documented React `useEffect` object-dependency trigger, and to separate amplification classes. The report distinguishes the application trigger from Tenant Service deployment, authorization dependency and recovery load. It did not report a data-plane network outage. Sources:

- https://blog.cloudflare.com/deep-dive-into-cloudflares-sept-12-dashboard-and-api-outage/
- https://react.dev/reference/react/useEffect

This is not a framework ranking. Framework-specific records explain how an invariant can be violated; invariant records determine what Pleris must enforce. A mitigation is not equivalent to making the failure unrepresentable.

## Decision: one shared terminal result per admitted local operation

For the existing synchronous `pw-resource::Resources` prototype:

1. Admit a per-key operation atomically under the state lock, before invoking its callback.
2. Store a shared completion cell. Duplicate callers receive the same success or failure, not placeholder or expired cache data.
3. Execute user callbacks without holding either the global state lock or a completion-cell lock. Independent operations remain usable.
4. Fence publication using the identity of the admitted request. Invalidation and final-subscriber release terminate that generation. A later completion cannot overwrite a replacement or remove its in-flight record.
5. Wake waiters when an operation terminates. A loader panic releases the query slot. A command panic records an uncertain outcome and must not automatically rerun the mutation under that key. Preserve the original caller's panic.
6. Reject a synchronous same-thread wait on its own unfinished operation instead of deadlocking.
7. Do not reuse a cache entry or in-flight request under a contradictory public/private classification.
8. Saturate test-clock and backoff arithmetic rather than wrapping time or panicking at large values.

No new dependency, application annotation, compiler rule or generated code format is required for this repair. The existing manifest remains the compiler/runtime boundary (ADR-0018).

## Boundaries this repair does not cross

This is a local synchronous prototype, not the durable command protocol. It does not establish multi-process or crash-safe deduplication, payload binding, principal-scoped operation identities, revocation, durable reconciliation, bounded idempotency retention or the E10-I source-to-host path.

Invalidating a completion does not preempt an arbitrary synchronous callback, close a network connection or undo a committed command. Host adapters still owe cooperative cancellation and deadlines. `Manifest::timeout` remains an unresolved adapter obligation; it must not be described as enforced. Private/public classification agreement is not tenant isolation. Production identity remains `EntryIdentity`, not a new competing authority.

The synchronous API cannot solve arbitrary cross-thread dependency cycles. This repair detects the immediate same-thread cycle only. A hung callback with no cancellation/invalidation remains a liveness limitation. Memory, retry, deadline and distributed admission budgets remain separate work.

## Evidence and acceptance

Add regression tests before the implementation where practical. Use channels to hold a callback open and observable admission events to schedule duplicates. Wall-clock timeouts are deadlock guards, not the property being tested. Assert every caller's result, actual mutation counts, stale-publication rejection, replacement preservation, panic cleanup, privacy mismatch rejection and independent-key progress.

Record exact test commands, commit identity and actual results. Tests authored or waiting for CI are not passing tests. Do not change historical census statuses to covered merely because one local witness passes. Link implementation evidence separately.

## Consequences

Concurrent duplicate calls may now block until their shared result is available; returning immediately with an unrelated string was incorrect. Callback panics retain explicit failure rather than enabling silent mutation replay. This adds one small synchronized completion object per active query or retained command. Command retention and synchronous liveness are not solved by this object.

The research catalogue stays deduplicated and framework-searchable. The rejected/accepted corpus must retain positive controls and semantic-cause assertions. Larger P0 proposals require their own ADRs and source-to-runtime witnesses before implementation claims.
