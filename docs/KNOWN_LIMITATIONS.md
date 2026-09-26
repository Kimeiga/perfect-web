# Known limitations

Reviewed 2026-09-15 against master `4a9bcddc095b3b4d0eac0e0c2ddb8a0143de03d9`
and the evidence-gate repair. Current milestone state is owned by
[STATUS](STATUS.md); implementation order is in [NEXT](NEXT.md).

The former file was primarily an E0 snapshot and still said no compiler or Linux
CI existed. Its complete bytes are retained in
[the historical limitations file](KNOWN_LIMITATIONS-history-2026-09-15.md).
Those historical statements must not override current source or test results.

## Value checking and generated execution

**E9-V1..V6 are met** (2026-09-24, ADR-0031). What the value relations do not
decide is Undecided, counted by `pw audit-values`, and never reported as
agreement:

- **A member of a value of unknown type is not judged** (ADR-0048). Member
  existence is a relation now (PW0610), and a read whose value's type nothing
  states is undecided, as every relation's is.
- **Sum-type variant constructors** (`Circle(3)`) have no type, because the
  workspace does not resolve variant names as terms. Record and opaque
  constructions are checked.
- **Named-argument calls** are Undecided: a signature does not carry parameter
  names. None occurs in the corpus.
- **A name bound at two sites in one body is unknown** to the value
  relations, whatever each binding holds (ADR-0053). Their environment is
  flat and cannot say which binding a use means. A lambda's parameters take
  the types its use declares, except under such a name.
- **A `()` body discards its last value** (A-018). A branch mismatch in
  statement position is not refused; as a result, it is.
- **Generic layouts at the boundary.** Phantom parameters map to one WIT
  layout; a generic whose layout depends on its arguments is refused, because
  specialization is not implemented.
- **Named function values are not instantiated.** A generic function named as
  a value (`List.map` passed along) has its type parameters as holes.
- **The unit value `()` lowers to `Expr::Error`.** Its type is therefore
  unknown. This is harmless to the relations, which never guess, but it is a
  lowering gap.
- **`g(a)` followed by `()` on the next line parses as `g(a)()`.** The language
  has no statement terminator; nothing in the corpus depends on the difference.

A scope/resource policy is now attached to the resolved type rather than its
spelling, preserving the existing type-level policy. This is not a per-value
ownership proof. Private RPCs now require conditional principal preservation;
that records a runtime obligation, not its execution. Stable nominal capture
identities distinguish modules; complete transitive schema-evolution and host
semantic-ABI validation remain separate work. Foreign code and backend internal
record/variant layouts still need their documented integration proofs.

**E10-I is established** (2026-09-24, ADR-0032): the store's commands compile
to components and run through the E8 host, and the Rust closure path is deleted.
The component backend is narrow, and everything outside it is refused by name
rather than approximated:

- **A generic record is not instantiated** (ADR-0050). `Box<Int>` has no
  layout: `Type::Nominal` names a declaration without its arguments. A
  generic *function* is instantiated.
- **Straight-line bodies, at E10-I.** Import calls and scalar constants were
  supported, and values moved flat or in their canonical layout. Matches over
  `Option` and `Result`, field reads and their cases came after (ADR-0036),
  and then computation (ADR-0039, below).
- **The invocation region is the memory strategy** (ADR-0046, measured). It
  is a bump region reclaimed by each export's post-return, and nothing can
  outlive an invocation. A call's peak is its whole allocation: kiokun's
  Search reaches 3.2 MB for the one-letter query `T`.
- **Each call instantiates afresh** (ADR-0032). At 7–14 µs it costs more than
  most kiokun calls themselves (ADR-0046).
- **An opaque value is not built or read inside a component** (ADR-0048).
  `PositiveInt(1)` and an opaque type's `.value` are refused by the Wasm
  encoder; opaque values cross the boundary as their representations.
- **The data layer is not Pleris.** `store:data/carts` is the deployment's
  (`owner: external`), as the contract records.

