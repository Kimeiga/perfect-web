# E2D — Source effect inference

**Status: COMPLETE.** Direct, helper-hidden and callback-hidden propagation all
work over `.pw` source, with the chain in the diagnostic. Geometry reads written
as property access are visible via E2B member lookup, charter §7.5A's frame
phases are checked, and the corpus stands at **33/44 with zero wrong-reason
catches**.

Effect inference does not reach every analysis: the placement solver and the
privacy flow still read *declared* rows. That is recorded as an open E2B/E2C
item, not as part of E2D — see `docs/evidence/P0/readiness.txt`.

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

## Four codes, because there are four questions

| code | question |
|---|---|
| `PW0400` | does the row name every effect the body performs? |
| `PW0401` | is this effect permitted where this declaration runs, whatever it declares? |
| `PW0402` | is it happening in a frame phase that permits it? |
| `PW0403` | does an observation cause the change it observes? |
| `PW0404` | does a subtree declared independent depend on something outside it? |

Declaring the effect fixes the first. It does not fix the second — a `view`
reaching the database is not repaired by admitting to it. The last two are not
effect-row violations at all, which is why they are not aliased onto one:
`PW0403` is a relation between an observation and a write, and `PW0404` is a
declared assertion the code contradicts.

An effect's **type argument is part of its identity**.
`style.mutate<LayoutAffect>` and `style.mutate` are different effects, because
one can invalidate layout and the other cannot; naming them the same would make
the distinction a comment. `EffectRef` therefore carries both the family path
and the written form. The same is true of `clock.wall` versus `clock.read`: a
monotonic read measures a duration, and forbidding the whole family in a
build-time page would have caught the corpus fixtures while stating something
false.

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

## Corpus: 19/44 → 33/44

`R-036` is the case E2D was built for — `layout.measure` reaching a pure view
through a helper that looks pure at the call site. Then E2B's member lookup
added `R-032` (`anchor.offsetWidth`) and `R-037` (a callback measuring layout
inside `List.map`), and the frame-phase intrinsics added `R-033`.

**Frame-phase keywords carry their own effect.** `measure { .. }` reads geometry
by definition, whatever it calls inside. That is a *language* fact, not a
library one — a language that did not know it would need a library function to
explain its own keyword — so it sits in `effects::intrinsic_effect`, four
entries, each a phase keyword the grammar already reserves.

## The layout family, and why it is a separate module

Member lookup made the *reads* visible. What was left was not about seeing an
effect — every effect in these five is individually legal — but about how they
relate. `compiler/pw-core/src/layout.rs` owns that:

| file | invariant | shape |
|---|---|---|
| `R-034` | a layout read *after* a layout-affecting write, in one frame | ordering |
| `R-035` | a layout write *during* the measure phase | phase |
| `R-038` | a resize observer whose handler invalidates what it observes | cycle |
| `R-040` | a layout-affecting operation inside a compositor animation | capability |
| `R-041` | `dom.mutate` inside a painter | context |
| `R-042` | forcing layout after paint, in the same frame | phase |
| `R-043` | containment with a cross-boundary layout dependency | false assertion |

**Every one has an accepted twin already in the corpus** — A-018, A-020, A-021,
A-023 — differing in exactly the one way the rule is about. That pairing is the
negative control `docs/RISK_QUEUE.md` asks for. A rule that banned `observe
resize`, or `animate`, or `subtree independent` outright would catch the
rejected file and fail its twin. All 24 accepted files check clean.

Two of the rules are stated in the platform package rather than in the compiler:

- A property is **compositable** exactly when its setter declares
  `animation.composite`, so `style.pw` gained `set_transform` and `set_opacity`.
  The compiler holds no list of property names, and the diagnostic's list of
  permitted properties is read from the signature table — adding one to the
  platform updates the message.
- Reaching the document is `dom.read`/`dom.mutate`, described by `document.pw`.

An animated property is told from an animation *parameter* structurally: the
animated ones are written as transitions, and `duration: 180.milliseconds` is
not. So `duration` needs no exemption and no list either.

A **painter** is identified by `paint.custom` in its row, not by a declaration
kind — `paint X(..)` lowers as an ordinary declaration.

## Four fixtures were silent for a reason that was not the checker

`R-001` called `fetch_store`, `R-041` called `document.query`, and `R-004` and
`R-012` used modules they never imported. In each case the call resolved to
nothing, so no effect was inferred and a rule that already existed could not
fire. Each was repaired by giving the fixture the declaration it calls.

This is worth stating plainly because it looks like the opposite of what it is:
a fixture that produces no error can mean the checker is missing, or it can mean
the *program* is missing something. `R-025` is the same shape and is
deliberately **not** repaired — its second half is a real E5 connection (the
placement rule reads declared rows, not inferred ones), and repairing the import
would leave it silent anyway.
