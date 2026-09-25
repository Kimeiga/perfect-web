# ADR-0036: the backend matches over `Option` and `Result`, reads fields, and builds variants

Status: accepted under the owner's instruction of 2026-09-24 ("finish the rest
of E10 … build a small kiokun slice"). Date: 2026-09-25. Milestone: E10.

## Context

After E10-I the component backend compiled straight-line bodies of import
calls. Every other construct was refused by name. The kiokun slice's lookup is
the first real body that needed more:

```text
match Entries.get(word) {
    Some(entry) => match entry.redirect {
        Some(target) => Entries.get(target),
        None => Some(entry),
    },
    None => None,
}
```

It reads a variant's case and binds its payload, reads a record field, builds
`Some(..)` and `None`, and joins two arms' values.

## Decision

### 1. `match` is structured in the IR

`Instr::Match { scrutinee, arms: [MatchArm { case, binding, body: Region }] }`.
Each arm is a region whose last value is the arm's value. The IR's
CFG terminators stay, but Wasm's control flow is structured, so a region is
what the encoder emits directly (`if`/`else`). No structure has to be
reconstructed from a graph. `Instr::Variant { case, payload }` builds
`Some`/`None`/`Ok`/`Err`, whose constructors have no `DefId`.

### 2. Lowering is bidirectional

`expr` takes the type its context fixes: the declaration's result, a
parameter's type, the arms' common type. `None` has no type of its own and
takes it from there. `Some`, `None`, `Ok` and `Err` are the language's own only
where the program declares no term of that name, which is the typer's rule.
`let x = e` binds, and so does `let _ = e`.

### 3. The encoder types what cannot type itself from its uses

A constructed variant's component type comes from where it is used: the
export's result, an import's parameter, or the enclosing match's result. The
type propagates backwards (`expected_types`). Every other value is typed where
it is made. Each move is checked against the use-site type with `same_type`:
a variant's payload, an arm's value, an argument, the result. This is the same
component-level identity check E10-I introduced, so two types that flatten
alike are still not interchangeable.

### 4. Layouts are `wit-parser`'s

- **Matching.** A match reads the 8-bit discriminant at offset 0 of the
  scrutinee's canonical layout. Each arm binds its payload as an address at
  `SizeAlign::payload_offset`.
- **Projection.** Projecting a field of a record in memory is address
  arithmetic from `field_offsets`. Projecting from a flat record takes the
  field's slice of locals.
- **Construction.** `Some(x)` allocates from the invocation region, stores the
  discriminant, and `memory.copy`s the payload when it has a layout, or stores
  its flat values (`store`, the inverse of `load`).

### 5. What is still refused, by name

- a nested pattern;
- a match that does not cover every case;
- a match over anything but `Option` or `Result`;
- a variant whose type nothing fixes;
- a declared record or variant constructed;
- a call to another compiled declaration.

## Findings

- **The checker does not check exhaustiveness over `Option` and `Result`.**
  `match get(w) { Some(x) => Some(x) }` passes `pw check`. The backend refuses
  it ("does not cover every case"), so no component is built with a missing
  arm. The checker should say so first (KNOWN_LIMITATIONS).
- **Refactoring the encoder changed no store component.** The committed
  `add_to_cart` and `clear_cart` are byte-identical (`evidence_is_current`).

## Acceptance

- **`compiler/pw-conformance`**, the one crate that links compiler and engine,
  and ships nothing:
  - `tests/variants.rs` runs the compiled lookup:
    - a stub followed;
    - an entry returned through a newly built `Some`;
    - a missing word, and a stub whose target is missing;
    - empty, 15 KB and emoji strings intact;
    - three refusals, each asserted by its exact reason.
  - `tests/oracle.rs`: every compiled declaration agrees with an independent
    reference ([oracle evidence](../evidence/E10/oracle-2026-09-25.md)).
- The whole kiokun shard: 16,921 entries looked up and rendered, 103 redirects
  followed, 0 failures ([kiokun evidence](../evidence/E10/kiokun-2026-09-25.md)).
