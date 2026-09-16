# ADR-0030: resolved signatures own value types

Status: accepted under the owner's repair authorization; implemented with the
regression results linked below.
Date: 2026-09-16. Milestone: reopened E9.

## Decision

Replace signature type strings with recursive `TypeResolution` values. Missing
annotations, unresolved annotations, and known types remain different states.
All signature consumers must inspect semantic identity; printed types are for
diagnostics and display only. A field or member is indexed by its receiver's
resolved declaration or language constructor, never a bare type spelling.

Use a provenance-free semantic key for maps. `ResolvedType::same_as` delegates
to that key, so ordering and equality cannot disagree about source spans.
Boundary and WIT decisions consume complete resolved types. Stable artifact
identities derive from declaration identity, not process-local indices.

The migration is atomic on the integration branch. Do not merge a mixture of
written and resolved signature authorities. Subsequent call/return checks must
consume the same resolved signatures; do not create a separate string checker.

## Acceptance

Preserve existing accepted/rejected controls. Add same-spelled nominal types,
reordered files, nested generic payloads, missing versus unresolved annotations,
and resolved receiver controls. Run compiler/workspace tests, independent WIT
validation, corpus checks, formatting, lint, audit and the PR's own CI before
merging. No E9 or E10-I completion claim follows from representation alone.

## Observed consequences

The migration exposed real spelling-based behavior: imported handlers could go
unchecked, parameter-owned field labels were lost, same-named nominal types
shared capture schemas/resource policies, and RPC privacy was flattened to
Public. New regressions cover those boundaries. WIT projection now retains
nested generic arguments, every variant payload field, and unit/empty-record
shape. Remaining opaque/effect/variant source strings are parsed by the existing
type grammar; lowering preserves their complete nested arguments.

Current rejected programs still fail for their own rules after missing imports
and optimistic helper result types are repaired. Historical files are unchanged;
unknown captures remain refused and the old unknown event annotation is recorded
as blocked rather than mislabeled as a known nominal mismatch.

See [bounded evidence](../evidence/E9/signature-authority-2026-09-16.md).
Ordinary-call unification, callable generics, nominal arity, and E10-I remain open.
