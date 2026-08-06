# ADR-0015 — The Koka backend lowers a pure subset, and what that does not prove

**Status:** Accepted
**Date:** 2026-08-05
**Milestone:** E2 (gate item 4)
**Depends on:** ADR-0001 (Koka is temporary), ADR-0011 (Koka is an effects-only
oracle), ADR-0014 (HIR)

## Context

Charter §14 M2 gate item 4: *"A simple domain function executes through
generated Koka."* Nothing in the repository lowers `pw` to anything, so the
front end has never been shown to produce something that runs.

The risk is not writing the backend. It is what the backend gets claimed to
prove afterwards. E0 already measured three facts that constrain this:

- **F-4** — Koka erases single-field `value struct`s at the JS boundary, so
  `Money_usd(350)` **is** `350`. Nominal identity does not survive.
- **F-8** — Koka enforces exhaustiveness only for functions declaring a total
  effect row, so a non-exhaustive match passes vacuously under `exn`.
- **F-6** — a multi-operation effect needs one `handler` block; chained
  `with fun op(..)` shorthands leave later operations unhandled.

ADR-0011 already concluded from these that `pw` owns value semantics and Koka is
an oracle for *effects only*.

## Decision

**Lower only the pure subset, and only from HIR.**

A declaration is lowerable when all of the following hold. Anything else is
skipped, and the skip is reported rather than silently omitted:

```text
fn        declares an empty effect row `!{}`
types     record and union declarations with monomorphic field types
exprs     literals, names, field access, calls to functions and constructors
          declared in the same module, arithmetic and comparison, match, if,
          let, blocks, parenthesised groups
```

Not lowered: effect rows other than `!{}`, generics, standard-library calls
(`List.map`, `List.fold`), templates, the body-level statement family, lambdas,
pipelines. Each has a reason to exist in `pw` that Koka would answer for us.

### Name mapping, verified against Koka 3.2.3 rather than assumed

| pw | Koka | note |
|---|---|---|
| `CartLine` (type) | `cart_line` | Koka type names start lowercase |
| `CartLine` (record constructor) | `Cart_line` | Koka derives it from the struct name, first letter capitalised |
| `Draft`, `Confirmed(x)` (union ctors) | unchanged | already capitalised |
| `Int` `String` `Bool` | `int` `string` `bool` | |
| `fn f(..) -> R !{}` | `pub fun f(..) : total r` | `total` is what makes F-8's vacuous pass impossible |

Every row was checked by compiling and running a probe module, not read from
documentation. Underscores are legal Koka identifiers; the kebab-case in the E0
spikes was a style choice, not a requirement.

**Every generated function declares `total`.** That is the whole point of
generating Koka at all: under `total`, Koka's own exhaustiveness and termination
checks are not vacuous, so compiling the output is a real second opinion on the
subset it covers.

## What this is, and is not, evidence for

**Is evidence for:** that `pw` source parses, lowers, and produces a program a
third-party compiler accepts and executes with the expected result. That is
gate item 4 and nothing more.

**Is not evidence for:**

- **that `pw`'s nominal types are enforced** — F-4 says they are erased. A
  `Money<USD>` that survives this path proves nothing about type identity.
- **that the effect system works** — only `!{}` functions lower, so the backend
  never exercises an effect row.
- **that Koka is the eventual backend** — ADR-0001 makes it temporary, and
  charter §14 M9 replaces it.
- **that the generated code is fast, idiomatic, or fit to ship.**

`docs/EVIDENCE_LEDGER.md` carries these limits alongside the claim, so the
distinction survives being quoted out of this file.

## Consequences

- E2 gate item 4 becomes answerable, with an explicit statement of coverage:
  how many corpus declarations lower, and why the rest do not.
- The subset boundary is enforced in code (`Lowerable`/`Skipped`), so growth is
  a deliberate edit rather than a drift.
- Cost: a second lowering to maintain until E9's own checker exists. Bounded by
  the subset, and deleted with ADR-0001.

## Revisit when

The `pw` checker reaches corpus parity (ADR-0001's deletion condition), or the
subset needs effects — at which point ADR-0011's effects-only reading of Koka
becomes the design question rather than this one.
