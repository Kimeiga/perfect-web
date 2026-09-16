# ADR-0028: preserve complete type structure before the signature cutover

**Status:** Accepted for implementation under the owner's repair authorization.
**Date:** 2026-09-15
**Milestone:** Reopened E9, prerequisite for the resolved-signature migration.

## Concrete blocker

`ResolvedType` is recursive, but `DeclaredType` stores its arguments as strings.
The resolver consequently refuses every nested generic rather than resolving it.
The grammar already represents nested type arguments. The information is lost at
lowering, not absent from the language. Return annotations also still use a head
and a separate argument vector, unlike parameter and record-field annotations.

A signature cutover built on this boundary would either reject legal existing
nested types or invent another parser to recover information lowering discarded.

## Decision

- Make `DeclaredType` recursively own its argument types. `lower.rs` derives it
  directly from the existing syntax tree, for parameters, fields and returns.
- Store a declaration's entire return annotation in one `Option<DeclaredType>`.
  Remove the separate return-argument field. An absent annotation remains absent.
- Resolve each argument recursively. One unknown leaf prevents a resolved outer
  type; preserve the whole written type in the resulting diagnostic provenance.
- Require the exact arity of the language's built-in constructors. Resolve a
  qualified type path only in the type namespace. Recognize the grammar's `()`
  spelling as the unit primitive.
- Keep legacy `Signature` consumers wholly on their existing representation
  until the atomic resolved-signature cutover. Any written projections they
  still require are mechanically derived, never an additional authored field.

No new language syntax, unification rule, privacy policy, or representation-
transparent nominal conversion is introduced. This is not completion of E9 or
E10-I. Ordinary argument and return-value compatibility remain separate work.

## Evidence obligations

Tests must parse and lower real source before resolving it. Include nested
positive cases, unresolved leaves, argument order, distinct nominal identities,
qualified non-type names, built-in arity in both directions, and stable artifact
identities across file orderings. Keep a blocked-resolution control unrelated
to the nesting limitation that this repair removes. Run existing compiler and
artifact tests; report changed projections rather than silently rewriting them.
