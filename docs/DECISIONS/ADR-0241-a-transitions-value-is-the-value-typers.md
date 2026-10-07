# ADR-0241: an optimistic transition's value is the value typer's

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-07.
Milestone: E14. A correction, found with ADR-0240.

## Context

- **A transition produces the value of the entry it replaces** (PW0331,
  the architect's ruling of 2026-08-11). The platform
  restores the value the entry held when the speculation is abandoned, so a
  transition producing another type puts something in the entry that nothing
  can mean.
- **`check.rs` read the transition's type through the older typer**
  (`infer.rs`'s `Types`), and an answer it did not have was no violation.
  That typer has no answer for a literal. So `optimistic Thing(x) as t =>
  "no"`, where `Thing` holds an `Int`, checked, with a body or without. A
  call's type it had: `count(cart)` over a `Cart` was refused.
- **The value typer types every expression** (`values.rs`, E9-V). It
  already gives a transition's binder its target's value, and relates what
  a transition calls. It related nothing about the transition's own value.
- **Two typers asked one question**, one able to answer it.

## Research

- **One fact, one detector** (ADR-0112, ADR-0239): a rule reported by two
  analyses is reported twice or, as here, decided by the weaker one.
- **An undecided relation is never agreement** (ADR-0031): what the value
  typer cannot type is counted by `pw audit-values`, not reported as either.

## Decision

1. **The value typer relates each arm's transition to its own target's
   value**: a `Transition` relation, its success type where the target is a
   `Result`. Disagreeing, it is PW0331, at the transition, as it read:
   "`c`'s optimistic transition produces `String`, but it targets a resource
   whose value is `Int`". The target is the related span, "this resource
   holds `Int`", and the repair names the binder: "produce an `Int` from
   `t`".
2. **`check.rs` keeps what the value typer does not say**: that a target is
   a resource's entry at all (PW0331's other two refusals). Its comparison of
   types is retired, so one fact has one detector.
3. **Undecided stays undecided**, counted by the audit.

## Acceptance

- **`compiler/pw-core/tests/transition_values.rs`, 4 tests:**
  - `=> "no"` over an `Int` is PW0331, with a body and without; the
    control, `t + 1`, checks;
  - `label(t)`, a call to a `String`, is PW0331 once, not twice;
  - the repair names the binder, and the target holds the value;
  - a target that is a call to a function, not a resource, is PW0331.
- **The store's relations are all decided** (`value_relations.rs`), its
  transitions among them.
- **Run whole, near what this changed or the value typer's own**:
  `clauses_read_once_mutations.py` 11 of 11 killed, `call_branch` 10 of 10,
  `clause_key` 10 of 10, `derived` 3 of 3, `one_checker` 5 of 5,
  `e9_value` 10 of 10, and `names` 20 of 20 after one survivor.
- **Two survivors, each found by a run and its test written:**
  - "a target that is no resource's entry is not refused", in this ADR's
    own controls: its one test was `clauses_read_once.rs`'s, which they do
    not run. `transition_values.rs` holds it now;
  - "a clause needs no value or block", in `names_mutations.py`, older than
    this ADR. Since ADR-0216 a function's body admits no clause, so the
    test, written in one, no longer asked the rule anything.
    `every_name_resolves.rs` asks it in a component's body now.
- **`scripts/transition_values_mutations.py`: 7 mutants**, recorded by
  `just e14-transition-values`.
- **`clauses_read_once_mutations.py`'s "an interface's transition need not
  produce its target's value"** is re-anchored as "an interface's
  transition's target need not be a resource's entry", what `check.rs` still
  reads of an interface's clause; its test gains that case.
- **The workspace, 2,142 tests**: 2,141 at the full check, and the one
  written for a survivor after it. **The browser suite, 782 in three
  engines.**

## Not claimed

- **A transition's value against a target the typer cannot type.** It is
  undecided, and counted.
