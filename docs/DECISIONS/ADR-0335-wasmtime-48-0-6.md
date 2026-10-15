# ADR-0335: Wasmtime 48.0.6, for RUSTSEC-2026-0353 to 0364

Status: accepted under the owner's delegation of 2026-10-02, the downloads
approved by the owner 2026-10-14: the release's 28 crates from crates.io,
and its CLI archives from GitHub. Date: 2026-10-14. Milestones: E8, E14.

## Context

- **GitHub's `ci` fails its third job on each push since 21:33 UTC on
  2026-10-14.** Both build-and-test jobs pass; "licenses and advisories"
  (`just audit`: `cargo deny check`, then the Node audit) fails on eleven
  RustSec advisories against the pinned Wasmtime 48.0.5. Run 38641520286
  (branch `parked/train-7-docs`) ends "advisories FAILED, bans ok, licenses
  ok, sources ok". The last green run was `track/train-7`'s, 20:07 UTC; the
  first red, `track/bounded-statements`', 21:33; four branches were red by
  21:36, and `track/pc-setup`'s run 38659086894 failed it too. The Node
  audit, behind `cargo deny`, did not run.
- **RustSec published twelve advisories against `wasmtime` that day**,
  RUSTSEC-2026-0353 to 0364. Eleven are patched in `>=48.0.6, <49.0.0` and
  `>=49.0.3`, and the job names those eleven. RUSTSEC-2026-0360 is not
  against 48.x: its advisory says `unaffected = ["< 49.0.0"]` and patched
  `>=49.0.3` alone, because 49.0.0's refactoring introduced it.
- **The release fixes three more, in crates nothing here uses.**
  RUSTSEC-2026-0350 and 0351 are in `wasmtime-wasi-http`, and 0352 in
  `wasmtime-wasi`. The workspace declares `wasmtime-wasi` and no crate
  depends on it: neither lockfile has it, as ADR-0244 found.

## Research

- **What the host enables and does.** The engine is
  `engine::engine_config()` (ADR-0244): the component model on;
  component-model async, GC and exceptions off; the rest Wasmtime 48's
  defaults, function references among them. The use: a fresh `Store` for
  each call, a fuel budget and a memory limiter, host functions defined with
  `LinkerInstance::func_new`, the export called with the synchronous
  `Func::call`.
  No Rust source, manifest, shell script or recipe here mentions
  `call_async`, `instantiate_async`, `run_concurrent`, `StreamReader`,
  `FutureReader`, `async_support`, `wasmtime_wasi` or
  `set_max_component_handles`. The components are only what the compiler
  emits: a core module (`backend/wasm.rs`), wrapped by `wit-component`, in a
  WIT world with no `resource`, `stream`, `future` or `async` (those words
  are reserved, and escaped when a program uses one, `wit.rs`); the only
  reference type the module uses is a plain `funcref` table's. The host
  loads nothing else, so a hostile component first needs a replaced build
  artifact, as in ADR-0244. The standalone spike
  (`spikes/wasmtime-component/host`) makes its engine with Wasmtime's
  defaults, for the two guests it builds itself, and ships nothing.
