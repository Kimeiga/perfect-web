# ADR-0214: an opaque type's own module reads its representation as `.value`, and declares no member of that name

Status: accepted under the owner's delegation of 2026-10-02. It fixes
ADR-0210's urgent defect 8, and records assumption A-020 as ruled (0031-c).
Date: 2026-10-05. Milestone: E14.

## Context

- **`.value` reads an opaque type's representation, in its module**
  (ADR-0048).
- **`browser` declared `fn value<T>(snapshot: LayoutSnapshot<T>) -> T`**, the
  accessor A-015 to A-017 read a snapshot through. In `browser`,
  `snapshot.value` named both, and the member was read in the
  representation's place.
- **A module that wrote `fn value(p: PositiveInt) -> Int { p.value }`**
  would call its own member, for ever.

## Decision

- **An opaque type's own module declares no member named `value`**, PW0626.
  Another module's member of that name is its own business, since the
  representation is not readable there. So is a record's module's, since a
  record has none.
- **`LayoutSnapshot`'s accessor is `measured`**: `box.measured.left`. A-015
  to A-017 and assumption A-020 read it so. A-020 is ruled (0031-c): a
  snapshot is read explicitly.

## Acceptance

Recorded by `just e14-opaque-value` in `docs/evidence/E14/opaque-value.txt`:

- **`compiler/pw-core/tests/members.rs`**:
  - `value` in an opaque type's module is PW0626;
  - another name, a record's module and another module's `value` are not;
  - a snapshot is read by `measured`.
- **The accepted corpus checks**, A-015 to A-017 included.
- **`scripts/opaque_value_mutations.py`**: 2 mutants.
