# x64 pw-core full-coverage integration sharding

The earlier x64 baseline ran `cargo test --locked -p pw-core --no-fail-fast`
sequentially in 24.3 minutes. In experiment run 40135659315, four
integration-binary partitions plus a separate library/doctest job passed
**exactly the same 1,812 tests**, with two intentionally ignored,
238 distinct integration binaries, and no failures. The critical path was
about 7.7 minutes (~3.2x faster); additional runner-minutes are a tradeoff.

New targets are discovered from Cargo metadata and checked against direct
tests/*.rs filenames; each target joins exactly one of four shards, with
the expected runtimes based on historical baseline job 120972413421.
Unknown/malformed metadata or weights fail closed. Each binary runs all
its normal tests, unchanged, with the same compiler, RUSTFLAGS and debug
profile. Unit and doctests are executed separately without filters.

The required x64 status is still `ubuntu-24.04`, which depends on both
the original non-unit matrix (Koka, lint and mutation controls) and the
complete native test matrix. Every skipped, cancelled or failed shard
makes that aggregate fail. The x64 cache saver is reassigned to exactly
one group, pw-core-int-0, preserving shared-key x64-native-tests.

Do not interpret passing CI as proof every runtime/perf issue is resolved.
Evaluate subsequent end-to-end wall time and runner-minute costs after
merging, and preserve the original parity baseline and rollback path.

Evidence: https://github.com/studiokimeiga/perfect-web/actions/runs/40135659315
Original: https://github.com/studiokimeiga/perfect-web/actions/runs/40127610536
