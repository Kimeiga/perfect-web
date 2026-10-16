# ADR-XXXX: a table declares an app's data and gives its reads and writes

Status: implementation of ADR-0338 D1; unnumbered for the integrator.
Date: 2026-10-16. Milestone: E14. Track: `tables`.

ADR-0338's accepted ruling is the authority: apps declare tables in Pleris;
generic adapters serve them. This slice proves the compiler half. It does
not implement an adapter or claim that a table can yet be served.

## Decisions

- A table is declared in the module of its name, with one table per module.
  Its `row` is a record declared beside it. The generated functions are
  ordinary declarations of that module; their effects name that table and
  their host imports use `pw:tables/<table>#<function>`. This reuses ordinary
  resolution and the existing capability boundary. Several tables in a module
  would need new subnamespaces; postpone that extension until a use needs it.
  Generated tuple names are checked for collisions (`a_b,c` and `a,b_c`
  would otherwise silently share one record with two different shapes).
  Distinct tables and functions must also emit distinct ABI identities:
  `Todo_Items`/`TodoItems` and `by_a_b`/`by_a__b` collide after WIT naming.
  PW0361 refuses those collisions using the emitter's canonical
  `interface_of`/`import_of`, both using the existing `wit::ident`; it does
  not introduce another ABI spelling algorithm. The two new tests failed
  before the guard (`tables-abi-before.txt` beside the other D1 evidence).
- `row`, `key`, `order`, `index`, `unique`, `refers`, `bounded` and `migrate`
  have declared policy domains. An index is named `by_<name>`. A key, order
  or index uses scalar fields: integers, strings, booleans, opaque types over
  these, or nullary cases. `refers` holds the target's one-field key, directly
  or as an `Option`; composite references are a later extension.
- Each keyed read has a `_many` form. A proper key prefix has an
  `of_<fields>` read and `delete_of_<fields>` write. Tuple arguments to set
  reads are generated record types. A unique index answers `Option<Row>`;
  `insert` and a unique table's `put` answer `Result<(), Row>`, whose error
  carries the conflicting row. A table without uniqueness has `put -> ()`.
- Reads are deterministic: whole-key reads by key; prefix and index reads
  by `order` then key; set reads grouped by their requested values in ascending
  order, then the same row order. Strings compare by code point. The emitted
  `sorted_by` records this contract for D2's adapter suite.
- PW0361 checks malformed tables, PW0362 refuses hand-written bindings
  reading or writing a declared table (and hand-written `pw:tables` bindings),
  PW0364 refuses `all()` without `bounded`, and PW0366 checks migrations.
  PW0354..PW0360 and PW0363 remain reserved for ADR-0338's other tracks.
- **Correction of interrupted WIP:** PW0365 is a warning for every valid
  cross-source reference, whether its field is required or optional. An
  `Option<Key>` does not make a foreign key across databases enforceable.
  This follows ADR-0338 decision 4; the earlier WIP's error for required keys
  was stricter than that decision. Both forms check and build, and are tested.
- `pw build` writes `tables.json` and `schema.lock`; `--schema-lock FILE`
  on check/build holds the declaration to the released schema. Additions are
  planned; destructive field changes need a `migrate` (`renamed from`,
  `dropped`, `backfilled with` a typed literal). A key change, dropped table
  or source move is refused: this slice has no language for their data moves.
  The lock hashes its canonical schema and refuses edits without a matching
  hash. The host applying this plan and holding drift is D2/D3, not D1.
- Generic fields are projected with a storage-argument environment keyed by
  resolved binder identity. This recursively instantiates nested constructors
  without introducing a second semantic type substitution. Arguments are
  projected before opening a definition: finite `Box<Box<Int>>` is distinct
  from a recursive definition. Actual recursion retains its declared symbolic
  recurrence. A stored type change `Box<Int>` to `Box<String>` is destructive.
- **Interim source guard, ruled by the integrator 2026-10-16:** a table or
  reference whose source name names more than one declaration is refused as
  PW0361, with both qualified declarations and a distinct-name repair. The
  existing host seam and source analyses use bare source names, so comparing
  two different `Data` declarations would silently consider two databases one.
  This guard is not T1's source architecture. The broader pre-existing risk
  remains for non-table resources: PW0345 can accept writes spanning two
  different declarations of `source Data`; duplicate-resource checks also
  distinguish sources only by bare name. T1 must carry qualified source
  identities through those analyses and the host seam.
- Corpus C27 uses A-050..053 and R-100..107. Accepted warning fixtures name
  their exact warning codes with `@expect-warning`; errors and undeclared or
  missing warnings fail. Migration fixtures name a companion released lock.

## Evidence and remaining boundary

`just e14-tables` records clause parsing, generated signatures/effects and
imports, fixture build metadata, declaration refusals with accepted controls,
schema diffs, and the whole mutation suite in `docs/evidence/E14/tables.txt`.
Final measured counts are recorded after execution below.

The two re-anchored older scripts were run whole at `697bbe5c`:
`python3 scripts/declared_once_mutations.py` (5-test baseline, 10/10 killed)
and `python3 scripts/computed_holes_mutations.py` (62-test baseline, 23/23
killed). Neither stopped a process for memory. Their outputs are preserved
as `tables-declared-once.txt` and `tables-computed-holes.txt` in D1's evidence.
Two final direct D1 controls also cover the table namespace's platform
ownership and the valid WIT representation of `Result<(), E>`; no production
code changed to add them.

Adversarial tests were red before the guards/projection repair: all three
misses (duplicate source name, tuple-name collision and nested generic storage)
reported in `docs/evidence/E14/tables-adversarial-before.txt`, produced by
`cargo test --locked -p pw-core --test tables` on the then-working tree.

No app-specific Rust, dependency download, database or browser is needed.
The bounded-checker track's generated-pair integration must use the generated
metadata directly, not require a handwritten `set`: flat prefix/index set
results and composite keys do not fit handwritten pair-shape checking.
D2 first supplies memory primitives and converts TodoMVC; D1 never starts it
before its compiler gate passes.
