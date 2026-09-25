# ADR-0045: an affine value is consumed exactly once, on every path

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10 (charter §14 M10
task 7).

## Context

Charter §14 M10 task 7: "Expose affine annotations only for scarce resources
where they express a real invariant." In Pleris the annotation is the effect
row of the function that makes or ends a resource:
- `Database.begin()` declares `resource.acquire<DatabaseTransaction>`;
- `commit` and `rollback` declare `resource.release<DatabaseTransaction>`.

Nothing is written where the value is used, and gate item 5's evidence holds
that no ordinary value needs an annotation
([ownership-2026-09-25.md](../evidence/E10/ownership-2026-09-25.md)).

The invariant the annotation expresses is PW2005: "an affine value must be
consumed exactly once, in the scope that acquired it". The check enforced less
than that. It asked whether a release came before each `return`. Three shapes
passed `pw check`:
- a transaction opened and never ended, in a body with no `return`;
- a transaction live across a failing `?`, which is an exit too;
- a transaction ended twice.

A declaration that took a transaction in order to end it was never held to
doing so. The module's own header still described the textual scan that the
branch-aware analysis had replaced.

## Decision

### 1. Every path is counted

Each path from the acquisition to where the value leaves its scope must
release it exactly once. The value leaves its scope at the end of the block
that binds it, at a `return`, or at a failing `?`. The analysis counts
releases along each path of the statement tree, as 0, 1 or "more than once",
and reports the first fault a reader meets:
- a path that leaves without releasing;
- a path that releases twice;
- a release inside a loop or a function value, which runs any number of
  times **(ruling needed:** refused even when the loop would run once**)**.

The statement tree is enough because `pw` has no `goto`, no labelled
`break`, and no loop that can carry a resource out.

### 2. The body's value moves the resource to the caller

`fn open() -> DatabaseTransaction !{ resource.acquire<..> } { let tx =
Database.begin(); tx }` consumes `tx` by returning it; so does `return tx`.
**(ruling needed)**: the alternative is to require a producer to return the
acquiring call directly.

### 3. A declaration that promises to release a parameter must

A declaration whose row says `resource.release<T>` takes its `T` parameter in
order to end it. Its body owes that parameter what an acquisition is owed:
exactly one release on every path. A `todo` body is not written yet, and
promises nothing. **(ruling needed)**: the obligation is read from the row,
as the acquisition's is, and needs no new syntax.

### 4. Unchanged

- A `use` binding is released by the block that binds it, so it must only
  not escape (R-012). It is not held to an explicit release.
- Passing a value to a function whose row does not release it is a use, not a
  consumption. Pleris has no borrows, and a function that writes inside a
  transaction declares no release.
- No syntax is added. The annotation stays the effect row, written once per
  library function, and ordinary values carry none.

## Acceptance

- `examples/generality/affine_not_consumed_once/`: five new `GENERAL`
  witnesses, each refused and each accepted before:
  - never released;
  - live across `?`;
  - released twice;
  - released in a loop;
  - a parameter not released.

  Two new `NEIGHBOUR`s pass: a helper that releases its parameter on each
  path, and a producer that returns the value.
- The accepted corpus checks clean, and R-011 and R-012 are refused as before.
- Mutation controls: `scripts/affine_mutations.py`, `just e10-affine`. Seven
  mutants, each undoing one piece: the end of a scope as an exit, `?` as an
  exit, the second release, the loop, parameters, the move to the caller,
  and `use`.
