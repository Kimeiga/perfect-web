# ADR-0016 — E2A-R is a thread-scoped runtime, and its results are behaviour, not guarantees

**Status:** Accepted
**Date:** 2026-08-06
**Milestone:** E2A-R
**Completes:** RQ-4, whose E2A-S half landed in `compiler/pw-core/src/scope.rs`

## Context

`docs/RISK_QUEUE.md` splits structured concurrency deliberately:

> A runtime task tree cannot make compile-fail fixtures fail at compilation.
> **E2A closes only when both halves pass.**

E2A-S is done and now runs on real `.pw` bodies. E2A-R — owner scopes,
cancellation propagation, cleanup ordering, leak detection, and results not
committing into dead scopes — is unstarted, and the risk queue is explicit that
the static rules **must never be described as covering it**.

The open question is what to build it *on*. Web workloads are asynchronous, so
the obvious answer is an async runtime. That answer is wrong for now: an async
executor is a large dependency and a large surface, E8 has not chosen the host
execution model yet, and building one here would decide E8's design as a side
effect of testing E2A's semantics.

## Decision

**Build the runtime on `std::thread::scope`. Add cancellation, ordered cleanup,
and the dead-scope commit rule on top. No async, no new dependencies.**

The five properties E2A-R owns are properties of *structure*, not of threads:

| property | where it comes from |
|---|---|
| a task cannot outlive its scope | `std::thread::scope`, reused rather than reimplemented |
| cancellation propagates parent → descendants | this crate |
| cleanup runs children-before-parents, LIFO within a scope | this crate |
| a result arriving after its scope died is discarded | this crate |
| no leaked task after a scope exits | asserted by a live-task counter returning to zero |

Reusing `std::thread::scope` for the first row is deliberate. The charter
prefers reuse, the guarantee is already sound, and re-deriving it would add risk
without adding evidence. What this crate must prove is the other four.

### What is deliberately not built

**No async executor.** E8 chooses the host execution model; deciding it here
would be deciding it by accident.

**No work stealing, no scheduler tuning, no timers.** None is load-bearing for
any of the five properties.

**No integration with `pw` source.** E2A-S already checks `.pw` bodies. The
runtime is exercised by Rust tests, because the property under test is runtime
behaviour and there is no code generator to drive it yet.

## What this is, and is not, evidence for

**Is evidence for:** that the semantics E2A-S enforces statically are
implementable, and behave as specified under real concurrency — including the
cases the static checker cannot see, such as a task that finishes *after* its
scope has already been torn down.

**Is not evidence for:**

- **anything static.** RQ-4's wording is the rule: these are behaviour tests.
  A passing runtime test does not make a program's misuse a compile error; that
  is E2A-S's job and it is separately evidenced.
- **the eventual async runtime.** The properties must be re-proved in whatever
  execution model E8 selects. This ADR's deletion condition is exactly that.
- **performance.** Nothing here is measured for throughput or latency.

## Consequences

- RQ-4 can close: both halves exist, each evidenced in its own register.
- A `runtime/` crate tree begins, as charter §12 anticipates. It holds one crate
  and no placeholder directories — §12 forbids empty directories that exist only
  to look complete.
- Cost: when E8 picks an execution model, these semantics move. The tests move
  with them and are the specification for the port, which is the point of
  writing them as behaviour tests rather than as unit tests of an implementation.

## Revisit when

E8 selects the host execution model, or a property here proves unimplementable
in that model — which would be a finding about the semantics, not about this
crate, and would send E2A-S back for revision.
