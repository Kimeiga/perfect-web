# ADR-0079: a function value carries the label of what it makes

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.8,
privacy labels).

## Context

ADR-0078 found an effect travelling through a function value unseen. A
probe of the privacy labels on 2026-09-26, at 936a7ec, found the same route
for a secret:
- **A name meaning a declaration was public.** The labels read a name's
  binding (ADR-0063), and a name no binding holds was public. So
  `let f = secrets.payments`, or `let f = payments` with `payments`
  imported by name, held a public value.
- **A call through a value was labelled by its arguments alone.** A call
  whose callee no declaration states is labelled by what goes into it: its
  arguments, a piped value and a receiver (ADR-0064). The callee itself was
  left out.

So each of these logged a `Secret<Payments>` publicly and passed:
- `f()`, where `f` holds `secrets.payments`, either spelling;
- `b.f()`, where `b` is a record whose field `f` holds it.

A helper whose declared result is the secret was already refused: its
signature carries the label.

## Decision

- **A declaration named as a value is labelled by what calling it makes**:
  its signature's label. A lambda is labelled by its body (ADR-0064), and a
  named function is the same thing written elsewhere. That covers a bare
  name, and a module's path, `secrets.payments`.
- **A call through a value carries its callee's label**, joined with what
  it already carried.
- **A binding in scope is its own value**, whatever declaration shares its
  name.

## Acceptance

- **`compiler/pw-core/tests/labels_through_values.rs`**, 2 tests, each
  with controls. The first fails at 936a7ec, the commit before. The second,
  that a binding named like a secret's source is its own value, holds there
  too, and guards this change.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean. The artifacts are
  byte-identical, apart from ADR-0058's two handlers.
- **Mutation controls:** `scripts/label_value_mutations.py`,
  `just e10-labels-through-values`, 3 mutants. The lexical control that
  reads a name's label through its binding is re-anchored to the new match.
