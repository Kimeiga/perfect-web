# ADR-0019 — The materializer's store is SQLite, and the outbox is a table in it

**Status:** Accepted
**Date:** 2026-08-06
**Milestone:** E6
**Depends on:** ADR-0018 (the manifest is data), ADR-0016 (a runtime is behaviour)

## Context

Charter §14 M6 task 2 says "add a transactional outbox in SQLite" and task 4
says "have commands emit events in the same database transaction as state
changes". Those two sentences are one requirement, and the requirement is the
whole point of the milestone.

An event published *after* a commit can be lost between the commit and the
publish, and then nothing regenerates — the failure this milestone exists to
remove, arriving through the door labelled "reliable messaging". An event
published *before* a commit can describe a change that then rolls back, and
something regenerates from state that never existed.

The only way out is for the event and the state change to be one atomic write.
That requires them to be in the same transactional store.

## Decision

**The state and the outbox live in one SQLite database, and a command writes
both inside one transaction.** The materializer reads committed rows and marks
them consumed in a second transaction.

```text
   command
      │  BEGIN
      ├─ UPDATE  cart …            the state change
      ├─ INSERT  outbox …          the event
      │  COMMIT                    ← both, or neither
      ▼
   materializer
      │  BEGIN
      ├─ SELECT  … WHERE consumed_at IS NULL ORDER BY id
      ├─ … regenerate …
      ├─ UPDATE  outbox SET consumed_at = …
      │  COMMIT
```

`rusqlite` 0.40.1, MIT, `features = ["bundled"]`. Verified on crates.io
2026-08-06 — the primary source, not a doc in this repository (charter §3.1).

`bundled` compiles SQLite from source into the binary. The alternative is
linking the system library, and on this project that is worse in a specific
way: the version would be whatever the host has, so a measurement recorded here
would be a measurement of one machine's macOS. `tools/versions.lock` exists to
stop exactly that.

## What this is not

**Not a claim that SQLite is the production store.** Charter §12 has a
`materializer/` directory and E8 has not chosen the host execution model. What
SQLite is here is a transactional store that runs in a test with no service to
start, which is what makes the failure-injection matrix (task 8) runnable at
all: a crash between the state write and the event write is a `panic!` between
two statements, not an orchestration exercise.

**Not a message queue.** The outbox is a table with an autoincrement id and a
`consumed_at`. Delivery is a `SELECT`. There is no broker, no retry topology
and no ordering guarantee beyond the id — and the materializer is written so it
does not need one, because charter §14 M6 gate item 2 requires duplicate events
to be harmless and task 8 requires out-of-order events to be survivable.

Those two properties are why the design can be this small. An idempotent
consumer that treats an event as *a reason to look at the graph* rather than as
*an instruction* does not need exactly-once delivery, and exactly-once delivery
is the expensive thing.

**Not the compiler's dependency.** `pw-core` does not know SQLite exists. The
materializer reads the serialized graph (`pw emit-graph`) the same way
`pw-resource` reads the serialized manifest, for the reason ADR-0018 gives: a
runtime that the compiler depends on is the only runtime there can ever be.

## Consequences

- One new workspace dependency, with a C compiler already required by the
  toolchain.
- Build time grows by the SQLite amalgamation, once per profile.
- The failure-injection tests can be ordinary `#[test]`s.
- If E8 chooses a different store, the materializer's logic moves and its tests
  move with it; the graph artifact and the event schema do not change, because
  neither mentions SQLite.
