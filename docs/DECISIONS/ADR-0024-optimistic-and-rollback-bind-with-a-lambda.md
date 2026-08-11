# ADR-0024 — `optimistic` and `rollback` bind their subject with a lambda

**Status:** Proposed — awaiting the project architect's ratification of the
*surface syntax*. The semantic requirement below is already ruled and is not in
question.

**Date:** 2026-08-10

## The ruling this implements

Architect, 2026-08-10:

> `optimistic` and `rollback` are different: their contents are actual Pleris
> programs and must go through the full normal semantic pipeline.

and, on the binding:

> Do **not** fix it with `if policy == optimistic: inject magic variable named
> "cart"`. That recreates ambient framework convention inside the compiler. […]
> I'm not locking that surface syntax yet. I am locking the semantic
> requirement:
>
> > **Every identifier visible inside optimistic/rollback code must arise from
> > ordinary lexical scope or from an explicit binder in the construct itself.**

Two candidate surfaces were offered — `optimistic Cart as cart { .. }` and
`optimistic |cart| { .. }` — with the invitation to pick one that fits Pleris.

## The decision

```pleris
optimistic cart => cart.add(item, quantity)
rollback   cart => cart.remove(item)
```

The value is a **lambda**, and its parameter is the binder.

## Why this one

**It is not new syntax.** `x => e` already parses as `K::LambdaExpr`, and
`resolve::local_bindings` already collects a lambda's parameters. Adding
`optimistic Cart as cart { .. }` would mean a new node kind, a new binder form,
and a second place the language introduces a name — for a construct that already
has one. The alternative surfaces were rejected on that ground alone.

**It says the right thing.** An optimistic transition *is* a function from the
current value of a resource to the next one, and a rollback is its inverse.
Writing it as a lambda makes the type an ordinary one — `Cart -> Cart` — rather
than a relationship the compiler has to know about because of the keyword.

**It scopes correctly for free.** A binder in `optimistic` is not in scope in
`rollback`, because each policy value is its own expression. That fell out of
the lambda rather than being arranged: `check.rs` computes the term's scope from
`local_bindings_from(body, term)`, rooted at the term.

**It matches the language's existing idiom.** `{#each menu as item}` and
`resumable(captures = { item }) => add_to_cart(..)` are the two places Pleris
already introduces a name in an expression position. The second is a lambda.

## What is NOT decided here

- **The parameter's type.** Nothing checks that `cart` is the resource the
  command returns. The obligation is real — an optimistic transition over the
  wrong resource is a bug the compiler should catch — and it is recorded in
  `docs/NEXT.md` rather than implied by this ADR.
- **The execution context.** Architect ruling, same day: `optimistic` and
  `rollback` are execution contexts (`placement browser`, before the round trip
  and after a failure respectively), and the operations inside determine the
  effects. That model is not built. Until it is, an embedded term is resolved
  and **excluded from the declaration's effect row** — it is not reachable from
  the body's root, so no walk absorbs it. See `hir::Policy::term`.
- **Whether `cart.add` should resolve.** It does not, and that is the honest
  state: `Cart` is a resource with no declared operations. The lambda binds
  `cart`; what may be called on it is a separate question about resource
  interfaces.

## The alternative that was rejected outright

Injecting a binding named after the policy — the compiler supplying `cart`
because the keyword is `optimistic`. It was named in the ruling and it is the
shape `docs/RISK_QUEUE.md` has recorded five times: meaning resolved from a
spelling. It would also have made the corpus check clean while proving nothing,
which is what the whole 2026-08-10 sequence exists to stop.

## Consequence for the corpus

`examples/store/app.pw`, `examples/accepted/A-005`, `examples/rejected/R-029`
and two `examples/generality/optimistic_no_rollback/` witnesses wrote
`optimistic cart.add(item, quantity)` with `cart` bound by nothing. All five gain
the binder. No verdict moves — `R-029` is still caught for `optimistic` without
`rollback` — which is what says this is a binding repair and not a change to
what any fixture is about.
