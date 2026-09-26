# ADR-0067: a record is built with each of its fields, once

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Decisions marked **(ruling needed)** were made without a
ruling and are offered for reversal. Date: 2026-09-26. Milestone: E10
(charter §7.1, the value checker).

## Context

A probe for wrong programs that pass `pw check`, run after ADR-0066, found
two more. On 2026-09-26 each of these checked:
- **A record built without one of its fields:** `Box { value: n }`, where
  `Box` also declares `label`.
- **A field the record's type does not declare:** `Box { value: n, label:
  "a", colour: "red" }`.
- **A field given twice:** `Box { value: n, value: 2, label: "a" }`.
- **An `if` without `else` as a body's value:** `fn f(n: Int) -> Int { if n >
  0 { 1 } }`.

A record built by its fields' names was related field by field, against the
fields it was given. A field its type lacks was skipped, and nothing asked
which declared fields it left out, or gave twice. The backend was the first
to refuse each ("built with its field `label` 0 times").

An `if` without `else` had no stated type, so a body ending in one was
undecided. The backend gives it the unit value whichever way it goes
(ADR-0051).

## Decision

### 1. A record's fields are a relation of their own (PW0612)

A record built by its fields' names is related to the fields its type
declares. Each of these is PW0612, "a record must be built with each field
its type declares, once, and no other":
- a field its type does not declare, at that field;
- a field given more than once, at its second;
- a declared field left out, at the construction.

A construction with exactly its fields is one agreeing relation, so the audit
counts it. A shorthand field, `Box { value: n, label }`, is given.

A record built positionally, `Box(1, "a")`, is a call, and its arity is
PW0604's, as before.

### 2. An `if` without `else` is the unit value

Its value is `()` whichever way it goes, as the backend has it. So a body
that ends in one is refused where it declares another result (PW0606). As a
statement it is unchanged.

## Acceptance

- **`compiler/pw-core/tests/record_fields.rs`**, 5 tests:
  - a field left out;
  - a field the type lacks;
  - a field given twice;
  - a shorthand field, the control that it counts as given;
  - an `if` without `else`, refused as a value and not as a statement.

  Four fail at 3ae67d9, the commit before. The fifth guards shorthand.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean. Their audits change only
  by one agreeing fields relation per record they build: 1 in the store, 6
  in kiokun, 3 in the corpus.
- **The store's and kiokun's artifacts are byte-identical,** apart from
  ADR-0058's two handlers.
- **Mutation controls:** `scripts/record_field_mutations.py`,
  `just e10-record-fields`, 5 mutants.
