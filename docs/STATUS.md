# STATUS

<!-- Charter §3.4 requires exactly these sections. Keep them. -->

**Reviewed:** 2026-09-24, against master `145588e` plus the E9-V change.
**Charter:** v2, `PROJECT_CHARTER.md`.
**Numbering:** engineering E0-E15, public proofs P0-P9, risk experiments RQ-*.

**Current milestone:** E9 is **closed again** (2026-09-24). Its reopening gates,
E9-V1..V6, are met by checks that refuse programs, with discriminators and
mutation controls: [evidence](evidence/E9/value-relations-2026-09-24.md),
[ADR-0031](DECISIONS/ADR-0031-value-relations.md). **E10-I is next**: compile
`add_to_cart` through the production component backend and execute it through
the E8 host, with the alternate Rust closure path deleted. See [NEXT](NEXT.md).

Three ADR-0031 decisions were made without an architect ruling and are offered
for reversal:
- privacy qualifiers are not value-transparent (§7);
- function types exist (§4);
- snapshots are read through `.value` (A-020).

The former status file mixed chronological notes with obsolete headlines such as
"E9 is complete" and "no benchmarks yet". Its complete bytes are preserved in
[the historical ledger](STATUS-history-2026-09-15.md). That ledger records earlier
observations, not the current completion state. No old raw evidence is rewritten.

## last passing commit

