# ADR-0116: Wasmtime 48.0.3 replaces the vulnerable 47.x pin

Status: accepted under the owner's 2026-10-01 instruction to close known gaps.
Date: 2026-10-01. Milestones: E8, E10.

## Context

The supply-chain CI gate began failing on 2026-10-01 because the RustSec
database now reports two vulnerabilities in the workspace's Wasmtime 47.0.4:

- RUSTSEC-2026-0315: fuel spent by some indirect calls and exception returns can
  be dropped, allowing fuel amplification.
- RUSTSEC-2026-0316: dynamic record lifting can allocate beyond the host-call
  fuel limit.

Fuel is part of Pleris's resource boundary. Ignoring either advisory would make
the host claim a budget that the engine version cannot reliably hold.

Wasmtime 48.0.3, released 2026-09-24, contains both fixes. Its MSRV remains
below this repository's Rust 1.97.1 pin.

## Decision

Move every live Wasmtime engine dependency in this repository to **48.0.3**:

- the workspace host and conformance engine;
- the standalone capability-host spike;
- both Cargo lockfiles;
- the bootstrapped Wasmtime CLI on macOS arm64, Linux x86_64 and Linux arm64.

Do not suppress either RustSec advisory. The bootstrap continues to pin a
SHA-256 for every supported release asset. Historical evidence files keep the
47.x version they actually measured; current status and executable reports use
48.0.3.

This is a security patch, not a change to ADR-0006's capability model or
ADR-0008's WASI 0.2 decision.

## Acceptance

- `cargo deny` reports no Wasmtime advisory.
- `just ci` passes on Linux x86_64 and Linux arm64 with the updated lockfile.
- `just bootstrap` verifies the pinned 48.0.3 CLI archive on both Linux CI
  architectures.
- the standalone spike's report names the crate version it actually builds.
- no ignore is added to `deny.toml`.

## Evidence scope

Existing E0, E8 and E10 measurements recorded under Wasmtime 47.x remain
historical measurements. They are not relabelled as 48.0.3 measurements. Any
performance claim sensitive to engine version must be re-recorded before being
presented as a current 48.0.3 number.
