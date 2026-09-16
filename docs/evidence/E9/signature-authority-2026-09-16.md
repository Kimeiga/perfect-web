# Resolved-signature authority: bounded evidence, 2026-09-16

## Scope and provenance

Compiler baseline: `49398dc02be6f2c3ea1a28fdfb77bf66c15266b6` (PR #4).
Integration base: `c200ae3e39934285858064fa56a09dabe882657d` (PR #5).
The latter's resource implementation and tests are preserved byte-for-byte;
only its STATUS additions are reconciled with the new compiler status.
ADR-0030 owns this compiler change; concurrent ADR-0029 owns the runtime change.
No dependency, lockfile, or permanent workflow change is included.

## Executed controls

`signature_behavior.rs` uses public compiler entry points, parses every input
first, and asserts intended behavior. All eleven tests failed on the baseline
and pass on the repair. The baseline copy differs only in formatting, verified
by normalizing it with the same Rust formatter and comparing bytes.
[Raw baseline output](signature-behavior-baseline.txt) and the exact
[baseline test source](signature-behavior-baseline.rs.txt) are preserved.

These tests cover same-spelled nominal event mismatches, imported handlers,
private field labels, a user-defined homonym of a platform privacy constructor,
precise resource identities (including generic arguments), stable capture
schemas, nested WIT types, every variant payload field, Unit/empty records,
and conditional private RPC transfers. Accepted neighbors and reordered-file
controls constrain the repairs. Generated WIT is independently parsed by the
locked wit-parser rather than accepted because the generator emitted it.

Six additional tests cover signature identity/provenance, absent versus unresolved
slots, UI signatures without invented term-callability, principal identity, the
single stable projection, and reuse of the existing type parser. Two historical
controls keep C0 unknown captures refused and verify the repaired event fixture.
No historical corpus text or old raw measurement is edited.

## Results

| Check | Actual local observation |
|---|---|
| Baseline behavioral regressions | 0 passed, 11 failed |
| New behavioral regressions on repair | 11 passed |
| New authority and historical controls | 8 passed |
| Core unit suite | 178 passed |
| All 53 core integration targets | 418 passed |
| Other workspace crates, after integrating PR #5 | 360 passed |
| Core documentation tests | One pre-existing ignored test |
| Workspace Clippy, all targets, warnings denied | Passed |
| Wasmtime engine-feature Clippy | Passed |
| Rust formatting | Passed |
| just test-compile | 59-file accepted and 37-file store programs clean |
| Contract/WIT regeneration | Passed; committed WIT matches emission |
| Census structural validator and Python suite | 224 records validated, 20 tests passed |
| Layout-attribution Node tests | 20 passed; no browser timing claim |

Total workspace tests across completed local batches: **956 passed, zero failed,
one existing ignored documentation test**. The increase relative to PR #5 is
19 compiler regression/control tests. [Raw completed outputs](signature-authority-test-output.txt).

Long combined local commands exceeded the review runner's execution window,
including just ci during the unchanged recipe-test harness. These interrupted
runs are not passes. All compiler targets and other workspace tests were run to
completion in separate commands. This PR's final-head just ci, both Ubuntu
architectures, Koka integration, and current audit must pass remotely before merge.
No baseline or temporary transfer-workflow check may substitute for that result.

The baseline comparison used an isolated worktree. After replacing source
snapshots, affected Cargo artifacts were cleaned/rebuilt to avoid treating old
build output as proof of the replacement. Local toolchain: Rust 1.97.1, Python
3.13.5, Node 22.16.0, just 1.58.0, Linux x86_64; dependencies are locked/offline.

## Reproduce

```sh
cargo test --locked -p pw-core --test signature_behavior --test signature_authority --test corpus_history
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p pw-host --features engine --all-targets -- -D warnings
cargo fmt --all -- --check
just test-compile
just e8-contracts
just e8-wit
just ci
just audit
```

For the baseline control, copy the archived baseline source to
compiler/pw-core/tests/signature_behavior.rs in a separate worktree at the
baseline commit. Use a separate target directory or clean rebuild. New API-level
authority tests deliberately cannot compile against the old representation.
Raw integration outputs identify every target; seven targets per batch (four
in the last batch), then the core unit/doc targets and other workspace crates.

## Actual contract and fixture changes

Signature parameters/results, member dispatch, inferred bindings, capture types,
boundary profiles, and WIT/backend callable interfaces now consume resolved
identity. Interface is derived from Signature, not re-created from declaration
syntax. No written/resolved dual fields remain. Stable artifact identities use
qualified declarations and recursive arguments, never process-local DefIds.

WIT no longer confuses nested generic commas, drops later variant fields, turns
empty records into Bool, or treats a same-spelled foreign generic as a platform
privacy wrapper. RPC privacy is no longer replaced with Public: it produces a
conditional principal-preservation obligation. Lowering preserves complete nested
effect/variant/opaque types; remaining source fragments use the existing grammar.

C7 documents explicit-import repairs and an effectful optimistic helper whose
return really is Cart. R-029 still fails for impurity, not an accidental return
mismatch. The platform signature hash changes for the list module's imports.
C0's R-010/R-030 are refused as unknown captures; its unimported R-022 event is
blocked rather than compared by spelling. Current imported controls retain their
intended diagnostics. These distinctions are tested, not silently excused.

The generated contract JSON changes from source strings to StableTypeId objects.
This is an intentional research contract format change. External consumers of the
old representation must regenerate or adapt; it is not a rolling-deployment
compatibility claim. Store WIT removes a redundant erased privacy alias; emitted
host callable payload shapes remain independently validated.

## Remaining obligations

This does not implement general ordinary-call argument/return unification,
callable generics, all unknown-type diagnostics, nominal constructor arity,
per-value resource ownership, or E10-I. Internal backend record/variant layout
work remains distinct from WIT signature correctness. Conditional RPC privacy
requires runtime enforcement. Stable nominal capture identity is not complete
transitive schema evolution. Existing capability/host/interop assumptions remain.
The active roadmap remains E9 value checking, then compiled-command integration.
