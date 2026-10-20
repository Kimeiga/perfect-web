# ADR-0363: Wasmtime 48.0.7, for RUSTSEC-2026-0378 and 0379

Status: accepted under the owner's delegation of 2026-10-02, the downloads
approved by the owner on 2026-10-20 (relayed by the integrator): the
release's 28 crates from crates.io, and its CLI archives from GitHub. Date:
2026-10-20. Milestones: E8, E14.

## Context

- **GitHub's `ci` fails its third job on every branch since RustSec's
  advisories of 2026-10-19.** Both build-and-test jobs pass; "licenses and
  advisories" (`just audit`: `cargo deny check`) ends "advisories FAILED,
  bans ok, licenses ok, sources ok" on two advisories against the pinned
  Wasmtime 48.0.6: run 39409729146 (branch `wip/train11-ci-check`) and
  39417885419 (`track/train-11`), each at d8f383d0. Nothing the train
  changed causes it; every branch's job fails the same way.
- **The two advisories**, read from RustSec's files in `advisory-db` and
  from the GitHub advisories they point to:
  - **[RUSTSEC-2026-0378](https://rustsec.org/advisories/RUSTSEC-2026-0378.html)**
    (GHSA-9pvw-hwfg-92h5, medium): Wasmtime may panic when the host lowers
    a component-model `stream` or `future` it created with one payload type
    into a guest that expects another. Patched `>= 48.0.7, < 49.0.0` and
    `>= 49.0.4`; unaffected `< 46.0.0`.
  - **[RUSTSEC-2026-0379](https://rustsec.org/advisories/RUSTSEC-2026-0379.html)**
    (GHSA-mghf-wjfp-p5xm, critical): with the Cranelift backend on an
    aarch64 target, a `br_table` with more than about 256k targets is
    compiled to an incorrect conditional branch; the advisory: "Any
    Wasmtime engine running on aarch64 with default settings can be forced
    to jump to arbitrary code by a user-controlled Wasm module". Patched
    `>= 36.0.19, < 37.0.0`, `>= 48.0.7, < 49.0.0` and `>= 49.0.4`.

## Research

- **What the host enables and does** is as ADR-0335 read it:
  `engine_config()` turns the component model on and its async, GC and
  exceptions off, and leaves the compiler Wasmtime's default, Cranelift
  (nothing here selects Winch or Pulley). Each call is a synchronous
  `Func::call` on a fresh `Store`. No Rust source, manifest, script or
  recipe here names `StreamReader`, `FutureReader`, `call_async` or
  `run_concurrent`.
- **0378, against that: not reachable.** It needs an embedder that creates
  streams or futures (`StreamReader::new`, `FutureReader::new`) and passes
  them through guests; the host creates none, and a component that uses a
  stream or a future needs the component model's async, which the host's
  engine refuses when it loads one
  (`engine_proposals::a_component_using_the_async_abi_is_refused`).
- **0379, against that: reachable on an aarch64 host, by a hostile
  component.** The host compiles with Cranelift on aarch64 on the Mac and on
  CI's `ubuntu-24.04-arm` runner. The compiler's backend emits no `br_table`
  (`compiler/pw-core/src/backend`: none; its branches are `br`, `br_if` and
  `if`), so no component Pleris emits reaches it; a replaced build artifact
  could (ADR-0244). The version closes it.
- **The release**
  ([v48.0.7](https://github.com/bytecodealliance/wasmtime/releases/tag/v48.0.7),
  2026-10-19 19:20Z; on crates.io at 19:16Z, not yanked): a patch on the
  pinned line, as ADR-0116, ADR-0244 and ADR-0335 chose. 49.0.4 fixes both
  as well, but 49 is a major release and its own decision. The release's
  assets carry SHA-256 digests on GitHub, from which the CLI is pinned.

## Decision

1. **Wasmtime 48.0.7**, every live pin: the workspace (`wasmtime`,
   `wasmtime-wasi`) and its lockfile, `pw-host`, `pw-conformance`'s tests,
   the standalone spike, its lockfile and its printed labels, and the
   bootstrapped CLI on macOS arm64, Linux x86_64 and Linux arm64
   (`scripts/bootstrap.sh`). Cargo moved the release's own crates in each
   lockfile and nothing else: 13 `cranelift-*` (0.135.6 to 0.135.7) and 15
   at 48.0.6 to 48.0.7 (`wasmtime`, `wasmtime-environ`, 11
   `wasmtime-internal-*`, `pulley-interpreter`, `pulley-macros`).
   `tools/versions.lock` records what the CLI prints,
   `wasmtime 48.0.7 (c6846e13f 2026-10-19)`, which `just doctor` compares.
2. **The engine's configuration stays** as ADR-0244 and ADR-0335 left it.
3. **No advisory is ignored in `deny.toml`**: a patch on the pinned line
   exists for both (ADR-0116).

## Acceptance

- **`cargo build --workspace --locked`** downloaded the 28 approved crates
  and nothing else, and builds. The standalone spike's host builds with
  `cargo build --release --locked --offline`.
- **`bash scripts/bootstrap.sh`**, Linux x86_64, here: the archive's
  SHA-256 is its pinned digest (2192e2b5...), and the installed CLI prints
  `wasmtime 48.0.7 (c6846e13f 2026-10-19)`. The three digests are GitHub's,
  for `wasmtime-v48.0.7-{aarch64-macos,x86_64-linux,aarch64-linux}.tar.xz`;
  CI's two Linux jobs run `just bootstrap` and check the Linux pair.
- **`runtime/pw-host/tests/engine_proposals.rs`, 5 tests** (the `engine`
  feature), pass on 48.0.7: the proposals stay refused when a component
  loads.
- **The advisory ranges**: 48.0.6 falls inside both advisories, 48.0.7
  inside neither (their `patched` ranges, above).
- **`just ci`** passes on the commit that carries this, chained to its push
  (the train's record, `tracks/train-11.md`, has its figures); **CI's three
  jobs** on that push, "licenses and advisories" among them, are its check.

## Not claimed

- **49.x.**
- **`cargo deny` here**: not installed on this machine; CI's job runs it.
- **The spike's guests and `spikes/wasmtime-component/run.sh`**: not run, as
  ADR-0335 ruled; `docs/evidence/E0/spike-wasmtime-component.txt` stays a
  record of what it measured then.
- **That 0379 cannot be reached**: on an aarch64 host a hostile component
  could; the version closes it, and only components the compiler emitted
  are ever loaded.
- **The macOS arm64 digest** has GitHub's word alone until the next `just
  bootstrap` on the Mac checks it.

## Numbered on train-11 (2026-10-20)

- **Numbered ADR-0363** on the train that carries it (the integrator's
  ruling of 2026-10-20: a train is numbered before it is pushed), after
  ADR-0362. It is named in the comments of `scripts/bootstrap.sh` and
  `tools/versions.lock`, and in `docs/vision/capabilities.md`.
