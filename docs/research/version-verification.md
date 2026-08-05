# Version verification log

Charter §5: *"Do not trust dates, versions, package names, or APIs in this
charter without checking current primary sources. Pin what was actually tested."*

Every version this project depends on, where it was checked, and whether it was
merely *read* or actually *run here*.

**All checks below performed 2026-08-05.**

## Checked against primary sources

| what | source queried | result | status |
|---|---|---|---|
| Koka | `api.github.com/repos/koka-lang/koka/releases/latest` | **v3.2.3**, published 2026-03-18, `koka-v3.2.3-macos-arm64.tar.gz` present | **run here** |
| Wasmtime | `api.github.com/repos/bytecodealliance/wasmtime/releases/latest` | **v47.0.3**, published 2026-07-31 | **run here** |
| Lima | `api.github.com/repos/lima-vm/lima/releases/latest` | **v2.2.0**, published 2026-07-21 | read only (Milestone 11) |
| Rust stable | `static.rust-lang.org/dist/channel-rust-stable.toml` | **1.97.1** (8bab26f4f, 2026-07-14) | **run here** |
| Node LTS | `nodejs.org/dist/index.json` | active LTS **v24.19.0** "Krypton"; host has v22.21.1 | see A-002 |
| pnpm | npm registry | latest **11.20.0**; host has 10.15.1 | pinned to host version |
| marko | npm registry | **6.3.32** (`latest`), MIT, `engines.node >=22` | **run here** |
| @marko/run | npm registry | **0.11.8**, MIT | **run here** |
| @marko/run-adapter-node | npm registry | latest **2.0.6**, MIT, peer `@marko/run ^0.7.4‖^0.8‖^0.9‖^0.10‖^0.11` | **run here** |
| @marko/vite | npm registry | **6.1.9**, MIT, peer `vite ^8` | transitively |
| vite | npm registry | **8.2.0**, MIT, `engines.node ^20.19.0 ‖ >=22.12.0` | **run here** |
| Playwright | npm registry | **1.62.1**, Apache-2.0 | read only (Milestone 3) |
| annotate-snippets | crates.io | max stable **0.12.16**, MIT OR Apache-2.0 | **run here** |
| wit-bindgen | crates.io | **0.60.0** | **run here** |
| ariadne / codespan-reporting / miette | crates.io | 0.6.0 / 0.13.1 / 7.6.0 | considered, not adopted (ADR-0009) |
| rowan / logos | crates.io | 0.17.0 / 0.16.1 | candidates for Milestone 2's lossless tree |

## Corrections this check produced

Assumptions that were wrong and were fixed before anything depended on them:

1. **`annotate-snippets` 0.13 does not exist.** Max stable is **0.12.16**.
   The first `Cargo.toml` pinned `"0.13"`.
2. **The `annotate-snippets` API is not what earlier versions used.** 0.12 uses
   `Level::ERROR.primary_title(..).element(Snippet::source(..))` and
   `AnnotationKind::Primary` / `::Context`. Verified from docs.rs before writing
   code rather than after failing to compile.
3. **`@marko/run-adapter-node` is at 2.0.6, not 0.11.8.** The adapter's version
   line diverged from `@marko/run`'s; its `peerDependencies` confirm 2.0.6 works
   with `@marko/run` ^0.11. Pinning by matching version numbers would have failed.
4. **Marko 6 is `latest` on npm**, not `next`. Both tags exist
   (`latest: 6.3.32`, `next: 6.1.8`); `next` is *older*.
5. **`wasmtime` CLI has no `component wit` subcommand** in 47.0.3.
6. **WASI 0.3 is not reachable** through Rust's stable `wasm32-wasip2`; emitted
   imports are `@0.2.9`. This is the measurement charter §10.3 asked for
   (ADR-0008).
7. **The host is not the machine the charter assumes** — M2 Pro / 16 GiB, not
   M3 Max / 64 GB (assumption A-001).

## How to re-verify

```bash
just doctor        # compares live versions against tools/versions.lock
just env-record    # regenerates the lock and docs/environment/macbook.md
just spikes        # re-runs every measurement in this file
```

`tools/versions.lock` records what was actually installed and tested, and
`just doctor` warns when a live version drifts from it.

## Re-verification triggers

- Any Rust or Wasmtime upgrade → re-run `just spike-wasmtime` (WASI version).
- Any Koka upgrade → `node/kki.mjs` **will throw**; re-verify the `.kki` format.
- Any Marko/Vite upgrade → re-run `just spike-marko` and compare byte budgets.
