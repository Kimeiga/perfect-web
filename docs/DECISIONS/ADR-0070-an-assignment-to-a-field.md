# ADR-0070: an assignment to a field has the field's type

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.1, the
value checker).

## Context

`b.value = "wrong"`, where `b` is a `Box` whose `value` is an `Int`, passed
`pw check` on 2026-09-26. An assignment was related to its target's type only
where the target is a name (ADR-0051), and an assignment to a field related
nothing. The backend refuses an assignment to a field by name, so the program
never ran, and the checker said nothing about why it is wrong.

## Decision

An assignment whose target is a field read, `b.value = e`, relates `e` to the
field's type, as `x = e` relates `e` to `x`'s (PW0607). The field's type is
the one a read of it has, under the record's instance.

Not decided here: whether a field of a binding not declared `let mut` may be
assigned. The backend compiles no assignment to a field.

## Acceptance

- **`compiler/pw-core/tests/field_assignment.rs`**, 1 test with a control. It
  fails at 34d1511, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/field_assignment_mutations.py`,
  `just e10-field-assignment`, 1 mutant.
