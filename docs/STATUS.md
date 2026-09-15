# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**Reviewed:** 2026-09-15, starting from master
`4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`.
**Charter:** v2, `PROJECT_CHARTER.md`.
**Numbering:** engineering E0-E15, public proofs P0-P9, risk experiments RQ-*.

**Current milestone:** E10 backend work has a reopened E9 prerequisite.
Ordinary call argument types and declared return types are not comprehensively
checked. E10-I, compiling `add_to_cart` through the production component backend
and executing it through the E8 host without an alternate Rust closure, is open.
The required order remains the resolved-signature migration, argument/return
checking, then component adapters and E10-I. See [NEXT](NEXT.md).

The former status file mixed chronological notes with obsolete headlines such as
"E9 is complete" and "no benchmarks yet". Its complete bytes are preserved in
[the historical ledger](STATUS-history-2026-09-15.md). That ledger records earlier
observations, not the current completion state. No old raw evidence is rewritten.

## completed gate items

- The Web Failure Census and layout-attribution repair were merged in
  `c10b825de40528a591e101653218ddff52e9ec6e`. The inventory contains 224 failure
  records and 72 source/build plus 72 runtime obligations. These are research
  records and proposed acceptance contracts, not 224 eliminated compiler bugs.
- The supply-chain repair was merged in
  `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`. The September 15 baseline CI run
  [35034596466](https://github.com/Kimeiga/perfect-web/actions/runs/35034596466)
  passed both Ubuntu architectures and the license/advisory job. Its head,
  `3bd5dbdb02b84d52a49e033a12bb71cff6a62b31`, was that baseline plus a temporary
  read-only source-snapshot workflow. That helper is removed by this repair.
- The evidence-gate repair adds `errexit` and `pipefail` to the recipe shell,
  drains summary output instead of closing the pipe early, and puts regression
  tests in `just ci`. Locally, all 45 success/failure scenarios over 17 real
  recipes passed. The original shell masked 24 of 28 injected failures.
  Grouped-command, renderer, large-output, and deliberately weakened-shell
  controls also passed. See [the bounded evidence report](evidence/tooling/evidence-gates-2026-09-15.md).
- The existing census Python suite and layout-attribution Node suite were rerun
  locally: 20 tests passed in each. No browser timing measurement was rerun.

The baseline CI result is not a result for this patch. Use the patch's own PR
checks for its Rust builds, audit, and complete gate-regression suite. Earlier
E7/E8 observations remain in the historical ledger and milestone documents;
this tooling repair does not independently re-establish those milestones.

## failing gate items

- **E9 value checking remains open.** `compiler/pw-core/src/signatures.rs` still
  represents signature types with written heads/arguments. `check.rs::call_arity`
  checks argument count, not argument-type agreement. The test
  `canonical_abi::a_call_site_is_not_type_checked` deliberately pins the missing
  ordinary-call relation. E9-V1..V6 distinguish representation from enforcement;
  do not close the gate merely because `ResolvedType` exists.
- **E10-I remains open.** A valid core Wasm module and a separately functioning
  host do not prove compiled Pleris execution through the production component
  path. Keep ADR-0023's deferred integration obligation.
- **Broader proposals remain proposals.** Temporal authorization, compatibility
  across live versions, commitment/unknown outcomes, composed budgets, and
  browser-owned editing behavior are recorded in the census. Their presence in
  prose does not establish complete generated-runtime enforcement.

## exact commands to reproduce

```sh
just evidence-gates
just ci
just audit
python3 research/failures/tools/validate.py
python3 -m unittest discover -s research/failures/tools -p 'test_*.py' -v
node --test spikes/layout-phase-scheduler/test/loaf.test.mjs
cargo test -p pw-core --test resolved_types --test call_arity --test canonical_abi --test one_comparison --test evidence_is_current
```

`just evidence-gates` requires Python 3 and `just`, but not Rust or browsers. It
runs actual recipe bodies in temporary trees with fake producers. The other
commands retain their real toolchain requirements. `just doctor` is read-only;
`just bootstrap` installs the pinned project dependencies.

## known environmental issues

The September 15 local review environment was Linux x86_64 with Python 3.13.5,
Node 22.16.0 and just 1.58.0. Rust was not available locally; actual Rust builds
and dependency auditing are checked through GitHub Actions, separately from the
injected-producer tests. No compiler or browser behavior is inferred from mocks.

Changing the parent recipe shell does not repair every nested shell, conditional,
or independently invoked script. Redirected evidence files may still be partial
after failure. A file's existence is not a successful test result.

## last benchmark summary

No new compiler, browser, memory, or application performance benchmark was run in
this review. Existing numbers retain their original revisions, machines, and
scope. In particular, earlier frame-level reads of forced-layout attribution
cannot establish browser non-support; ADR-0027 corrects that interpretation.

## next three concrete tasks

1. Complete the atomic `Signature` migration to recursive resolved semantic
   types, removing written-type semantic fallbacks. Follow the migration order
   in NEXT; do not merge the unfinished dual-representation branch.
2. Implement and discriminate ordinary call argument and declared return checking,
   including generics, nested types and nominal identities. Replace pinned-gap
   witnesses with intended-diagnostic and accepted-neighbor controls.
3. Finish the component adapters and E10-I using those checked signatures;
   execute the compiled command through the host and delete the alternate closure
   path. Preserve the census's remaining design obligations rather than marking
   them implemented by association.