- **The twelve, against that.** Each is read from RustSec's file in
  `advisory-db` and from the GitHub advisory it points to (GHSA; GitHub rates
  all medium but 0364, low). "Not reachable" means a switch the host has
  off, or an API it never calls, stands between a component and the bug.
  - **[0353](https://rustsec.org/advisories/RUSTSEC-2026-0353.html)**
    (GHSA-m6w2-jwq2-cv2c): `Func::call` on an async-lifted export panics.
    Not reachable: such an export needs the component model's async, which
    the host's engine refuses when a component loads.
  - **[0354](https://rustsec.org/advisories/RUSTSEC-2026-0354.html)**
    (GHSA-3r89-v5mq-j5xf): dropping the future of `TypedFunc::call_async`
    before its task starts panics. Not reachable: the host calls nothing
    asynchronously.
  - **[0355](https://rustsec.org/advisories/RUSTSEC-2026-0355.html)**
    (GHSA-63h4-6cx9-pjfj): reading a `StreamReader<U>` that was made as a
    `StreamReader<T>` panics. Not reachable: the host makes no stream, and a
    component declaring a stream or a future is refused at load.
  - **[0356](https://rustsec.org/advisories/RUSTSEC-2026-0356.html)**
    (GHSA-7v24-42x9-pxf7): a guest's `realloc`, run while a stream or future
    result is lowered, can call back into the host and panic it. Not
    reachable: it needs stream or future results.
  - **[0357](https://rustsec.org/advisories/RUSTSEC-2026-0357.html)**
    (GHSA-93g3-36p8-x2gc): a zero-length stream read or write at an
    out-of-bounds address panics in `Direct{Source,Destination}`, which
    WASIp3 in `wasmtime-wasi` uses. Not reachable: streams are refused,
    nothing calls `Direct*`, and `wasmtime-wasi` is not linked.
  - **[0358](https://rustsec.org/advisories/RUSTSEC-2026-0358.html)**
    (GHSA-5hhx-4wrr-3j9g): `thread.yield` after an async-lowered import in a
    start function panics. Not reachable: both need the async feature.
  - **[0359](https://rustsec.org/advisories/RUSTSEC-2026-0359.html)**
    (GHSA-wjc8-633q-pwxr; every version since 42.0.0): a long chain of
    guest-defined types overflows the host's stack when it is dropped from
    the type registry. **Reachable, by a hostile component.** Function
    references are on by default in Wasmtime 48, whether or not GC is, and
    the host leaves them on, so a core module can define the chain (probed
    below). Upstream knows no workaround: the version is the fix.
  - **[0360](https://rustsec.org/advisories/RUSTSEC-2026-0360.html)**
    (GHSA-54p9-vhhq-8f7q): overlapping string transcode in a fused adapter
    panics. **Not 48.x**: 49.0.0 to 49.0.2 only. Here for being in the
    batch, not for this bump.
  - **[0361](https://rustsec.org/advisories/RUSTSEC-2026-0361.html)**
    (GHSA-hmwq-g22x-qjw5): event-loop work run after a trap, on a store left
    inconsistent, panics (an assertion in the component model's
    `concurrent.rs`). Not reachable: the host drives no event loop, and
    makes a fresh `Store` for every call, so none is used after an error.
  - **[0362](https://rustsec.org/advisories/RUSTSEC-2026-0362.html)**
    (GHSA-vcg5-q8m5-8379; since 42.0.0): a host function reached by an
    asynchronous call may re-enter Wasm with a synchronous one, which is not
    refused. Not reachable: the advisory says an embedder that uses no async
    at all is unaffected, and the host uses none.
  - **[0363](https://rustsec.org/advisories/RUSTSEC-2026-0363.html)**
    (GHSA-q554-3cjc-rx7p): component-model handle tables are capped per
    instance and charged to no limit, so a guest's `resource.new` grows host
    memory without bound. **Reachable, by a hostile component**: resources
    are the component model's own, behind no switch the host has off. The
    patch's default closes it, 100,000 live handles per store (probed
    below). Pleris's components have no resource.
  - **[0364](https://rustsec.org/advisories/RUSTSEC-2026-0364.html)**
    (GHSA-p79w-pv2r-29qp): with an odd number of exception tags, host
    structures are misaligned, undefined behaviour with no known exploit.
    Not reachable: exceptions are off (`wasm_exceptions(false)`, ADR-0244,
    the advisory's own workaround), so a component with a tag is refused at
    load.
- **Probed on 48.0.6**, with a scratch test that is not kept: WAT through
  `Component::new`, with the host's engine and, as the control, Wasmtime's
  default one.
  - An async-lifted export: refused, "canonical option `async` requires the
    component model async feature". `thread.yield`, `waitable-set.new`,
    `task.return`, a `stream` and a `future`: each refused, "requires the
    component model async feature"; the default engine loads them.
  - An exception tag, and a GC struct: refused.
  - Function types that refer to one another (`(ref null $t0)`): loaded; a
    chain of 20,000 loaded in the host's engine and was dropped without
    overflow. With `wasm_function_references(false)` the same types are
    refused, "function references required for index reference types".
  - A resource and `resource.new`: loaded. In one call 99,999 handles were
    made and 100,000 were not: "cannot allocate another handle: store handle
    limit exceeded". The host sets no `Store::set_max_component_handles`, so
    the patch's default stands.
- **The release**
  ([v48.0.6](https://github.com/bytecodealliance/wasmtime/releases/tag/v48.0.6),
  2026-10-14): fourteen fixes, the eleven above and the three in the WASI
  crates. It is a patch on the pinned line, as ADR-0116 and ADR-0244 chose,
  so Cargo moved 28 crates in each lockfile and nothing else, and the Rust
  sources change only in a version label. 49.0.3 fixes the same eleven and
  0360, but 49 is a major release, and its own decision (ADR-0244). The
  release's assets carry SHA-256 digests on GitHub, from which the CLI is
  pinned as ADR-0116 and ADR-0244 pinned theirs.

## Decision

1. **Wasmtime 48.0.6**, every live pin: the workspace and its lockfile,
   `pw-host`, `pw-conformance`'s tests, the standalone spike and its
   lockfile, and the bootstrapped CLI on macOS arm64, Linux x86_64 and Linux
   arm64. Cargo moved the release's own crates, in each lockfile and nothing
   else: 13 `cranelift-*` (0.135.5 to 0.135.6) and 15 at 48.0.5 to 48.0.6
   (`wasmtime`, `wasmtime-environ`, 11 `wasmtime-internal-*`,
   `pulley-interpreter`, `pulley-macros`). The compiler's `wasm-tools`
   crates are untouched. `tools/versions.lock` records what the CLI prints,
   `wasmtime 48.0.6 (72bc45bd1 2026-10-14)`, which `just doctor` compares
   for equality.
2. **The engine's configuration stays** as ADR-0244 made it: the proposals
   Pleris's components do not use remain off, and the reading above is the
   check ADR-0244 asks of any that comes back. Two advisories, 0359 and
   0363, are closed by the version alone. One narrowing is left for its own
   decision: `wasm_function_references(false)`, which refuses 0359's types
   (probed). The backend's only reference is a `funcref` table behind
   `call_indirect`, and `pw-host`'s and `pw-conformance`'s suites pass with
   the switch off (243 tests, run once, the edit not kept); the development
   server's, which run the most components, were not run so. It wants its
   own test and a full run.
3. **No advisory is ignored in `deny.toml`.** A patch on the pinned line
   exists for all eleven, and an ignore would let the host claim a bound
   the engine could not hold (ADR-0116).

## Acceptance

- **`cargo build --workspace --locked`** builds, and downloaded the 28
  approved crates and nothing else. The standalone spike's host builds with
  `cargo build --release --locked --offline`.
- **`bash scripts/bootstrap.sh`**, Linux x86_64, here: the archive matched
  its pinned digest, and the installed CLI prints
  `wasmtime 48.0.6 (72bc45bd1 2026-10-14)`. The three digests are GitHub's,
  for `wasmtime-v48.0.6-{aarch64-macos,x86_64-linux,aarch64-linux}.tar.xz`.
  The two Linux jobs of GitHub's `ci` run `just bootstrap`, so they check
  the Linux pair; the macOS one is checked by the next `just bootstrap` on
  the Mac.
- **The advisory ranges.** All 61 RustSec advisories for `wasmtime` and the
  one for `cranelift-codegen`, read from `advisory-db` with a script that is
  not kept (a version is affected unless an advisory's `patched` or
  `unaffected` range holds it): 48.0.5 falls inside exactly the eleven the
  red job names, and 48.0.6 inside none. No other crate this change moves
  has an advisory of its own name.
- **`runtime/pw-host/tests/engine_proposals.rs`, 5 tests**, pass on 48.0.6
  as on 48.0.5: the proposals stay refused when a component loads.
- **`just ci`** passes here: the workspace's 2,882 tests (25 ignored), clippy
  with `-D warnings`, the corpus checks, and the evidence gates' 156 tests.
- **CI's three jobs pass** on the push that carries this, "licenses and
  advisories" among them.

## Not claimed

- **49.x.**
- **`cargo deny` here.** It is not installed on this machine, and the job
  that runs it is the check. The range reading above stands in for it for
  the crates this change moves, not for the rest of the graph.
- **The Node audit.** It runs behind `cargo deny` and has not run on the red
  jobs; this change does not touch it.
- **The spike's guests, and `spikes/wasmtime-component/run.sh`.** Building
  the guests downloads `wit-bindgen` 0.60.0 and ten more crates that are not
  in this machine's cache, which the owner has not approved. `run.sh` was
  not run, and `docs/evidence/E0/spike-wasmtime-component.txt` stays what it
  was, a record of what it measured then.
- **That 0359 and 0363 cannot be reached.** They can, by a hostile
  component; the version closes them, and only components the compiler
  emitted are ever loaded.

## For the integrator

- Number this ADR and index it. `ADR-XXXX` is read in the comments of
  `scripts/bootstrap.sh` and `tools/versions.lock`.
- The four status documents are not edited here. A checkout whose
  `.toolchain` holds 48.0.5, the main one, fetches 48.0.6 on its next
  `just bootstrap` (the script fetches when the installed version differs).
  Every other branch's "licenses and advisories" job turns green only with
  this commit in it.

## The integrator's answers

On train-8, 2026-10-15:

- **Numbered ADR-0335.** `ADR-XXXX` reads `ADR-0335` in the comments of
  `scripts/bootstrap.sh` and `tools/versions.lock`.
- **`wasm_function_references(false)`** is a hardening track of its own,
  not this bump: its own test that the engine refuses RUSTSEC-2026-0359's
  types, and a full run, the development server's suites among them. NEXT
  holds it.
- **The E0 spike's evidence** stays what it was, a record of what it
  measured then (ADR-0116); a version label is no reason to download the
  guests' crates and run it again.
- **The macOS arm64 digest** has GitHub's word alone until the next `just
  bootstrap` on the Mac checks it; the two Linux digests are checked by
  every `ci` run's bootstrap.
