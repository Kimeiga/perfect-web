# Pleris Rust accelerator experiments

This is a deliberately separate build/test experiment; no CI or runtime
defaults are changed. The GitHub workflow tests Ubuntu x64 and ARM with
Rust 1.97.1. Each comparison uses pw-core, pw-table-memory and pw-host.

- Test runner: prebuild, compare exact test-name multisets (including duplicate
  names across binaries), preserve Cargo doctests, then alternate two runs
  of Cargo and cargo-nextest 0.9.149.
- Linker: compare runner default and distro-provided mold, with two fresh
  Cargo targets per variant in ABBA order, plus no-op incremental checks.
- Compiler cache: compare fresh Cargo-target builds with no wrapper against
  sccache 0.18.0, including a fresh sccache compile and rebuild after clearing
  Cargo artifacts but preserving the cache.

Every command records status, wall time, GNU time maximum RSS, free disk,
versions, and a full log. JSON counters from sccache are preserved verbatim.
No test failure, timeout, inventory mismatch, or skipped doctest is a valid
speedup. The experiment uses fresh Cargo targets, NOT a cold OS file cache.
Results are independent by architecture; tool installation overhead is not
part of timed trials. Sccache does not yet use a cross-run remote cache.

Local parser checks: python3 -m unittest discover -s tools/perf -p test_*.py -v

Relevant documentation:
https://nexte.st/docs/running/
https://github.com/rui314/mold
https://github.com/mozilla/sccache
