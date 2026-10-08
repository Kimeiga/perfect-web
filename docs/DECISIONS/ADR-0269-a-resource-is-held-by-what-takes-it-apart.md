# ADR-0269: a resource is held by what takes it apart

Status: accepted under the owner's delegation of 2026-10-02; found by
ADR-0250 and queued in NEXT ("a resource matched where it is made").
Date: 2026-10-08. Milestone: E14.

## Context

- **PW2005 follows an acquisition to what holds it** (ADR-0250): a name, a
  call that releases it, the caller, a resource's `acquire` clause. A
  `match` on it was refused: "matched where it is acquired, and what the
  arms bind is not followed".
- **Three correct programs were refused**, each a `Result` or an `Option`
  carrying what a call acquires, taken apart:
  - `match Maps.create(c, at) { Ok(h) => Maps.destroy(h), Err(_) => () }`:
    the refusal above;
  - `let r = Maps.create(c, at)`, then `match r { .. }` the same way: "not
    consumed on every path", since a release of `h` counted for nothing,
    and the `Err` arm, which holds no handle, owed one;
  - `let r = Maps.create(c, at)`, then `let h = r?` and `Maps.destroy(h)`:
    the `?`'s failure counted as an exit owing a release, and `h`'s release
    counted for nothing.

  `let h = Maps.create(..)?` was the one form that passed.
- **Rust**: "By default, identifier patterns bind a variable to a copy of or
  move from the matched value", and the wildcard, "Unlike identifier
  patterns, it does not copy, move, or borrow the value it matches" (The
  Rust Reference, Patterns): an arm's name takes what it matches, and `_`
  leaves it. **Austral**, a language of linear types: linear variables
  "must either be consumed in every clause or appear zero times in every
  clause" of a `case` (F. Borretti, "How Austral's Linear Type Checker
  Works"), as PW2005's paths already count.
- **A last segment names nothing.** The architect's ruling of 2026-08-07
  forbids resolving meaning from a path's last segment
  (`tests/last_segment.rs`); this ADR's first version read `Result.Ok` so,
  and the audit refused it.

## Decision

1. **What carries a resource is held by what takes it apart.** A value a
   call answers, a `Result` or an `Option` carrying what the call's row
   acquires, is held by the name an arm binds it to, `Ok(h)` or `Some(h)`,
   or by `let h = r?`; the name must end it exactly once on every path, as
   a binding must.
2. **Where it is not, there is nothing to end.** An arm that meets no
   carrying case, `Err(..)`, `None`, or an arm after one that took the
   carrying case whole, owes no release; nor does a `?`'s failure.
3. **An arm that drops it is refused where it does**: `Ok(_)`, or a `_`
   that a carrying case reaches, "bound to `_`, and nothing can release it"
   (ruling 0099-a); and a name or an or-pattern that holds the value whole,
   "matched by a name that holds it whole, which nothing follows". A
   binding's arm is reported at the arm, `Ok(_)`.
4. **The cases are named exactly**: `Ok`, `Result.Ok`, `Some` and
   `Option.Some`, as `check` names them, never by a last segment.
5. **A match on what carries nothing is refused as before**: a bare
   handle, `match tx { .. }`, is "matched where it is acquired".

## Acceptance

- **`compiler/pw-core/tests/matched_resources.rs`, 6 tests**, each with its
  control: a match on the acquisition, ended in its arm, in either order
  and with a `_` that meets only the failure, and not ended or ended twice
  there, or left by a failing `?` before it is ended; `Ok(_)`, a `_` that meets the handle, and a name that holds it
  whole, refused; a binding matched, ended, not ended, dropped by `Ok(_)`,
  and taken apart twice; a binding taken out by `?`, ended and not; and an
  `Option` a helper gives its caller from its own match, taken apart with
  `Some(h)` and dropped with `Some(_)`.
- **`compiler/pw-core/tests/let_discard.rs`**: the match ADR-0250 pinned as
  refused is clean, and `Ok(_)` is refused, the repair naming `?` and only
  `destroy`.
- **`scripts/matched_resource_mutations.py`, 11 mutants**, recorded by `just
  e14-matched-resources`.
- **The six scripts over `affine.rs`, each run whole**, four mutants among
  them re-anchored to the code this changed.
- **The whole workspace's tests**, the corpus and the generality witnesses
  among them.

## Not claimed

- **A bare handle matched**, `match h { x => Maps.destroy(x) }`, is not
  followed into its arm: an arm's name holds only what a carrying case
  carries.
- **A carrying value nested in another**, `Result<Option<MapHandle>, E>`,
  or bound by an or-pattern in each alternative, is refused rather than
  followed.
- **A carrying value given to another function, or kept in a list or a
  record**, is followed as before: it is "taken" or "contained".
- **An arm that gives what it binds to the match's own value**, `let o =
  match Maps.create(..) { Ok(h) => Some(h), .. }`, is refused as not ending
  it there: the value the match makes is not followed. Where the match is
  the body's value, the caller is given it, and it passes.
