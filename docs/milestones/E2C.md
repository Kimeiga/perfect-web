# E2C — Core library and platform contracts

**Status: DELETION GATE MET.** Every module the corpus names is declared, and
the privacy checker reads the resolved signature instead of its own table. The
gate, verbatim, is now a CI test:

> No privacy, placement, or effect checker contains a built-in mapping from
> library function names to labels, effects, capabilities, or worlds.

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

## What the deletion exposed

`check.rs::introduced_restriction` is gone. `compiler/pw-core/src/signatures.rs`
builds one `Signature` per resolved declaration, and the privacy checker reads
the declared return label from it.

The deletion immediately found that **`current_organization()` did not exist**.
The rule for R-005 — a shared cache keyed without its tenant — worked only
because the table asserted what that function returned. The function itself was
never declared anywhere. It is now `packages/pw-platform-web/context.pw`, with
`Session<S>`, `User<U>` and `Organization<O>` as declared types, and R-005
imports it.

That is the argument for the gate in one example: a table can describe a
function that does not exist, and nothing notices.

A label now comes from **what a function returns, not how it is spelled**. A
test asserts both directions — a function called `innocuous_name` returning
`Secret<Signing>` carries the secret, and one called `scary_sounding_secret`
returning `Int` does not.

## What remains a compiler fact, deliberately

`placement.rs::worlds_for` stays. It maps a *capability family* to worlds —
`secret` is origin-only — which is a property of the deployment topology that
charter §1.7 states directly, not a fact about a library function. It becomes a
declaration at E8, when WIT worlds make the topology explicit.

## Still to do before E2C can close

- **Generate a compiled manifest**, so checkers read an artifact rather than
  re-parsing source. The signature layer is in place; only its serialization is
  missing.
- **Effect inference and placement must consume `Signature` too.** Privacy does;
  the other two still read declaration headers directly. That is E2D's work.
- **The minimal trusted intrinsics list**, kept explicit and tiny: primitive
  scalars, constructor mechanics, core effect-row operations, `match`.
  Everything else comes from a declaration.
- **Resolve uses, not only imports** (E2B). Until a *use* of an undeclared name
  is an error, declaring these modules is necessary but not sufficient.
