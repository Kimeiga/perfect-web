# ADR-0014 — HIR representation: id-indexed arenas with a span for every node

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** E2
**Implements:** the `ModuleId DeclId BodyId ExprId PatternId TypeRefId` layer
ADR-0012 named but did not specify

## Context

ADR-0012 settled that semantic analysis consumes **HIR ids and spans, never
syntax nodes**, and drew the pipeline. It did not say what HIR *is*. That is now
the blocking question: `compiler/pw-core` holds exhaustiveness, typed ABI
decoding, scope and capability checking — 62 passing tests — and
`compiler/pw-syntax/src/grammar.rs` parses all 68 corpus files. **Nothing
connects them.** Every remaining rejected-corpus case needs that link.

The shape has to be decided before lowering is written, because every checker
signature depends on it and each one written against the wrong shape is a
migration later.

## Decision

**HIR is a set of id-indexed arenas. Every id resolves to both a node and a
source span. Exactly one module is allowed to see a syntax node.**

```text
Hir
├── modules : Arena<Module>      ModuleId
├── decls   : Arena<Decl>        DeclId    → name, kind, signature, Option<BodyId>
└── bodies  : Arena<Body>        BodyId
                └── each Body owns its own arenas:
                    exprs  : Arena<Expr>      ExprId
                    pats   : Arena<Pattern>   PatternId
                    types  : Arena<TypeRef>   TypeRefId
```

Four properties, each chosen against a specific failure:

1. **Per-body arenas, not one global arena.** A body is the unit of reanalysis.
   Global arenas would make every id in the program shift when one function is
   edited, which defeats the incremental reuse ADR-0012 adopted Rowan for.

2. **A span for every id, stored in the arena beside the node.** Not optional,
   not reconstructed by walking back to syntax. Charter §16.3 requires each
   diagnostic to carry an origin span, a boundary span, an inferred label and a
   legal alternative; a checker that cannot name a span cannot produce a legal
   diagnostic. Making the span a sibling field means "I have an id" implies "I
   can point at source".

3. **`lower.rs` is the only module that may import a syntax node type.** This is
   the enforceable form of ADR-0012's rule. It is checked by a test, not by
   convention — a grep-based test fails if any other `pw-core` module mentions
   `SyntaxNode`.

4. **Lowering is total and never panics.** Unparseable or unrecognised syntax
   lowers to `Expr::Error` carrying its span, so a body with one bad expression
   still yields a walkable HIR for every other expression in it. A checker
   should degrade, not vanish, on a file it cannot fully understand.

### What lowering deliberately does not do

**No name resolution.** `a.b.c` lowers to nested `Field` nodes, exactly as
parsed. Whether `Stores.get` is a module path or a field access on a local is a
resolution question and belongs to the next layer; folding it during lowering
would bake a guess into the representation. This mirrors the parser decision not
to absorb dotted paths (`docs/DECISIONS.md`, 2026-08-05).

**No desugaring that loses source shape.** `ParenExpr` collapses to its inner
expression because parentheses carry no meaning beyond grouping, which the tree
already encodes. Nothing else is rewritten. A pipeline `a |> f` stays a binary
node rather than becoming a call, because an effect diagnostic that points at a
synthesised call the author never wrote is worse than one that points at `|>`.

## Consequences

- Checkers take `(&Body, ExprId)` and return diagnostics carrying spans. They
  never see Rowan, so a syntax change that preserves lowering cannot break them.
- The existing `pw-core` algorithms need adapters, not rewrites: they already
  work on ids and spans, which is why ADR-0012 insisted on that interface before
  any of them were written.
- Incremental reanalysis reduces to re-lowering one body.
- Cost: lowering is a real pass to maintain, and any node kind the grammar adds
  must be lowered or explicitly recorded as `Expr::Error`. The totality property
  makes the second option safe rather than silent.

## Revisit when

Name resolution needs to interleave with lowering (it should not — resolution
consumes HIR), or per-body arenas prove too coarse because a single body is
large enough that re-lowering it dominates edit latency. Neither is measurable
before E9.
