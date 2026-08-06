# ADR-0011 — Koka is an effects-and-handlers oracle only; `pw` owns value semantics from the start

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** E0 → E1 boundary
**Supersedes in part:** ADR-0001

## Context

Milestone 0 (now E0) measured four properties of Koka 3.2.3 that the bootstrap
plan had assumed. All four came out negative:

- exhaustiveness is enforced **only** for functions whose effect row excludes
  `exn`; a partial match in an `exn`-declaring function compiles and fails at
  runtime;
- single-field `value struct`s are **erased** — `Money_usd(350)` *is* `350`;
- `Nothing` and `Nil` are **both `null`** and runtime-indistinguishable;
- the generated `is_*` predicates are **discriminators, not validators**
  (`is_just(42) === true`; `is_qok(null)` throws).

Evidence: `docs/evidence/E0/spike-koka-js-interop.txt`.

The charter's original plan (§14 M1) leaned on Koka as the oracle for ADTs,
opaque wrappers, `Money`, and exhaustiveness. Three of those four it does not
provide. Koka is genuinely strong at exactly one thing: **effect rows and
handlers**, which it exposes machine-readably through `.kki`.

The project architect reviewed the evidence and ruled. This ADR records the
resulting design change.

## Decision

> **Koka is no longer the bootstrap oracle for the language as a whole. It is an
> effects-and-handlers oracle only. `pw` owns value semantics, exhaustiveness,
> ABI identity, decoding, privacy, placement, and lifecycle rules from the
> beginning.**

Concretely:

**1. `pw` performs exhaustiveness checking on its own typed pattern matrix,
before lowering to Koka.** An incomplete match is rejected regardless of the
function's effect row. Koka's result is supplemental, never the acceptance
oracle.

**2. `panic` becomes a tracked core effect.** It is forbidden in `view`,
build-placed computation, static generation, shared materialization, cache-key
computation, serialization/ABI codecs, authorization predicates, and
transaction-finalization logic. It appears in inferred signatures, effect
reports, semantic PR diffs, and capability manifests.

The escape hatch is deliberately hostile and never in the ordinary prelude:

```text
unsafe.partial_match(value) -> <panic, unsafe.partial_match> T
```

A compiler-proven impossible branch uses `unreachable(proof)` instead, which
carries no `panic` because uninhabitability was established by the type checker.
Writing "this cannot happen" in a comment is not proof.

**3. Nominal identity lives in a type-directed ABI schema, not in the payload.**
Three modes:

| mode | representation | guarantee |
|---|---|---|
| generated-to-generated | erased scalar + shared manifest | **compiler-level** |
| untyped / external caller | canonical `pw` ABI + structural validator | runtime-checked |
| debug / adversarial | tagged payloads with type and constructor identity | runtime-detected cross-wiring |

Production builds verify by content hash that artifacts and ABI manifests come
from the same compilation; mismatched artifacts are rejected by the loader.

**4. Boundary decoding is always type-directed.** `decode(payload, ExpectedType)`,
never `infer_runtime_koka_type(payload)`. Because `Nothing` and `Nil` both
collapse to `null`, runtime self-description is unavailable and the expected
schema is the only source of truth.

**5. `Result<T, E>` is a platform prelude type**, with generated adapters from
Koka's `error<a>`. No charter language may imply Koka supplies it.

**6. Generated boundary code does not use Koka's `is_*` predicates.** Instead,
codecs are generated **from the `pw` ABI schema** and emit a canonical
representation we own:

```text
Internal Koka ADT
  -> generated Koka pattern match
Canonical pw ABI value
  -> generated structural validator
JavaScript / component / network boundary
```

A direct adapter for Koka's raw emitted representation may exist temporarily for
performance experiments, but it is **not** the public ABI, and it requires golden
fixtures per pinned Koka version covering: payload-free constructor, single-field
erased constructor, multi-field tagged constructor, recursive constructor, list
cells and empty list, `maybe`/option, nested ADTs, record field layout, and
constructor insertion with tag renumbering. Any fixture change blocks the Koka
upgrade until the adapter is requalified.

**7. Capability compliance is checked against the final component artifact**, not
against source declarations or the nominal WIT world. Every build emits declared
capabilities, actual component imports, and the difference; any undeclared import
is a build failure (`PW4007`). Generated production components are
`no_std`-equivalent by default; a richer profile must be explicit
(`runtime_profile wasi-cli`).

## Consequences

- **ADR-0001's deletion condition is unchanged**, but its scope narrows: Koka is
  removed from normal builds when the own checker reaches corpus parity, and it
  was never the value-semantics oracle to begin with.
- **A new engineering milestone `E1A` is inserted** between `E1` (Koka effect
  feasibility) and `E2`, carrying `pw`-owned ADTs, the effect-independent
  exhaustiveness checker, opaque nominal types, platform `Result`, the ABI
  schema, structural validators, and tests proving `Option<T>` and `List<T>`
  cannot be confused at the seam.
- **`E1`'s scope shrinks to effects only** — inferred rows, handler discharge,
  propagation through direct calls, row-polymorphic callback effects, `.kki`
  span recovery, and diagnostic mapping. Its gate proves only that Koka is a
  usable temporary oracle for effects and handlers.
- **`E1`'s gate is not weakened by the narrowing.** The ≥30 compile-pass /
  ≥30 compile-fail requirement stands, now scoped to effects; `E1A` carries its
  own value-semantics corpus. A narrowed milestone does not get an easier gate.
- **The whitepaper may not claim runtime nominal safety.** Approved wording is
  recorded in `docs/EVIDENCE_LEDGER.md` under forbidden stronger wording.

## Revisit when

- The `pw` effect checker exists and Koka's role can end entirely.
- A pinned Koka upgrade breaks the golden representation fixtures.
- Row-polymorphic propagation fails (see ADR-0012), which would demote Koka
  further, from effects oracle to backend/reference implementation.
