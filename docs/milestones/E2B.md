# E2B — Program graph and name resolution

**Status: IMPLEMENTED, gate partly met.** The module graph, namespaces and the
six resolution diagnostics exist and run inside `pw check`. What remains is
resolving *uses*, not only imports.

## Why it exists

`docs/ASSUMPTIONS.md` A-009 says "one `pw check` invocation is one program",
and the implementation reads that as *every declaration in every supplied file
is ambiently visible everywhere*. The architect ruled that wrong:

> `pw check` constructs one workspace module graph. It should not mean every
> declaration in every supplied file is ambiently visible everywhere.
> There should be no ambient union of user declarations.

A file matching on a type it never imported must fail name resolution.

## Owns

workspace discovery · module identities · explicit imports · local lexical
scopes · declaration lookup · type and constructor lookup · qualified names ·
duplicate definitions · visibility · unresolved and ambiguous names · import
cycles · cross-file source attribution

## Gate

```text
- Every accepted corpus import resolves through the workspace package graph.
- No accepted fixture relies on ambient or undeclared symbols.
- Every external implementation has an explicit interface declaration.
- User declarations are visible only through lexical scope, module membership,
  or explicit imports.
```

## The blocker, and how it was cleared

The corpus imported four modules that did not exist — `domain` (15 imports
across 12 files), `decode`, `capability`, `browser`. Strict resolution would
have failed all of them on day one.

The architect's ruling split them by ownership:

| module | owner | status |
|---|---|---|
| `domain` | the corpus | **written** — `examples/domain.pw`, 26 types |
| `decode`, `capability`, `browser` | the platform | E2C's signature manifests |

`examples/domain.pw` declares every type the corpus imports, so the corpus is
now an actual program rather than a set of excerpts. That is the point: imports,
visibility, constructor resolution and cross-file exhaustiveness get exercised
instead of assumed.

**No ambient opaque-symbol fallback.** `import domain.Store` must not succeed
because `Store` might exist somewhere. External *implementation* is allowed
through an explicit, versioned, content-hashed interface module; a missing
*declaration* is not:

> External implementation is allowed. Missing declaration is not.

## What the resolver found

**The corpus is not one program**, and it said so: five rejected fixtures reuse
module names with each other and 17 are reused across the buckets. The real
shape, which is also how the corpus is used:

```text
library + accepted/*        one program        (0 collisions)
library + one rejected/*    one program each   (a counterexample)
```

`just ci` and the test harness were both checking all 68 files as one program.
That was never a question anyone meant to ask, and the answer was 20 duplicate
declarations.

**Names need namespaces.** `A-003` declares `query Store` and imports
`type Store`; `A-008` declares `query Recommendations` and `view
Recommendations`. Neither is a mistake — a type, a data operation and a rendered
view are different kinds of thing. A single flat namespace called both
duplicates. Resolution is now keyed by `(Namespace, name)` with three
namespaces: `Type`, `Term`, `Ui`.

**A visibility keyword only worked on some declarations.** `private type X`
parsed as *two* declarations — a bare word and an unqualified type — so the type
looked public and any module could import it. The keyword is now re-parented
into whatever declaration follows it.

## The honest cost: 19/44 became 18/44

`R-004` was being caught **through** the ambient union. It materializes a
`session query Cart` declared in `A-004` — a different file it never imports —
and that is where its `Session` label came from. Remove the union and the label
does not propagate, so the fixture is silently uncaught.

Recorded rather than restored. The fix is the one the architect prescribed for
R-007: the fixture must import what it uses, and E2B must resolve *uses* as well
as imports so that failing to import is itself an error rather than silence.

## Still to do before E2B can close

- ~~R-007 must import `OrderState`~~ — **done.** `OrderState` moved into
  `examples/domain.pw` and both `A-002` and `R-007` import it, so an
  exhaustiveness fixture is no longer red for a resolution reason.
- ~~Dedicated resolution fixtures~~ — **done.** `examples/rules/resolution/`
  has all six, each with its own code, plus a valid case so the suite can tell
  a resolver from something that rejects every import.
- **Resolve uses, not only imports.** Today an `import` that names a missing
  module or name is an error; a *use* of an unimported name is silence. That
  silence is what leaves R-004 uncaught.
- **Hand `DefId` to the semantic analyses.** `check.rs` still matches some
  names textually. The architectural invariant is not met until it does not:

  > After E2B, semantic analyses consume `DefId`. They do not make semantic
  > decisions from textual names.