**The backend matches over `Option` and `Result`**, reads fields, and builds
their cases (2026-09-25, ADR-0036). **It computes** (2026-09-25, ADR-0039):
`Int` and `Float` arithmetic, comparisons, `&`, `|`, `!`, `if`, string
literals, interpolation, pipelines, records built, and calls to other
declarations, inlined, or compiled beside the export when they recurse, at
the types a generic callee's arguments give it (ADR-0050). An early
`return`, `?`, `for` loops and `let mut` bindings compile (ADR-0051). Still
refused by name:

- nested patterns, and a declared variant built or matched;
- a `return`, a `?` or an assignment inside a lambda a list operation runs,
  and an assignment to a field (ADR-0051);
- `%` on a `Float`, and a `Float` interpolated: their semantics are not
  decided;
- list and string operations beyond the ones the standard library declares
  (ADR-0040, below).

- **Traps are not distinguished by cause.** An `Int` overflow and a zero
  divisor both stop the invocation, and the host reports a failed call; which
  one it was is not carried.
- **An operand the checker cannot type is undecided** (ADR-0043). PW0609
  refuses operands of two known types that an operator does not take, and
  counts the rest as undecided; the backend refuses what remains.
- **There is no implicit conversion between `Int` and `Float`.**
  `Float.from_int` converts where a count meets a measurement (ADR-0043).
- **The logical operators are `&` and `|`.** `&&` does not parse. They
  short-circuit.

- **Some matches are not analysed for exhaustiveness.** Each is counted
  Blocked by the match audit, with its reason: neither proven nor refused.
  - A constructor pattern nested under `Some`, `Ok` or `Err`
    (`Some(Some(x))`). The backend refuses nested patterns.
  - A literal pattern (`Some("a")`).
  - A scrutinee the value relations cannot type, including a name bound at
    two sites.

  Until 2026-09-25 matches over `Option`, `Result` and calls were not checked
  at all, and four other shapes were proven exhaustive when they were not
  (ADR-0038).
- **A binding cannot share a constructor's name** (ADR-0038, ruling needed).
  A bare pattern name that some type has as a constructor is read as that
  constructor, and against another type it is PW0608.
- **`return` is a statement, not an expression.** Its value is the statement
  after it, in a block or on its line in a match arm (ADR-0038).
- **Template matches take apart `Option` and `Result` only** (ADR-0042). A
  declared sum type's constructors in `{#match}` are refused (PW5019), as the
  component backend refuses them. `{:else}` in `{#each}` is refused;
  `{#if xs}` around the list says the same.
- **An interpolated attribute is refused in a `style`**, and a URL with holes
  must begin with text (ADR-0042). A hole must be a value path.
- **Nothing emits patches for the new template parts.** A `{#match}` region
  or an interpolated attribute renders on the server; the dev server's patch
  generator is written per operation, and the store uses neither. kiokun's
  pages are static.
- **A clause's words are the clause's** (ADR-0047). Every name resolves in
  lexical scope, but `scope application`'s `application`, or `load`'s
  `on_first_interaction`, is a word of its clause and not a name. A word
  outside its clause's closed set is left to the analysis that reads the
  clause, and most clauses written inside a block have none.
- **`derived e` is parsed, not checked** (ADR-0047). It is one expression,
  and the charter calls it pure; no rule refuses an effect inside it.
- **The standard library is small** (ADR-0040). `List` has `length`, `get`,
  `take`, `concat`, `map`, `filter`, `fold`, `any`, `all`, `find`, `sort_by`
  and `group_by` (by a `String` key, adjacent runs); `String` has `length`, `codepoints`, `from_codepoints`,
  `starts_with`, `ends_with`, `contains`, `join`, `trim` and
  `to_lower_ascii`; `Float` has `from_int` (ADR-0043). There is no slicing, no Unicode case mapping, and no map
  or set type. `sum`, `maximum` and `enumerate` still have placeholder
  bodies.
- **An affine value has no borrow** (ADR-0045). A function whose row does not
  release a transaction may use it, and one whose row does must end it once
  on every path. There is no way to say "this function reads the value and
  hands it back"; a `use` block releases its value and is the scoped form. A
  release inside a loop is refused even when the loop would run once.