Baseline master `c200ae3e39934285858064fa56a09dabe882657d` includes the
recursive-type prerequisite (PR #4) and concurrent resource repair (PR #5). The new change's final-head CI must pass
before integration; the baseline pass is not a substitute.

## completed gate items

- **2026-09-24: E9-V1..V6, the ordinary value relations.**
  - `compiler/pw-core/src/values.rs` checks, three-valued, with diagnostics
    projected from a queryable analysis:
    - arity and argument types for path, member, piped, query, construction
      and policy-term calls;
    - declared results through branches, arms, `return` and `?`;
    - annotated bindings;
    - written types that resolve to nothing.
  - Language additions: callable type parameters instantiated per call,
    `type` parameters kept, function types typed bidirectionally, `?` as a HIR
    node, and declared-constructor arity.
  - Tests: 38 gate tests; 10 of 10 mutation controls killed.
  - Coverage: every relation in the store program is decided and agrees;
    the accepted corpus has 144 agreeing value relations and 343 resolving
    annotations; `pw audit-values` counts the undecided remainder.
  - Defects found and repaired (corpus C8): in the milestone demo, the
    accepted corpus and the libraries; see the evidence.
  - The pre-C8 text of every repaired rejected fixture is still caught for its
    declared invariant (`corpus_history.rs`).
  - `just ci` passes.
  - Not claimed: member existence, sum-type variant constructors,
    named-argument calls, non-phantom generic layouts at the boundary.


- **2026-09-16 resource repair:** shared requests now return the real terminal
  outcome; cancellation/invalidation fence late publication; command admission
  is atomic within this runtime; unwinding commands retain an explicit unknown
  outcome; logical deadlines and privacy mismatches are enforced. The full local
  workspace passed **937 tests, 0 failures, 1 existing ignored test**, including
  **24 new tests**. All nine initial regressions failed on the old runtime first.
  Formatter, workspace Clippy, corpus checks, recipe gates, and census/tooling
  suites passed. See [ADR-0029](DECISIONS/ADR-0029-owned-resource-flights-and-command-outcomes.md)
  and [scope, reproduction, and census mapping](evidence/E4/resource-repair-2026-09-16.md).
  This is a synchronous process-local repair, not durable exactly-once, a
  production cancellation adapter, or completion of E9/E10-I. Its own PR checks
  must establish the engine-feature build and current dependency audit.


- **2026-09-16: atomic resolved-signature cutover.** Parameter and return slots
  now contain recursive `TypeResolution`; the old written fields and independent
  `Interface::of` derivation are removed. Inference, members, privacy, captures,
  boundary decisions, WIT and backend callable signatures consume resolved
  identity. Contract identities are stable structured projections.
  Eleven new behavioral regressions failed on the old compiler and pass here;
  six authority tests and two historical-boundary controls also pass. Local
  workspace validation: **956 passed, one existing ignored documentation test**,
  including 178 core unit tests and 418 core integration tests. The accepted
  and store programs, formatting, workspace Clippy and census tests pass.
  [Evidence and exact scope](evidence/E9/signature-authority-2026-09-16.md).
  **This is not comprehensive ordinary-call typing or E10-I completion.**

- **2026-09-15 compiler follow-up:** recursive written types and one complete
  return annotation now survive lowering. Nested arguments resolve recursively;
  built-in arity and the qualified type namespace are checked; unit spelling is
  recognized. Direct generic resource results retain their arguments in manifests.
  The local compiler run passed **178 unit tests and 399 integration tests**
  across all 51 integration targets, including 11 new regressions. Formatter,
  compiler Clippy, and the accepted/store corpus checks passed. See
  [the bounded E9 evidence](evidence/E9/recursive-written-types-2026-09-15.md).
  That prior change did not complete ordinary argument/return checking or the
  signature migration. The latter is completed by the September 16 change above.

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
these compiler changes do not independently re-establish those milestones.

## failing gate items

- **Member existence is not a value relation yet.** `box.x` on a `Rect`
  without `x` is unknown rather than refused. This is the first follow-up after
  E10-I, recorded in KNOWN_LIMITATIONS; it was not one of E9-V1..V6.
- **E10-I remains open.** A valid core Wasm module and a separately functioning
  host do not prove compiled Pleris execution through the production component
  path. Keep ADR-0023's deferred integration obligation.
- **Broader proposals remain proposals.** Temporal authorization, compatibility
  across live versions, commitment/unknown outcomes, composed budgets, and
  browser-owned editing behavior are recorded in the census. Their presence in
  prose does not establish complete generated-runtime enforcement.

## exact commands to reproduce

```sh
cargo test --locked -p pw-resource
just evidence-gates
just ci
just audit
python3 research/failures/tools/validate.py
python3 -m unittest discover -s research/failures/tools -p 'test_*.py' -v
node --test spikes/layout-phase-scheduler/test/loaf.test.mjs
just e9-values
cargo test --locked -p pw-core --test value_relations --test corpus_history
python3 scripts/e9_value_mutations.py
cargo test -p pw-core --test signature_behavior --test signature_authority --test corpus_history
cargo test -p pw-core --test recursive_declared_types --test resolved_types --test call_arity --test canonical_abi --test one_comparison --test evidence_is_current
```

`just evidence-gates` requires Python 3 and `just`, but not Rust or browsers. It
runs actual recipe bodies in temporary trees with fake producers. The other
commands retain their real toolchain requirements. `just doctor` is read-only;
`just bootstrap` installs the pinned project dependencies.

## known environmental issues

The September 16 resource repair used the pinned Rust 1.97.1 and locked registry
snapshot locally on Linux x86_64. Its full workspace run completed with a captured
zero exit code. These real runtime/compiler results are distinct from the earlier
mocked recipe tests. The evidence report records scope and environment.


The September 15 local review environment was Linux x86_64 with Python 3.13.5,
Node 22.16.0 and just 1.58.0. Rust was not available locally; actual Rust builds
and dependency auditing are checked through GitHub Actions, separately from the
injected-producer tests. No compiler or browser behavior is inferred from mocks.

The compiler follow-up used a verified, isolated copy of the pinned Rust 1.97.1
and its locked registry dependencies. Its real local compiler tests are distinct
from the earlier shell failure-injection tests. Long full-suite commands exceeded
the review runner's execution window, so all compiler integration targets were
run to completion in explicit batches; the incomplete attempts are not passes.

Changing the parent recipe shell does not repair every nested shell, conditional,
or independently invoked script. Redirected evidence files may still be partial
after failure. A file's existence is not a successful test result.

## last benchmark summary

No new compiler, browser, memory, or application performance benchmark was run in
this review. Existing numbers retain their original revisions, machines, and
scope. In particular, earlier frame-level reads of forced-layout attribution
cannot establish browser non-support; ADR-0027 corrects that interpretation.

## next three concrete tasks

1. **E10-I steps 4-6.**
   - Canonical ABI adapters, with every flattening number taken from
     `wit-parser` 0.257.1.
   - One core module per component, with `memory`, `cabi_realloc` and
     post-return.
   - Wrap with upstream `wit-component` 0.257.1.
   - Validate independently, including a component-level same-core-ABI
     mutation control.
2. **E10-I steps 7-9.**
   - `pw-host` links and calls the compiled component with host functions.
   - `add_to_cart` runs through it in the dev server.
   - Delete the Rust closure path, and record `just e10-i` evidence.
3. **The rest of E10.** Then the kiokun proof slice (lookup and search over one
   shard) as the first application written against checked signatures.
