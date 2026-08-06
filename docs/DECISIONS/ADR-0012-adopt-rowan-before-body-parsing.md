# ADR-0012 — Adopt Rowan before implementing body parsing

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** E2
**Resolves:** the lossless-tree question ADR-0009 left open

## Context

E2 currently has a gap-free token stream plus a declaration AST holding spans.
That is enough to round-trip source, but it is not a red/green tree: there is no
persistent node identity, no cheap subtree reuse, and no stable syntax-node API
for tooling to address.

The decision point is now, because E2's next task is body parsing. Once body
syntax, formatter behaviour, IDE features and incremental analysis all depend on
the current AST shape, migrating becomes much more expensive.

The project architect ruled: *"This is the last inexpensive point to do it."*

## Decision

**Adopt `rowan` before implementing body parsing.** The pipeline becomes:

```text
source text
    ↓  lexer            (existing; its losslessness invariants are kept as tests)
    ↓  rowan green tree
    ↓  typed syntax-node wrappers
    ↓  HIR lowering     ModuleId DeclId BodyId ExprId PatternId TypeRefId
    ↓  name resolution
    ↓  type / effect / scope / privacy analyses
```

Rowan owns: immutable lossless syntax, trivia and comments, incremental
reparsing, stable node APIs, formatter and refactoring support, editor tooling.

**Semantic analysis must not depend on Rowan nodes.** Checkers consume **HIR ids
and source spans**. That separation is the load-bearing part of this decision: it
keeps syntax-tree implementation details out of effect inference, exhaustiveness,
scope checking, privacy solving and placement analysis — the analyses that must
outlive any parser rewrite.

The existing losslessness properties (every byte covered, gapless contiguous
spans, no `Unknown` tokens across the corpus) are **retained as tests against the
Rowan tree** rather than discarded with the hand-rolled representation.

## Consequences

- `pw-core`'s five existing checkers are unaffected: they already take
  constructed data and spans, never parser types. That was accidental; it is now
  a rule.
- The declaration AST in `pw-syntax/src/ast.rs` becomes a typed wrapper layer
  over Rowan nodes rather than an independent structure.
- **Rowan does not give correct incremental parsing for free.** It supplies the
  representation. We still have to define reparseable boundaries, error-node
  behaviour, stable grammar rules, and when to fall back to whole-file parsing.
  Claiming otherwise would be the same category of error as claiming Koka gave
  us exhaustiveness.
- One new dependency, `MIT OR Apache-2.0`, compatible with the dual-licensed
  core. Version verified on crates.io: **0.17.0**.

## Alternatives rejected

**Keep the hand-rolled tree.** Cheapest today, most expensive at E9, and it
would force either a formatter rewrite or a formatter built on a representation
we intend to replace.

**Adopt after E9's incremental queries exist.** This is the option the ruling
explicitly closes: it locks the incremental layer onto a declaration AST that
cannot support the planned tooling cleanly.

## Revisit when

Rowan cannot express a needed syntax property, or the incremental-reparse
boundaries prove undefinable for this grammar. Either would be evidence, not
preference, and would need its own measurement.
