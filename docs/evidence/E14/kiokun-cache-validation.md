# Kiokun cache publication and lifetime: final validation

Base: `7ec6a5342acb27a4c5b498d365e88fb3a6b9668a`, branch `track/kiokun-cache`. The cache correction was
validated uncommitted, then committed only after the complete gates passed.

## Actual results

- Unchanged algorithm plus private scheduling witnesses: **0/5**, all built
  assertion failures. Same/different digest live writers return SQLite disk
  I/O error; delayed same-digest publisher loses its stage; new version
  removes old reader pathname while its existing Unix query still succeeds;
  failed malformed input leaks its own stage.
- `just e14-kiokun-cache`: **10/10** private tests, then **10/10** mutants
  killed through built assertions. No empty/missing test result or build
  error is counted as an assertion kill.
- `python3 scripts/kiokun_word_mutations.py`: complete affected script,
  baseline **56/0**, **138/138** built assertion kills; no survivors, missing
  anchors, compile-error kills or process stops. These are two whole scripts,
  **148 controls total**; the baseline/test sets overlap and are not summed.
- Default `just ci`: **exit 0**, `ci: OK`, complete-log failure scan empty.
  All ten private cache tests individually pass inside this run. Its server
  summary is `test result: ok. 576 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 618.78s`.
- Formatting, both clippy commands, case checks, all **2,923 anchors in 265
  scripts**, complete workspace tests, complete `test-compile`, and
  `mutant-programs` pass in the default CI dependency chain.
- The source/config manifest was frozen after the own recipe. All
  **1,855** hashes match after the affected word owner restores and again
  after full CI. Manifest SHA256:
  `9eab5253051627eaa29e5e868dcbce91a39f93bc8a55ada1d0b9a64e49070e86`. It is retained under
  `work/` for the integrator; the source hashes below are also in raw CI.

started: 2026-10-16T08:35:17Z
finished: 2026-10-16T09:04:44Z

## Commands and environment

Every build uses its own `/home/haki/code/pw-wt/kiokun-cache/target`,
`CARGO_BUILD_JOBS=4`, `RUST_TEST_THREADS=4`, and offline Cargo/npm after
quietly sourcing `/home/haki/pw-setup/env.sh`. The mutation campaigns are
cargo unit tests, not browser launches. The ordinary unit-server factory
uses the repository sample; external/oracle measurements remain ignored.
Direct cache tests use private synthetic CSVs above 6 MiB and private cache
roots. The final CI child explicitly unsets `KIOKUN_DATA`/`KIOKUN_APP`
after setup. No global environment is changed and no external publisher is
stopped. No browser host-isolation result is claimed by this phase.

The own recipe runs the ten-test cache module before the complete new
mutation script. The affected word script runs all 138 controls. Then,
after their natural restoration boundary, the driver verifies the frozen
source/config manifest, runs fmt/anchors, runs default `just ci` without
skips or overrides, and verifies the same manifest again.

## Publication scope

Actual target: Rust 1.97.1, Linux 6.18.54.1-microsoft-standard-WSL2, ext4.
The std-only same-filesystem probe published an absent destination and
refused a nonempty destination with OS39, preserving winner inode/content
and the loser stage. Real SQLite barrier tests verify the cache behavior.
Cooperating publishers install only completed nonempty digest directories;
existing reuse candidates and nonempty concurrent winners are validated.
Arbitrary out-of-protocol concurrent empty-directory creation is not
certified. No crash durability or additional-platform certification is
claimed. Failed/lost builders clean only their own stage; killed stages
and old published versions are retained and ignored. ADR0314's automatic
stale deletion policy is explicitly superseded by ADR-0358-kiokun-cache.

## Raw evidence SHA256

- `docs/evidence/kiokun-cache-before.txt`: `63e0910802e80512ce3db0a7037211431a779cca55569b7c380a1e01981a6a1b`
- `docs/evidence/E14/kiokun-cache.txt`: `080fcef5949441c14c2f5e16f28c1958b5dc05d8629af68b1f1905d51289c3da`
- `docs/evidence/E14/kiokun-cache-word-mutants.txt`: `ce4dd9cf40822ec209b0f35cae1e93812ef6334d63d0981e5867cbd1eb9d3b71`
- `docs/evidence/E14/kiokun-cache-ci.txt`: `75470d633f2e69fd6940ff097a5c44b12847700a74fe300057d73c59e6f15c59`

## Tested source/config SHA256

- `spikes/own-renderer/server/src/kiokun/search.rs`: `853cca07d37fb12561999f2cc628c6926c84c84f581b8cc72aa34c9d8b767cf4`
- `spikes/own-renderer/server/src/kiokun/search/cache_tests.rs`: `f31eb07da47453ef3a1d4e7c72afd66ee49369ad1a146cf93af007ff1cf044dd`
- `scripts/kiokun_cache_mutations.py`: `d8d50a4444281c351ad0ab848653f34230756489015cfc15e3a5804884f3e2b2`
- `just/kiokun-cache.just`: `127d397f3dab198ab76baee915c244edb3e110b07e7158ef515e2d45e0b346b2`
- `justfile`: `45d8bf9c17686641b2260312496a59001b953503fd55d657c2ff0230b3b4179f`
