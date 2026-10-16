# ADR-0350: a table declares an app's data and gives its reads and writes

Status: accepted under the owner's delegation; integrated train validation pending.
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
- **Shared resolution correction, ruled by the integrator 2026-10-16:**
  `Workspace::visible_module` returns the checking unit's own module first,
  then explicitly imported modules through the existing canonical graph.
  `sees_module`, qualified path resolution and table lookup consume that one
  API. D1's previous global first-by-name table scan let a namesake sibling
  supply a different reference key; the new direct test checks both source
  orders and fails before the fix. Direct resolver tests also pin own-module identity
  and an unimported sibling's invisibility.
  Duplicate modules remain invalid (PW0024, from the registry). While judging
  a unit, the interim source guard ignores an alternate copy of that unit's
  own module: the copied fixture is not an additional database in the first
  fixture's diagnostics. The guard uses the same Workspace API. Same-file
  duplicate sources and same-named sources in distinct modules are still
  refused, including modules with no import between them. This is diagnostic
  isolation for an already-invalid duplicate-module program, not a repair of
  the broader source identity limitation above.
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

Full workspace CI exposed five failed targets; its complete raw output is
`tables-ci-before.txt`. Four were expectation updates: a generated `host`
policy points to its originating table clause rather than a written `host`
keyword; the `bounded` grammar fixture needs a number; the WIT assertion
must expect the valid `result<_, string>` form; and C27 adds six written
resource declarations to the manifest coverage sample (19, previously 13).
Written policy spans remain strictly checked, generated spans are checked
against their table provenance, and the corpus's exact diagnostic comparison
is unchanged. That comparison exposed the genuine source-guard isolation
defect; a direct adversarial test also exposed the global table lookup defect.
Their red-before outputs are `tables-source-isolation-before.txt` and
`tables-isolation-before.txt`. These are distinct from the fixture expectation
updates. Four lint corrections only elide a lifetime or simplify test syntax.
After repair, all 12 affected core suites pass (157 tests), as does the resource
manifest suite (7 tests). The own suite now has 44 controls, including direct
controls for both module lookups and source-guard diagnostic isolation; its
final whole run and full CI are recorded after they complete.

Final D1 validation on source commit `4d84377e`:

- `just e14-tables`: core 34/34 and CLI 2/2 tests; baseline 36/36;
  **44/44 mutants killed by test failures**, no process stopped for memory.
  All three new isolation controls are killed independently. The complete
  record is `docs/evidence/E14/tables.txt`.
- `just ci > /tmp/tables-ci-final3.log 2>&1`: exit 0, **`ci: OK`**.
  Its 338 Rust result groups total **2,932 passed, 0 failed, 25 ignored**;
  the ignored external-data/measurement cases are unchanged. Evidence gates
  pass 161 tests (one existing skip); workspace and engine lint, formatting,
  case checking and all 2,938 mutation anchors pass. The source-program gate
  reports 237 variants, 11 named-rule refusals and 0 failing cases.
- The unchanged strict corpus diagnostic comparison passes 7/7 (25.88s),
  direct identity tests 5/5, and both latency tests pass (22.95s). The
  coordinator was told latency had completed before its separate contention
  phase. Corpus remains 40 accepted / 82 rejected; generality is 58/58.
- `docs/evidence/E14/tables-ci.txt` is the **complete, byte-identical raw CI
  output**, SHA-256
  `c1cc195e0676a2c2bf0e4c978a765dab387d423cd10c8ffeb29d8d403cf323a4`.
  Earlier whole 37- and 41-control runs remain explicitly historical records.

The integrator owns generated-pair registration at the train merge, from
`Hir::generated`'s table/primitive/by metadata, preserving handwritten pair
checking. All generated pairs need descriptive help that selects each turn's
matching rows from the flat many-result; substituting the entire batch would
be wrong even for a one-field prefix/index. No integration of that other track
is claimed in this isolated D1 tree.

The integrator clarified set semantics before D2: requested values are
deduplicated, each matching stored row occurs once, missing values contribute
no row, and empty input answers `[]`. Results use D1's canonical by-fields,
declared order and full-key tie-breaker, independent of request positions;
comparisons use declared scalar semantics. D2 records and tests that ruling
at two sizes, including permutations, duplicates, misses, empty input and ties.
This clarification requires no change to D1's emitted ordering metadata.

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

## Train integration decision, 2026-10-16

The bounded-read checker registers generated read pairs from the existing
`Hir::generated` table, primitive and `by` metadata. It pairs Get/GetMany,
Prefix/PrefixMany and Index/IndexMany only for the same table and fields.
Generated flat row lists and composite arguments have the compiler's declared
shape; they do not pass through the handwritten `set` shape rule. That rule
continues to require named keys and the exact Result/Option answer.

All generated repairs describe the actual qualified many function, scalar
values or a composite key record with its field names, a read before the
loop and selection of each turn's matching rows. They do not substitute the
whole flat batch for each keyed result. Source `sets` costs apply to generated
many reads, including prefix and index operations. New integration tests and
independent mutation controls must demonstrate both the repair and PW0406;
no assembled passing result is claimed before those commands run.


### The integration's mutation-result instrument, 2026-10-16

The first generated-read control run killed only 42/47: the new Cargo
command ran two binaries but the harness read only the first result.
Separate commands exposed all five failures, and the complete 47 passed.
An isolated adversarial review then found six failing cases among eight
regressions against the committed harness: missing test results, stopped
processes, inconsistent exits and a later failed build could be miscounted.
An invalid build could also be counted as a killed control.

Before changing the instrument, the integrator rules: count every Rust
binary summary, require consistent result states and command exit status,
and refuse missing summaries, stops and build failures. Refused controls
must fail the campaign and earn no kill. Keep the full negative record;
then run all eight regressions, the whole 47 controls and full integrated
CI. This strengthens evidence handling and changes no compiler acceptance.
The separate child-process sample-selection repair is shared from train-9;
production still respects deployment paths. Runtime adapter cost admission
remains D2 work, including for this fixture's declared one_call source.

## Completed local integration validation, 2026-10-16

The pending merge into train-10 passed full local CI: 346 Rust result
groups, 3,058 passed, 0 failed and 25 ignored; Python 233 passed with
2 existing skips. All 302 source mutation variants were judged through
463 checks, with 16 named-rule refusals and no failing case. C27 remains
42 accepted / 87 rejected, generality 60/60. The complete raw CI output,
input manifest and command ledger are linked from
`docs/evidence/E14/train10-d1-integration.txt`.

The final whole bounded47 and PostgreSQL12 campaigns killed all controls
through built test failures, with no process stopped. The guide baseline
passed 12 browser tests in three engines after both dictionary deployment
variables were unset. Its 74-control campaign reports 66 document witnesses,
5 browser assertions and 3 exact named checker refusals; a separate private
injection found invalid command exits can still be misclassified by that
guide instrument. Its hardening remains open, rather than being silently
certified by this passing campaign. Final restoration rebuilds are gated;
the guide mutator does not repeat a final restored browser suite.

Two earlier full-CI failures came from fixture anchors, not production
guarantees: Addresses spacing after canonical formatting, and a StoreData
patch that assumed `changes none` was EOF before `sets one_call` was added.
Only those anchors changed; both exact refusal assertions remain. Their
negative raw hash and the bounded-result parser's six red/eight green
regressions are retained in the ledger. All 2,366 final source/config
hashes matched after mutation restoration and again after full CI.

This closes the local D1 compiler integration gate. D2 runtime adapters,
provider cost admission, generic-result integration and the final assembled
train's CI/GitHub/quiet-host proofs remain separate obligations.
