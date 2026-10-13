# ADR-XXXX: a name imported by name is the file's, before a whole module's

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14 (the language). Found on the
way to NEXT 6a (money that is rounded only where a program names how).
Builds on the corpus' resolution rules (PW0021, PW0022) and ADR-0226 (an
import binds nothing by its name, as the name check reads it).

## Context

- **An import with no names makes every name of its module bare.**
  `resolve.rs`'s `resolve_in` reads `import List` as every one of `List`'s
  names, and `import domain.{ sum }` as `sum` alone.
- **The two together were ambiguous, and only the build said so.**
  `UserCarts.pw` imports `List` whole and, for 6a, `domain.{ sum }`. Its
  `sum(..)` resolved to both `List.sum` and `domain.sum`. `pw check` passed
  the call: a bare call's check reads an ambiguous resolution as resolved.
  The lowering refused it: "`sum` names no declaration this program
  contains". A program checked clean that did not build.
- **PW0022 was the import's alone.** It refuses one name imported by name
  from two modules, at the import. Two whole modules, or a whole module and
  a name, were never compared.
- **Practice.** Rust: "Items and named imports are allowed to shadow names
  from glob imports in the same namespace" (the Reference, "Use
  declarations", glob imports, read 2026-10-12). Haskell reports an
  ambiguous occurrence where the name occurs, not where its modules are
  imported.

## Decision

1. **A name a file imports by name is the one its bare uses mean**, before
   any module it imports whole: `import List` beside `import domain.{ sum }`
   makes `sum` `domain`'s. A local declaration still comes first, and the
   platform's prelude last.
2. **Two whole modules that both declare a name the file calls leave it
   ambiguous, and the call is refused where it is written** (PW0022): "`sum`
   is declared by `a` and `b`, each imported whole, and the file names
   neither", with "write `a.sum` or `b.sum`, or import `sum` by name from the
   module meant". Nothing is built from it, as before; it is now said at
   check.
3. **Nothing else changes**: a name imported by name from two modules is
   refused at the import, as before; a module's own declaration is its own.

## Alternatives

- **A whole-module import binds the module's name alone** (`List.map`, never
  bare `map`). Cleaner, and a change to every program that calls a whole
  module's name bare; not needed to remove the defect.
- **Refuse every name both a whole module and an import by name provide.**
  The file named the one it meant; refusing it would make an explicit
  import worth less than a whole one.
- **Rename `domain`'s `sum`.** It would leave the next such pair to be
  found by a build.

## Acceptance

- `compiler/pw-core/tests/explicit_imports.rs`: a name imported by name
  beside a whole module that declares it is the named module's, checks
  clean and builds, either way round; two whole modules' name called bare
  is refused where it is written, with the repair, and nothing is built;
  each qualified, and one whole module alone, check clean and build.
- Every program in the repository checks as before (`just ci`'s
  `test-compile`): none called such a name.
- `scripts/explicit_imports_mutations.py`, 5 mutants, each killed (`just
  e14-explicit-imports`).

## Not claimed

- **A whole module's name read as a value**, not called: the call is what
  is refused; a bare name used otherwise is read as before.
- **Types and views**: the order applies to every namespace's resolution;
  only a bare call's ambiguity is refused at check.
