# E2B — Program graph and name resolution

**Status: PARTIAL.** Architect ruling, 2026-08-06 — E2B stays open until
use-site resolution is *integrated*, not merely checked.

```text
module graph                complete
namespaces                  complete
import resolution           complete
declaration/use resolution  in progress
fixture isolation           in progress
```

I had marked this "gate met" on the strength of `pw check` reporting an
unresolved use. That was overstated: the check runs, but the downstream analyses
still compare source strings such as `"secrets.payments"` instead of consuming
the `DefId` resolution produces. Until they do, the graph exists beside the
checkers rather than underneath them.

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

## Use resolution, and the cost being repaid

Import checking alone was not enough. A *use* of an undeclared name was silence,
and silence is what let R-004 look uncaught rather than unresolved.

`pw check` now reports a qualified call whose head is not a module the file can
see. Making that true required the corpus to say what it uses:

```text
15 modules the corpus called and never imported
37 corpus files given the imports they were relying on ambiently
```

Two false positives were designed out rather than tolerated. A method call on a
value (`line.item_name`) is a field access, not a module path. And
`StoreError.DecodeFailed` is a *constructor* on a type in scope — both are
written `Head.member`, and treating the second as the first reported every union
constructor in the corpus as undeclared.

## The honest cost, and its repayment: 19 → 18 → 19

`R-004` was being caught **through** the ambient union. It materializes a
`session query Cart` declared in `A-004` — a different file it never imports —
and that is where its `Session` label came from. Remove the union and the label
does not propagate, so the fixture is silently uncaught.

**Now repaid.** R-004 imports `store.queries` and `cart.queries`, and a rejected
fixture's program is the library plus whatever accepted modules it imports —
because a counterexample may legitimately depend on a correct module. R-004 is a
page that *misuses* a correctly-declared session query, and the label that makes
it a violation comes from that query's own declaration.

Back to 19/44, and this time the label arrives through a declared import rather
than through an ambient union.

## Still to do before E2B can close

- ~~R-007 must import `OrderState`~~ — **done.** `OrderState` moved into
  `examples/domain.pw` and both `A-002` and `R-007` import it, so an
  exhaustiveness fixture is no longer red for a resolution reason.
- ~~Dedicated resolution fixtures~~ — **done.** `examples/rules/resolution/`
  has all six, each with its own code, plus a valid case so the suite can tell
  a resolver from something that rejects every import.
- ~~Resolve uses, not only imports~~ — **done.**
- **Hand `DefId` to the semantic analyses.** `check.rs` still matches some
  names textually. The architectural invariant is not met until it does not:

  > After E2B, semantic analyses consume `DefId`. They do not make semantic
  > decisions from textual names.
