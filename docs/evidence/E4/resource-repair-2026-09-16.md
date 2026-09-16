# Resource-runtime repair: measured scope, 2026-09-16

## Baseline and reproduction

Base commit: `49398dc02be6f2c3ea1a28fdfb77bf66c15266b6`.
Base tree: `68cdbc3876f57f939a0acc6f96762973a46043ed`.
The recovered checkout's Git tree was matched to that exact tree before edits.
This repairs existing resource contracts under [ADR-0029](../../DECISIONS/ADR-0029-owned-resource-flights-and-command-outcomes.md).
It does not close the resolved-signature migration, E9-V1..V6, or E10-I.

Before implementation, nine added regression tests all failed against that base,
while its existing resource suite passed. The failures were behavior assertions,
not parser errors or missing APIs. The final regression file includes those nine
and fourteen additional adversarial/accepted-neighbor tests. The older 50-reader
gate now checks every returned value, and the navigation gate cancels a genuinely
active request instead of releasing subscribers after it has already finished.

## Observed results

| Command or test group | Local result |
|---|---|
| Nine original regressions on the unchanged runtime | 0 passed, 9 failed |
| `cargo test --locked -p pw-resource` groups in the workspace run | 56 passed, 0 failed |
| New concurrency regression target | 23 passed, 0 failed |
| `cargo test --locked --workspace` | 937 passed, 0 failed, 1 existing ignored test |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | exit 0 |
| `just evidence-gates` | 8 methods passed; these test recipe failure propagation |
| `just test-compile` | exit 0; accepted and store programs report no diagnostics |
| Census validator and Python suite | structural validation passed; 20 tests passed |
| Layout attribution Node suite | 20 tests passed; no browser timing benchmark run |

The 24 new tests are 23 runtime regressions plus one manifest-policy mutation
control, explaining the increase from the prior 913-test workspace result.
The manifest control generates a manifest from real `.pw` source: the same
three-second callback is rejected under a two-second timeout and accepted under
a five-second timeout. Conversion uses the existing test-side bridge; this is
not claimed as production host integration.

Local environment: Linux 6.18.44 x86_64, Rust 1.97.1
(`8bab26f4f`, 2026-07-14), Python 3.13.5, Node 22.16.0, just 1.58.0.
Dependencies came from the pinned toolchain and locked registry snapshot; no
manifest or lockfile was changed. Long checks ran as tracked local processes
with saved logs and final exit codes, all completed before this report.

The `engine`-feature Clippy check and current vulnerability audit are separate
CI requirements. The local results above do not substitute for this patch's
head-specific CI. Merge only after its Ubuntu x64, Ubuntu arm64, audit, and census
jobs pass. The PR records those actual run IDs after completion.

## Reproduce on the patched tree

```sh
cargo test --locked -p pw-resource
cargo test --locked --workspace
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
just ci
just audit
python3 research/failures/tools/validate.py
python3 -m unittest discover -s research/failures/tools -p 'test_*.py' -v
node --test spikes/layout-phase-scheduler/test/loaf.test.mjs
```

To reproduce the original nine failures, copy only the first nine tests and their
helpers from `concurrency_regressions.rs` into a separate worktree at the base
commit. Later tests deliberately use the repaired API and cannot be applied to
the old implementation unchanged. Channels order the tested schedules; wall-clock
watchdogs are test-failure guards, not the resource's semantic clock.

## What changed and what it establishes

Request admission reserves an owned flight before invoking a callback. Matching
readers share its terminal result. Cancellation and invalidation revoke publication
and wake followers; late work cannot republish data or remove a newer flight.
Callbacks can observe a cooperative cancellation token. Panicking loaders wake
followers, release ownership, and preserve the original panic.

Commands reserve identity atomically and share one process-local result. An
unwinding callback may already have committed work, so the retained result is
`OutcomeUnknown`, not permission to retry. Same-thread self-joins fail explicitly.
Manifest/key and public/private cache mismatches fail rather than reuse data.
Injected-clock deadlines include retry time, and clock/backoff arithmetic saturates.

## Census mapping and residual obligations

- C03/C04: late-result fencing and last-subscriber behavior are now exercised in
  this runtime. This does not establish all renderer/host lifecycle guarantees.
- C05/G01/G04: cancellation is not rollback; reservations and unknown outcomes
  have process-local controls. Durable effect/deduplication atomicity, principal
  and payload binding (G03), restart recovery, and provider reconciliation remain.
- C07: this runtime now respects one whole-flight timeout. Cross-service deadline
  propagation and queue/transport budgets remain separate obligations.
- J04: invalidation revokes an old flight's publication rights. This is not proof
  of distributed version ordering, complete dependency tracking, or causal reads.

The historical census statuses are not blanket-promoted to covered. Synchronous
foreign work cannot be forcibly interrupted by this implementation. A replacement
may run after revocation while an uncooperative old callback is still executing.
The existing injected-clock backoff advances logical time; it is not a real-time
scheduler. Command outcomes remain in memory for the runtime lifetime, and the
host must provide correctly scoped identities. No new data-retention, authentication,
HTTP adapter, database, browser, or production-deployment guarantee is claimed.

## Tested source identities

The following Git blob hashes identify the exact locally tested Rust files and
were compared with GitHub's uploaded blob responses before creating the commit.

| File | Git blob |
|---|---|
| `runtime/pw-resource/src/lib.rs` | `b31622150545dcf6d99d26e3c74a976f572bfbb5` |
| `runtime/pw-resource/tests/concurrency_regressions.rs` | `56ee0c6117ec87569ab5241f826708b67b03388f` |
| `runtime/pw-resource/tests/gate.rs` | `0daa9aaf456aa217095c6443d7ebb729b4a81450` |
| `runtime/pw-resource/tests/from_manifest.rs` | `a5dcfd8575ed5a94b97c2774c1fa80ee17cda18d` |
