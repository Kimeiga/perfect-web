# ADR-0251: a resource's clauses are held to PW2005

Status: accepted under the owner's delegation of 2026-10-02; a correction
ADR-0250's tests found. Date: 2026-10-07. Milestone: E14.

## Context

- **A `resource` declaration's clauses were walked by no affine rule.**
  Its `acquire { .. }` and `release(h) { .. }` are terms, lowered into the
  declaration's arena but reached from no statement (`lower::policy_blocks`),
  as a policy's terms are, so that the declaration's effect row does not
  absorb them. PW2005 walked the body from its root, and A-007's body is an
  empty block: its clauses were never looked at.
- **So a `release` that never ended its handle passed**, in either form.
  A-007 promises "Acquire and release are paired by the compiler"; nothing
  held `release(handle) { () }` to it, as a declaration's clause or as a
  component's `resource` statement (A-019), whose clauses are statements
  the check did walk but whose `release` it never asked anything of.
- **The pieces were there.** A clause's binder is bound (`lexical`:
  `Binder::Term` for a declaration's, `Binder::Clause` for a statement's,
  with the `acquire` block it names), and typed as what `acquire` makes, its
  success where it can fail (ADR-0047; `values::term_binders`).

## Decision

1. **PW2005 walks every term root**, a declaration's clauses among them
   (`affine::reach`), for acquisitions, bindings and releases, as it walks
   the body.
2. **What an `acquire` clause makes, the resource holds**: an acquisition
   that is the clause's value, through blocks, branches, `?`, `Ok(..)` and
   `Some(..)` (ADR-0250), and a name bound in the clause that is its value,
   which moves to the resource. Another clause's value, a `release` clause's
   or a key's (`emits CartChanged(..)`), nothing reads: an acquisition there
   is refused, "acquired, and its clause drops it".
3. **What a `release` clause is given, it ends exactly once on every
   path** (`affine::released_clauses`), where its resource's `acquire`
   acquires it: `T` of `resource.acquire<T>` in a row the `acquire` clause
   calls. A resource whose `acquire` acquires nothing (A-024's
   `VendorSdk.mount`) owes nothing. The faults are PW2005's: not on every
   path, twice, in a loop or a function value, given to a function value.
   The diagnostic says "`handle` is what its resource's `acquire` made, and
   this `release` must end it".
4. **An alias in a clause is read** (`affine::alias_of`), `let end =
   Maps.destroy` then `end(handle)`, as in a body (ADR-0080).

## Acceptance

- **`compiler/pw-core/tests/resource_clauses.rs`, 7 tests**, each in both
  forms (A-007's declaration and A-019's statement) where it applies, with
  its control: a `release` that ends its handle, and one that ends nothing;
  ended twice, and on one branch; a `release` owing nothing where its
  `acquire` acquires nothing; an acquisition a `release` clause's statement
  drops, and one that is its value; an acquisition in a command's `emits`
  key; an alias in a `release` clause; a name an `acquire` clause binds,
  moved to its resource, and one not its value.
- **The whole workspace's tests**, the accepted corpus among them: A-007,
  A-019 and A-024 check clean.
- **`scripts/resource_clauses_mutations.py`: 14 mutants**, recorded by `just
  e14-resource-clauses`: 14 of 14 killed.
- **The chain runs on the push** (ADR-0245).

## Not claimed

- **A clause's own value beyond its resource.** A `draw` clause's or a
  transition's value is read by its runtime; an acquisition there is
  refused, not followed into it.
- **What ADR-0250 does not claim**: a resource matched where it is made, kept
  in a value, or acquired in a keyword's block without a name.
