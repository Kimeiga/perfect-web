# ADR-0085: a call carries what it is given

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.8,
privacy labels). Corrects ADR-0064 §1.

## Context

The charter asks for "an explicit, conservative label system" that tracks
"joins through data flow" (§7.8). ADR-0064 found that a declared function
read its silence about labels as "public", and carried a label only where
the result mentions a type parameter the argument brings in. A function over
plain values mentions none, so it still read its silence that way. Found
writing ADR-0084, on 2026-09-26 at 193ac5b, each of these logged a
`Secret<Payments>` publicly and passed:
- `String.trim("with {secrets.payments()}")`;
- `String.to_upper(..)`, `String.slice(.., 0, 3)` and
  `String.join([..], ",")` over the same;
- a program's own `fn echoed(text: String) -> String`.

The same rule made an element a secret index chose public, which
KNOWN_LIMITATIONS recorded. It also made the length of a list of secrets
public, which ADR-0064 chose.

## Decision

- **A declared call's result carries what it is given.** It is labelled by
  the declaration's label, joined with the label of each argument given to a
  parameter whose declared type states no label of its own.
- **A parameter that states a label keeps its contract.** A parameter
  declared `key: Secret<Payments>` receives the secret by contract, and the
  declaration's label says what comes out. `Payments.capture`'s receipt is
  not the key, and A-010 checks as before.
- **Consequences, each more conservative than before:**
  - the length of a list of secrets carries their label, where ADR-0064
    kept it public;
  - an element a secret index chose carries the index's label;
  - a list mapped to constant words carries the list's label.
- **`x |> f(..)` is labelled as the call**, which gives `x` to its first
  parameter. Before, the pipe also joined `x` on its own, so a secret piped
  to a parameter declared to receive it lost the contract it keeps when
  passed in place.
- **The receiver join is gone.** ADR-0079 joins the callee's label, and a
  method call's callee, `tokens.get`, carries its receiver's, so ADR-0064's
  separate join of the receiver was redundant.

**(ruling needed)** A function that should make public data from a secret,
a digest or a masked number, has to take the secret in a parameter that
states its label. The alternative is still label polymorphism in
signatures, as ADR-0064 said, or an audited declassification like the
escape hatches' `because "…"`.

## Acceptance

- **`compiler/pw-core/tests/labels_through_plain_values.rs`**, 3 tests,
  each with controls. All three fail at 193ac5b, the commit before.
- **ADR-0064's tests.**
  - The test that the length of a list of secrets keeps its contract now
    states the opposite.
  - A test that a piped list of secrets carries its label is added.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean. The artifacts are
  byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:**
  - `scripts/label_plain_mutations.py`,
    `just e10-labels-through-plain-values`, 3 mutants;
  - ADR-0064's control is reworked: its contract mutant is re-anchored to
    the new join, and the two mutants this change makes meaningless, a
    result's type parameters and the receiver's own join, are retired;
  - ADR-0079's callee control is re-anchored.
