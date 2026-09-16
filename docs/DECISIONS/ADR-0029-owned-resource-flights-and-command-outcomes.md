# ADR-0029: owned resource flights and terminal command outcomes

Status: accepted for the local synchronous runtime repair, 2026-09-16.

## Context

The owner's requested continuation includes the unfinished resource repair.
Nine tests against master `49398dc` reproduce failures despite the existing
resource gates passing: fabricated duplicate-read results, failure reported as
success to a duplicate, concurrent command execution, obsolete cache publication,
publication after last-subscriber release, stranded ownership after panic,
private cache reuse by a public manifest, ignored deadlines, and clock overflow.
This repairs existing runtime contracts; it does not replace or close E9/E10-I.

## Decision

A request has a unique owned flight with a shared terminal outcome. Admission,
cache publication, invalidation, and removal are serialized by the runtime state
mutex. Blocking callbacks run outside it. Followers wait for the actual outcome;
only successful shared completion is called `Deduplicated`. Every invalidation or
last-subscriber release terminalizes the flight. An old callback cannot publish
or remove a replacement flight, even if it ignores cancellation.

Expose a cooperative cancellation token for adapters while preserving `fetch`
for synchronous callers. The whole-flight timeout includes retries and backoff
and uses the existing injected clock. Check it before attempts, after callbacks,
and while joining. A token signals cancellation/deadline; it does not forcibly
interrupt arbitrary synchronous code or undo external work. New work may start
once old publication authority is revoked, even if an uncooperative old callback
has not returned. At most one *authorized flight* is distinct from physically
stopping foreign I/O. The adapter owns that final obligation.

Require manifest/key resource agreement, matching privacy on cache reuse, and
identical active-flight policies before coalescing. This is not authentication:
callers must supply correctly scoped keys and trusted manifests. `Key` remains
local lookup identity; this patch does not redesign the canonical `EntryIdentity`.

Commands atomically reserve identity before invoking user code. All concurrent
callers share the same result. A callback that unwinds may already have committed
external work: record `OutcomeUnknown`, wake followers, preserve the original
panic for the owner, and never automatically re-execute that key. `try_command`
exposes this state; the legacy infallible `command` fails loudly instead of
returning an invented success. Same-thread recursive joins fail instead of
waiting on themselves. Callbacks and panic propagation never run under a state
or completion mutex.

## Limits and cost

This is process-local, blocking coordination, not durable exactly-once execution,
a tenant authenticator, a host/renderer integration proof, or a deadlock detector
for arbitrary cross-thread dependencies. Command records remain for the runtime
lifetime; eviction would change replay guarantees and is not silently introduced.
The injected clock's existing retry-time advancement is simulation behavior, not
a real-time scheduler. Clock advancement and backoff arithmetic saturate.
Joining polls the injected clock with a bounded Condvar wait; wall time does not
define policy expiry. Production adapters still require real cancellation,
scheduling, retention, and authority/operation/payload-bound durable identity.

## Primary API checks

- Rust Condvar requires predicate rechecking, including spurious wakeups:
  https://doc.rust-lang.org/std/sync/struct.Condvar.html
- Unwinding cleanup cannot recover a process-aborting panic:
  https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
- Resuming the original panic preserves failure rather than converting it to a
  successful application value: https://doc.rust-lang.org/std/panic/fn.resume_unwind.html
