# E10 gate item 5 and task 10, 2026-09-25

Raw output: [ownership.txt](ownership.txt), produced by `just e10-ownership`.

## Gate item 5: no ownership syntax for ordinary application values

> Keep ordinary source free of borrow/lifetime syntax. Expose affine
> annotations only for scarce resources where they express a real invariant.
> — charter §14 M10 tasks 6 and 7

In Pleris a value is affine because its producer's effect row declares
`resource.acquire<T>`, as `Database.begin()` does. That is a fact about one
library function, and nothing is written where the value is used.
`compiler/pw-core/tests/ownership.rs` checks this against the two programs
`just ci` checks, rather than stating it:

| check | result |
|---|---|
| what any signature acquires, in the store and in the accepted corpus | `DatabaseConnection`, `DatabaseTransaction`, `MapHandle`: a connection, a transaction, an imperative widget's handle |
| the store's own declarations (`store.page.*`) | 10 declarations, 20 typed value positions, 0 affine, 0 annotations |
| `&Int`, `&mut Int`, `<'a>`, `'a Int`, `move x`, `Box<Int>` | none is Pleris: each fails to parse or names no type, beside a plain `fn f(x: Int)` that parses |

The list of affine types is asserted exactly, so a new one is a visible change.
The store's commands carry strings, integers and a list of records across the
component boundary. Their memory is the invocation region
([ADR-0032](../../DECISIONS/ADR-0032-compiled-components.md)), and nothing in
the source manages it.

## Task 10: deterministic serialization and versioning of resumable state

- **Versioning** is E7V's, complete: content identity, versioned hash schemes,
  strict matching with explicit migrations, and per-construct recovery
  ([deployment-matrix.txt](../E7V/deployment-matrix.txt)).
- **Serialization** of the state a document carries for a handler is new with
  compiled handlers (ADR-0033): `data-pw-captures`.
  `carried_captures_serialize_deterministically` holds two things. Two renders
  are byte-identical. Two templates that list the same capture paths in
  different orders also produce identical bytes, with the keys sorted. The
  paths come sorted from `resume::capture_paths`. The object's keys are sorted
  because nothing in the dependency graph enables serde_json's
  `preserve_order` (checked with `cargo tree -e features`). If something ever
  did, the permuted case would fail.

## Not claimed

- **The ownership check reads effect rows and written types.** It is not a
  proof that no future construct could need an annotation. It is the state of
  the programs that exist.
- **A capture's type change across deployments** is E7V's decision (the
  capture schema hash), not this test's.
