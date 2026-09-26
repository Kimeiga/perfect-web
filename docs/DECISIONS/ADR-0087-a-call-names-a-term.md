# ADR-0087: a call names a term

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1).

## Context

`resolve::Namespace` says what each kind of declaration is for:
- a view, a component, a page and a materialization are "rendered, never
  called";
- an event appears "only in `emits` and `invalidates_on`";
- an effect appears "only in an effect row".

The rule for a bare call did not hold to this. It asked whether the name
resolved in any namespace, and every name the file declares counted as
resolved. On 2026-09-26, at feafbce, each of these checked:
- `let v = Badge(1, 2)`, with `Badge` a view;
- `fn f() -> Int !{} { Badge(1) }`, a view returned where an `Int` is
  declared;
- `let e = CartChanged(s, 3)`, with `CartChanged` an event of one
  parameter;
- `let x = secret(1)`, with `secret` an effect the platform exports.

Every analysis answered for each call as for a call to nothing: no effects,
no label, no type. The value relations resolve a callee as a term or a type,
so none of these calls had its arguments or its result related.

## Decision

**A call names a term:** a function, a data operation, or a type it builds.
A call whose name resolves to no term and no type, but to a view, a
component, a page, a materialization, an event or an effect, is refused
(PW0027). The message says what the name is, and the explanation says how
that kind of declaration is used instead.
- **A binding keeps the name** (ADR-0066). A parameter or a local that
  shares a view's name is the value it holds, and calling it is a call
  through that value.
- **A term or a type keeps the name.** Each namespace declares a name once,
  so a function or a type may share a view's name. A call names the
  function, or builds the type.

## Acceptance

- **`compiler/pw-core/tests/calls_name_terms.rs`**, 5 tests, each with
  controls. The three that state a refusal fail at feafbce, the commit
  before. The two that state what keeps the name hold there too, and guard
  the change.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/call_name_mutations.py`,
  `just e10-call-names`, 3 mutants.
