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

- **Straight-line bodies, at E10-I.** Import calls and scalar constants were
  supported, and values moved flat or in their canonical layout. Matches over
  `Option` and `Result`, field reads and their cases came after (ADR-0036),
  and then computation (ADR-0039, below).
- **The invocation region is provisional.** It is a bump region reclaimed by
  each export's post-return, and nothing can outlive an invocation.
- **The data layer is not Pleris.** `store:data/carts` is the deployment's
  (`owner: external`), as the contract records.

**The backend matches over `Option` and `Result`**, reads fields, and builds
their cases (2026-09-25, ADR-0036). **It computes** (2026-09-25, ADR-0039):
`Int` and `Float` arithmetic, comparisons, `&`, `|`, `!`, `if`, string
literals, interpolation, records built, and calls to other declarations,
inlined. Still refused by name:

- nested patterns, and a declared variant built or matched;
- recursion, and a call to a generic declaration: calls are inlined;
- an early `return`, an assignment, a pipeline, and a loop;
- `%` on a `Float`, and a `Float` interpolated: their semantics are not
  decided;
- list and string operations beyond the ones the standard library declares
  (see below).

- **Traps are not distinguished by cause.** An `Int` overflow and a zero
  divisor both stop the invocation, and the host reports a failed call; which
  one it was is not carried.
- **The checker does not compare a comparison's operands.** `1 == "a"` types
  as a `Bool` and passes `pw check`; the backend refuses it. A value relation
  for it is the fix, and is not built.
- **The logical operators are `&` and `|`.** `&&` does not parse. They
  short-circuit.

- **Some matches are not analysed for exhaustiveness.** Each is counted
  Blocked by the match audit, with its reason: neither proven nor refused.
  - A constructor pattern nested under `Some`, `Ok` or `Err`
    (`Some(Some(x))`). The backend refuses nested patterns.
  - A literal pattern (`Some("a")`).
  - A scrutinee the value relations cannot type, including a name bound at
    two sites.

  Until 2026-09-25 matches over `Option`, `Result` and calls were not checked
  at all, and four other shapes were proven exhaustive when they were not
  (ADR-0038).
- **A binding cannot share a constructor's name** (ADR-0038, ruling needed).
  A bare pattern name that some type has as a constructor is read as that
  constructor, and against another type it is PW0608.
- **`return` is a statement, not an expression.** Its value is the statement
  after it, in a block or on its line in a match arm (ADR-0038).
- **Interpolated attribute strings do not render.** `href="/stores/{id}"` is
  read by the route checker, but the template IR has no part for it and blocks
  the render, naming `href={value}` as the form that renders.
- **The standard library's list functions are signatures.** `List.map`,
  `fold`, `filter` and `length` have stub bodies, and there are no string
  functions. Pleris programs type-check against them and cannot compute with
  them.

**Resumable handler bodies are compiled** (2026-09-25, ADR-0033), to one ES
module each, and the page's elements carry what the handlers read. The set is
narrow:

- **One command call per handler.** Its arguments may be captured values and
  their fields, literals, and opaque constructors over a primitive. Local
  computation, branches, multi-statement bodies and handler parameters (the
  event) are refused, and so is a command parameter that is not a primitive or
  an opaque type over one.
- **Captures are not a patched part.** An element carries the capture paths
  its handler reads, as rendered. A patch that changes a captured field without
  re-rendering the element leaves the old value there: for example, E7-P's
  keyed rename, which replaces only the item's text. The store's handler reads
  only `item.id`, the loop's key, which a keyed patch never changes.
- **Opaque invariants are not checked at the boundary.** The host types a
  browser's arguments by the component's parameters: `PositiveInt` arrives as
  an `s64`, and any `s64` is accepted. `opaque type PositiveInt = Int` states
  no invariant that could be checked.
- **String escapes are not defined** (A-023). A string literal containing a
  backslash, and a `"""` string, have no value in the handler backend or the
  Wasm lowering, and both refuse them. The Koka and Marko backends pass the
  token through to their targets' escape rules.

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
