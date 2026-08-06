# E2B — Program graph and name resolution

**Status: NOT STARTED.** Inserted by architect ruling, 2026-08-06, ahead of
E2C and E2D. Its first blocker is cleared.

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

## Still to do before E2B can close

- **R-007 must import `OrderState`.** It currently relies on the ambient union
  to reach its exhaustiveness defect. Name-resolution failure must not be what
  makes an exhaustiveness fixture red.
- **Dedicated resolution fixtures**, so resolution defects stop being smuggled
  into tests about other things:

```text
examples/rules/resolution/
├── unresolved-import
├── unresolved-name
├── ambiguous-import
├── private-declaration-access
├── duplicate-declaration
└── invalid-module-cycle
```
