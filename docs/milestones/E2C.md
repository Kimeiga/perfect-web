# E2C — Core library and platform contracts

**Status: FIRST SLICE LANDED.** Every module the corpus names is now declared,
with real effect rows. What remains is generating a signature *manifest* from
them and having the checkers read it instead of their internal tables.

## Why it exists

Three checkers had independently hard-coded the same facts:

| fact | where it was hard-coded |
|---|---|
| `secrets.payments()` yields a secret | `check.rs::introduced_restriction` |
| `secret.*` is origin-only | `placement.rs::worlds_for` |
| `database.read` is a database effect | `placement.rs::worlds_for` |

The architect's design replaces all three with one declaration:

```text
secrets.payments : () -> <secret.read<Payments>> Secret<Payments>
```

The effect checker reads the row. The privacy checker reads the return label.
The placement solver reads which worlds grant the capability. **Do not
independently hard-code those facts in three checkers.**

## What landed

```text
packages/pw-std/          decode, list
packages/pw-platform-web/ capability, browser, secrets, clock, style
examples/domain.pw        26 corpus-owned types
examples/services.pw      the application's own data services
examples/vendors.pw       external implementations, declared
```

Ten modules. The accepted corpus — 24 files — resolves and checks clean against
them, so the corpus is a real program rather than a set of excerpts.

The effect rows are the deliverable, not the bodies. `Stores.get` declaring
`!{ database.read }` is what will let E2D infer that a `view` calling it is not
pure, and it is already what E5's placement solver would need. `clock.now`
declaring `!{ clock.read }` is what makes a wall-clock read inside a shared
deterministic materialization (R-017) detectable at all.

`examples/vendors.pw` is the architect's external-interface rule in practice:

> External implementation is allowed. Missing declaration is not.

`VendorSdk.mount` declares `!{ dom.mutate, layout.measure }`. That row is why
A-024's audited escape is a *measurable* cost attributable to a named component
rather than an unaccounted-for one.

## Still to do before E2C can close

- **Generate a signature manifest** from these modules, so the checkers read a
  compiled artifact rather than re-parsing source.
- **Delete the stand-in tables.** `check.rs::introduced_restriction` and
  `placement.rs::worlds_for` must both come from the manifest. They are marked
  as the things to delete, and they are still there.
- **The minimal trusted intrinsics list**, kept explicit and tiny: primitive
  scalars, constructor mechanics, core effect-row operations, `match`.
  Everything else comes from a declaration.
- **Resolve uses, not only imports** (E2B). Until a *use* of an undeclared name
  is an error, declaring these modules is necessary but not sufficient.
