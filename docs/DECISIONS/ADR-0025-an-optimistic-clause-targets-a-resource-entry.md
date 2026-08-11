# ADR-0025 — an optimistic clause targets a resource entry, and its reversal is derived

**Status:** Accepted

**Date:** 2026-08-11

**Supersedes:** [ADR-0024](ADR-0024-optimistic-and-rollback-bind-with-a-lambda.md),
which was never accepted.

## The decision

```pleris
optimistic Cart(current_session()) as cart =>
    Carts.with_line(cart, item, quantity)
```

Three pieces of meaning, and they are different questions:

```text
Cart(current_session())   which resource ENTRY is speculatively updated
cart                      a lexical name for its current value
Carts.with_line(..)       a PURE transition producing the speculative value
```

There is no `rollback`. Writing one is `PW0327`.

## The ruling

Architect, 2026-08-11:

> An optimistic clause identifies a resource entry and binds its current value;
> its body is an ordinary Pleris transition expression.

and, on the reversal:

> "an optimistic transition is a function from the current value to the next,
> and rollback is its inverse" — the first half is excellent. The second is
> generally false. […] Pleris already knows the exact pre-optimistic resource
> value/version. Making the programmer describe the inverse is precisely the
> kind of redundant mechanism the project is trying to eliminate.

and, on purity:

> The desired shape is `Cart → Cart`, not `Cart → arbitrary client program with
> arbitrary effects`. […] That gives automatic rollback meaningful semantics. An
> arbitrary externally visible effect cannot generally be undone by restoring
> the resource value.

## Why the target is an entry and not a type

`Cart` names a type. Two entries can have it:

```text
Cart(session A)
Cart(session B)
```

A transition that named only the type would not say which one it changed, and a
command may eventually affect several resources. Pleris already has the stronger
notion — resource declaration, logical key, privacy partition, compatibility
generation — and an optimistic update should target that object.

It scales without ambiguity:

```pleris
optimistic Cart() as cart => ..
optimistic Inventory(item.store) as inventory => ..
```

Once a resource owns its session partition automatically, the key can be
implicit — `Cart()` — but the entry is still what is named.

## Why there is no written inverse

A cart holding `Apple × 3`, optimistically `+2`, is `Apple × 5`. A hand-written
`remove(apple)` does not restore `Apple × 3`, and `subtract(2)` stops restoring
it as soon as there are concurrent updates, normalization, server
reconciliation, or derived fields.

The runtime holds the value it displayed. Restoring it is exact; replaying an
inverse is an approximation the author was being asked to get right.

```text
before = the held resource state/version

optimistic transition(before)  → speculative state displayed
command succeeds               → authoritative result reconciles it
command fails                  → restore before
```

*How* the runtime restores — a retained immutable value, structural sharing, an
inverse patch it generates, a version restore — stays an implementation choice.
The author states the meaning: **current state → speculative state.**

If custom rejection reconciliation is ever genuinely needed, it is a separate
feature and it must not be called an inverse. The architect sketched
`on_reject (before, optimistic, error) => ..`; nothing is built.

## Why the transition must be pure

`PW0330`. The transition runs on the client, before the round trip, and is
abandoned by restoring the previous value. An externally visible effect cannot
be abandoned that way, so a transition that performs one has no defined
behaviour on rejection. Automatic restoration is only meaningful over a pure
function.

The transition is its own **execution root** with its own inferred effect row:

```text
command body root       → the command's effects
optimistic transition   → the transition's effects, checked against its
                          context's restrictions
```

Neither absorbs the other's. That is what makes the rule checkable at all — a
transition merged into the command's row would be indistinguishable from the
command performing the effect itself. `Inference::infer_rooted` is the entry
point; `hir::Transition` holds two `ExprId`s that no walk from `Body::root`
reaches.

## What is NOT decided here

- **The transition's return type is not checked against the target's value
  type.** It must be, and it is recorded in `docs/NEXT.md`.
- **`ExecutionContext` is not a type yet.** The purity restriction is applied
  directly rather than derived from a context declaring `placement browser` and
  an empty allowed-effect set. When the context model lands, this rule should be
  one instance of it — the same lesson as the frame phases.
- **The implicit form `Cart()`** is accepted by the grammar but nothing yet
  supplies the session partition, so the corpus writes the key.

## Consequence for the corpus

`examples/store/app.pw`, `examples/accepted/A-005`,
`examples/rejected/R-029` and two generality witnesses. Every `rollback` clause
is deleted, and every `optimistic` gains a target.

**R-029's subject changed**, which is the largest consequence and is recorded in
`docs/CORPUS.md` as C6. It demonstrated `optimistic` without `rollback`; that
invariant is retired, and it now demonstrates an impure transition. The
`@category` is unchanged because the failure mode is unchanged — an optimistic
update that cannot be cleanly abandoned leaves the UI permanently inconsistent
with the server — and only the mechanism moved. The
`examples/generality/optimistic_no_rollback/` witnesses moved to
`optimistic_not_pure/` with it.

`PW0327` was reused rather than retired: same code, same declaration, opposite
verdict. A reader looking it up finds what replaced it instead of a dead entry.

**And `Carts.with_line` was declared**, because the repair exposed that
`cart.add(item, quantity)` called a member `Cart` does not have — `Cart` is a
record of `lines` — and the effect the analysis attributed to it came from
`Carts.add` through the scoped last-segment member fallback. A three-argument
origin write standing in for a two-argument client-side transformation.
