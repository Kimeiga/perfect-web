# Known limitations

Reviewed 2026-09-15 against master `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`
and the evidence-gate repair. Current milestone state is owned by
[STATUS](STATUS.md); implementation order is in [NEXT](NEXT.md).

The former file was primarily an E0 snapshot and still said no compiler or Linux
CI existed. Its complete bytes are retained in
[the historical limitations file](KNOWN_LIMITATIONS-history-2026-09-15.md).
Those historical statements must not override current source or test results.

## Value checking and generated execution

**E9-V1..V6 are met** (2026-09-24, ADR-0031). What the value relations do not
decide is Undecided, counted by `pw audit-values`, and never reported as
agreement:

- **Member existence.** `box.x` on a `Rect` with no `x`, or `{row.status}` on
  a type without it, is unknown rather than refused. A-015 reads `box.x` and
  `box.bottom` from a `Rect` that declares neither; the corpus was written
  without this relation, and it is the next one to add.
- **Sum-type variant constructors** (`Circle(3)`) have no type, because the
  workspace does not resolve variant names as terms. Record and opaque
  constructions are checked.
- **Named-argument calls** are Undecided: a signature does not carry parameter
  names. None occurs in the corpus.
- **A `()` body discards its last value** (A-018). A branch mismatch in
  statement position is not refused; as a result, it is.
- **Generic layouts at the boundary.** Phantom parameters map to one WIT
  layout; a generic whose layout depends on its arguments is refused, because
  specialization is not implemented.
- **Named function values are not instantiated.** A generic function named as
  a value (`List.map` passed along) has its type parameters as holes.
- **The unit value `()` lowers to `Expr::Error`.** Its type is therefore
  unknown. This is harmless to the relations, which never guess, but it is a
  lowering gap.
- **`g(a)` followed by `()` on the next line parses as `g(a)()`.** The language
  has no statement terminator; nothing in the corpus depends on the difference.

A scope/resource policy is now attached to the resolved type rather than its
spelling, preserving the existing type-level policy. This is not a per-value
ownership proof. Private RPCs now require conditional principal preservation;
that records a runtime obligation, not its execution. Stable nominal capture
identities distinguish modules; complete transitive schema-evolution and host
semantic-ABI validation remain separate work. Foreign code and backend internal
record/variant layouts still need their documented integration proofs.

**E10-I is established** (2026-09-24, ADR-0032): the store's commands compile
to components and run through the E8 host, and the Rust closure path is deleted.
The component backend is narrow, and everything outside it is refused by name
rather than approximated:

- **Straight-line bodies only.** Import calls and scalar constants are
  supported; values move flat or in their canonical layout. Constructing a
  record or variant, reading a field, a branch, a call to another compiled
  declaration, and a string constant are refused.
- **The invocation region is provisional.** It is a bump region reclaimed by
  each export's post-return, and nothing can outlive an invocation.
- **Resumable handler bodies are not compiled.** `add_to_cart(item.id,
  PositiveInt(1))` in the page is a dev-server handler posting the pressed
  instance and the literal quantity; the command it reaches is compiled.
- **The data layer is not Pleris.** `store:data/carts` is the deployment's
  (`owner: external`), as the contract records.

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
