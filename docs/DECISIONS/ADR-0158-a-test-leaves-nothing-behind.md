# ADR-0158: a test leaves nothing behind

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-03.
Milestone: E14. Found while ADR-0157's tests ran on a disk 97% full.

## Context

On 2026-10-03 the temporary directory held 4,809 directories, 673 MB, that
this repository's tests had made and never removed:
- **3,561 `pw-served-*`**, and `pw-lines-*` under its earlier name: the
  development server's tests build a store for each test;
- **596 `pw-semantic-*`**: `pw-core`'s `semantic_diff.rs`;
- **222 `pw-js-*`**: `pw-conformance`'s `javascript.rs`;
- **about 100 `pw-plan-*`**: `pw-render`'s `plan_lists.rs`;
- **about 180 `pw-bench-*`**: the harness's isolation test, and the
  sandboxes it makes;
- `pw-bind-*`, `pw-event-*`, `pw-speculation-*` and `pw-koka-oracle-*`.

Each named its directory by the process id, so no run reused another's, and
none removed its own. After they were removed, two `just ci` runs and a
mutation run made 1,057 more. This project's disk runs nearly full, and an
interrupted build already costs 20 GB.

## Decision

1. **A Rust test's directory is a `tempfile::TempDir`.** It is removed when
   the test is done with it, whether the test passed or panicked.
   - `tempfile` 3.27.0 is already in the graph through wasmtime. It is a
     dev-dependency of the four crates whose tests make a directory.
   - Its API was checked against docs.rs for 3.27.0:
     - `TempDir::with_prefix`;
     - `TempDir::path`;
     - a destructor that deletes the directory and everything in it, and
       ignores any error doing so.
2. **The development server's test store keeps its directory as long as its
   server.** The server reads handler modules from it on request. `Served`
   holds both, and derefs to the server.
3. **The harness removes what it makes, however a step ends.**
   - The isolation test removes its task directories.
   - A sandbox that could not be made, or was refused for holding a hidden
     file, is removed.
   - A run's sandbox and its copied specs are removed in `finally`, unless
     `BENCH_KEEP` asks to keep them.

## Not claimed

- **A process killed before its destructors run leaves its directory**, as
  `tempfile` says: a signal, or a timeout's kill. The operating system's
  cleanup of its temporary directory is what remains.
- **Directories with fixed names are left as they are.** They are removed
  before and after use, and do not accumulate:
  - `one_comparison.rs`;
  - `name_keyed_maps.rs`;
  - `support::wit_dir`.
- **Neither are the pid-named directories already removed after use:**
  - `handlers.rs`;
  - `same_name.rs`;
  - kiokun's escape test;
  - the development server's handlers test.