- **A function value read from a record field is not called** (ADR-0052).
  A function is a value: a lambda or a declaration's name is stored,
  returned, passed and called. But `r.check(5)` reads as a method call. A
  lambda whose use fixes no parameter types is refused by name.
- **A statement keyword cannot name a value** (PW0013, ADR-0041, ruling
  needed). `let query = ..` is refused rather than read as a `query ..`
  statement at every use.

**The kiokun slice** (ADR-0037, ADR-0041) is one shard of kiokun.com, with
its logic in Pleris and its data layer in the host:
- **Search covers the loaded shard only.** A lookup reaches every shard.
  kiokun.com searches in SQLite FTS5, and an index over every shard is the
  deployment's.
- **The index is Rust.** Candidate retrieval, case folding and a stub's target
  are the host's, as the database's are kiokun.com's. The ranking is Pleris.
- **The ranking is held to the slice's port, not to kiokun.com's live search**
  (ADR-0041, corrected). kiokun.com's `/api/search` decides which queries are
  CJK differently (hangul yes, astral Han no) and searches script variants.
- **Korean is looked up, not searched.** An entry shows its Korean words,
  Japanese names and character (ADR-0037, amended). The index has Chinese and
  Japanese rows; kiokun.com's Korean rows need its romanization and ranking
  ported first. Pitch accent is not in kiokun's entries.
- **A call is a fresh instance** (ADR-0032). Bulk work needs a query over a
  list, as `shards.Places` is: a call a word made the whole shard's load three
  times slower.

**Resumable handler bodies are compiled** (2026-09-25, ADR-0033), to one ES
module each, and the page's elements carry what the handlers read. The set is
narrow:

- **One command call per handler.** Its arguments may be captured values and
  their fields, literals, and opaque constructors over a primitive. Local
  computation, branches, multi-statement bodies and handler parameters (the
  event) are refused, and so is a command parameter that is not a primitive or
  an opaque type over one.
- **Captures are not a patched part.** An element carries the capture paths
  its handler reads, as rendered. A patch that changes a captured field without
  re-rendering the element leaves the old value there: for example, E7-P's
  keyed rename, which replaces only the item's text. The store's handler reads
  only `item.id`, the loop's key, which a keyed patch never changes.
- **Opaque invariants are not checked at the boundary.** The host types a
  browser's arguments by the component's parameters: `PositiveInt` arrives as
  an `s64`, and any `s64` is accepted. `opaque type PositiveInt = Int` states
  no invariant that could be checked.
- **A hole cannot hold a string** (ADR-0049). `"{f("a")}"` ends the outer
  token at the inner quote. Escapes are defined, and every backend reads one
  decoder; policy strings (`because`, `route`, `host`) are read as written.

## Research requirements are not implemented guarantees

The census records temporal authority, unknown external outcomes, compatibility
across live versions, composed capacity budgets, optimistic overlap, and unmanaged
browser/foreign behavior. Some related mechanisms already exist, but the census
itself does not prove complete enforcement. Read the per-record status and the
[research/implementation boundary](../research/failures/IMPLEMENTATION.md).

## Evidence and test coverage

The new recipe tests replace producers in isolated temporary trees. They prove
exit-status propagation for the tested recipe shapes, not compiler correctness,
browser behavior, host isolation, performance, or production readiness.

`errexit` and `pipefail` do not make shell execution universally fail-safe.
Independently invoked scripts and shell contexts that intentionally handle errors
need their own tests. Redirected reports may be incomplete after failure; this
repair does not make report publication atomic or validate every printed claim.

No new real-device, screen-reader, cross-browser, database-fault, or production
rollout test was run locally in the September 15 review. Prior observations keep
their original scope. The false-green recipe defect does not prove old tests
failed, but a recipe's zero exit status alone was insufficient evidence.

## Historical measurement interpretation

ADR-0027 supersedes the inference that an undefined frame-level
`forcedStyleAndLayoutDuration` establishes browser non-support. That value is
read from script-attribution records by the repaired instrumentation. Missing
observations and observed zero must remain distinct. Old raw measurements are
preserved, not replaced with invented results.
