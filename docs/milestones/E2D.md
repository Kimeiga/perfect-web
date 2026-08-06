# E2D — Source effect inference

**Status: FIRST SLICE LANDED.** Direct, helper-hidden and callback-hidden
propagation all work over `.pw` source, with the chain in the diagnostic. What
remains is the largest single group: geometry reads written as *property
access* rather than as calls.

## Why it was pulled forward

Architect ruling, 2026-08-06: it is the highest-leverage missing capability.
`docs/RISK_QUEUE.md` RQ-2 measured that Koka propagates effects through
unannotated higher-order code; this is `pw` owning the same property over its
own source, with its own spans.

## The three shapes, each tested

| shape | what it looks like | test |
|---|---|---|
| direct | the body calls the effectful thing | `a_direct_call_contributes_its_effect` |
| helper-hidden | the body calls a local helper that does | `an_effect_hidden_behind_a_helper_still_propagates` |
| callback-hidden | the effect is inside a lambda handed to a generic function | `an_effect_inside_a_callback_still_propagates` |

The third is the one an approximation gets wrong, and it is why the body grammar
parses lambdas properly rather than skipping them.

## Where the effects come from

`crate::signatures::Signatures`, and nowhere else. E2C's deletion gate forbids a
table of function names here, and the reason is the one that gate already
proved: a table can describe a function that does not exist.

## Two codes, because there are two questions

| code | question |
|---|---|
| `PW0400` | does the row name every effect the body performs? |
| `PW0401` | is this effect permitted where this declaration runs, whatever it declares? |

Declaring the effect fixes the first. It does not fix the second — a `view`
reaching the database is not repaired by admitting to it.

## What the corpus taught, again

**Three correct programs were reported as violations**, and each was a real
distinction I had collapsed:

- `A-008` — a `<stream query={..}>` region resolves *after* the shell is sent;
- `A-014` — an `on:press` handler runs when the user acts;
- `A-022` — a `post_paint` block runs on a later frame phase.

None of them performs the work *during render*. Charter §7.5A's frame phases are
exactly this distinction, and treating deferred work as render-time work is the
mistake the phases exist to prevent. `effects::deferred_spans` now models it.

`A-022` also exposed a grammar gap: `post_paint !{ post_paint, trace } { .. }`
did not parse as one statement, because a statement could not declare its own
effect row. The block became a sibling, so the work inside it looked like it
happened during render.

**`R-012` had a second, real defect** that masked the one it was written for.
Its `setup` performs `dom.mutate` and `layout.measure` and declared neither, so
E2D caught it — correctly, but not for the invariant the fixture specifies. The
fixture now declares those effects, leaving only the affine-resource leak that
E9C owns. A fixture that fails for the wrong reason is not enforced.

## Corpus: 19/44 → 20/44

`R-036` is the case E2D was built for — `layout.measure` reaching a pure view
through a helper that looks pure at the call site.

## What the remaining layout family needs

`R-032`, `R-033`, `R-034`, `R-035`, `R-037`, `R-038`, `R-040`–`R-043` all read
geometry as a **property**, not a call:

```text
anchor.offsetWidth
el.getBoundingClientRect().width
```

Inference walks calls. A property read resolves to nothing, so no effect is
found. The honest fix is E2B's open item — **member lookup against a type's
declared accessors** — so `Element.offsetWidth` can be declared with
`!{ layout.measure }` and read like anything else. Hard-coding a list of DOM
property names in the checker is what E2C's deletion gate forbids.

`R-001` needs its `fetch_store` declared; it currently calls a function that
exists nowhere.
