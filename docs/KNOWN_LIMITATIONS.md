# Known limitations

Reviewed 2026-09-15 against master `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`
and the evidence-gate repair. Current milestone state is owned by
[STATUS](STATUS.md); implementation order is in [NEXT](NEXT.md).

The former file was primarily an E0 snapshot and still said no compiler or Linux
CI existed. Its complete bytes are retained in
[the historical limitations file](KNOWN_LIMITATIONS-history-2026-09-15.md).
Those historical statements must not override current source or test results.

## Value checking and generated execution

Ordinary call argument types and declared return types are not comprehensively
checked. Arity checking exists; the argument-type gap is explicitly pinned by
`compiler/pw-core/tests/canonical_abi.rs::a_call_site_is_not_type_checked`.
`Signature` still carries written type heads. The recursive resolved-type work
must become the single semantic authority before E9's reopened gate can close.

The recursive written-type prerequisite is now implemented (ADR-0028), including
nested resolution, built-in arity and qualified type namespace checks. Those are
properties of type formation/resolution, not proof that `pw check` applies full
value compatibility at every call or return. The legacy signature, privacy and
boundary consumers retain their previous representations pending the atomic
cutover. Nominal declaration arity, higher-kinded types and callable generics are
not added by this repair.

E10-I has not been established: producing core Wasm and testing host admission
separately does not demonstrate execution of the compiled Pleris command through
the complete component path with the alternative Rust closure removed.

## Research requirements are not implemented guarantees

The census records temporal authority, unknown external outcomes, compatibility
across live versions, composed capacity budgets, optimistic overlap, and unmanaged
browser/foreign behavior. Some related mechanisms already exist, but the census
itself does not prove complete enforcement. Read the per-record status and the
[research/implementation boundary](../research/failures/IMPLEMENTATION.md).

## Evidence and test coverage

The new recipe tests replace producers in isolated temporary trees. They prove
exit-status propagation for the tested recipe shapes, not compiler correctness,
browser behavior, host isolation, performance, or production readiness.

`errexit` and `pipefail` do not make shell execution universally fail-safe.
Independently invoked scripts and shell contexts that intentionally handle errors
need their own tests. Redirected reports may be incomplete after failure; this
repair does not make report publication atomic or validate every printed claim.

No new real-device, screen-reader, cross-browser, database-fault, or production
rollout test was run locally in the September 15 review. Prior observations keep
their original scope. The false-green recipe defect does not prove old tests
failed, but a recipe's zero exit status alone was insufficient evidence.

## Historical measurement interpretation

ADR-0027 supersedes the inference that an undefined frame-level
`forcedStyleAndLayoutDuration` establishes browser non-support. That value is
read from script-attribution records by the repaired instrumentation. Missing
observations and observed zero must remain distinct. Old raw measurements are
preserved, not replaced with invented results.
