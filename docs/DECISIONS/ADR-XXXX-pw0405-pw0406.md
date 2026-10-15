# ADR-XXXX: a read is bounded by construction, the checker's half (PW0405, PW0406)

Status: proposed by the worker of track `bounded-checker`, for the
integrator's merge. It builds step 2 of the design note "a read is bounded
by construction" (W9's, `docs/DECISIONS/ADR-XXXX-a-read-is-bounded-by-construction.md`
on `origin/track/bounded-reads`), which the integrator accepted on
2026-10-14, ruling the order of work: (1) the host's render merge (its own
track), (2) this, the checker's rules, (3) the per-layer conformance suite,
(4) the store's composites above the seam. Date: 2026-10-14. Milestone: E14.
Builds on ADR-0207 (a data source states what it guarantees), ADR-0101 (a
declaration's inferred row), ADR-0022 (record, never recompute), ADR-0063
(which binding a name means), ADR-0078 (an effect is performed where its
function is named) and ADR-0218 (a host function is a name and a signature).

## Context

- The owner asked that N+1 be impossible by construction, whatever the
  database. Until now it was only measured (4b, `tests/bounded_statements.rs`).
- The design note decides the rules, their codes (Effects', PW0405 and
  PW0406, the integrator's assignment) and their words; this ADR records
  what was built of it, how, and what was found running it over every
  reference program.

## Decision: what is built

### PW0405, a read keyed by its loop's element (an error)

A call that performs a source read is refused where it is inside a loop
over data and its key comes from the loop's element.

- **A source read**: a call whose inferred row (ADR-0101) has
  `database.read<R>` and that reaches a function a `host` clause binds. A
  host read keys a read by each of its parameters; a function with a body
  keys one by each parameter a key of a read it calls comes from (a fixed
  point over the call graph), so a helper called in a loop, `charge(l)`,
  is a read per element where its own read is keyed by `l`, and not where
  it is keyed by what is no element's. A row alone reads nothing:
  `Stores.find`'s body is `todo` and reaches no host read.
- **A loop over data**:
  - a callback a combinator applies to a list's elements. A function with
    no body (the standard library's `List.map`, `fold`, `filter`, `find`,
    `any`, `all`, `sort_by`, `group_by`) is one by its signature: a
    parameter `fn(.., T, ..)` beside one `List<T>`, which by its type can
    give its callback no `T` but the list's. A function of the program's is
    one where the call graph shows it: it passes the parameter on to be
    applied so, or calls it in a loop of its own. The callback may be
    written as the argument (the walk's `effects::callbacks`, which
    `enclosing_callback` reads), bound by a `let` and passed, or a function
    named as the callback (`List.map(lines, charge)`), refused where it is
    named. A pipe's value and named arguments are arranged as a call's
    (ADR-0081).
  - a `for` body;
  - a recursion over data: a function on a cycle of the call graph whose
    recursive call is passed a part of one of its parameters (through a
    field, a pure call, a `let`, or a loop's element: `count(r)` for `r` in
    `c.replies`). A parameter passed whole to its own position of the same
    function is passed along, not walked, and a read keyed by it is not
    refused. The recursive call is the recursion's turn, not a read.
- **A key from the element**: a name the loop binds, through a `let`'s
  initializer, a match arm's scrutinee, a field, a record or list built of
  it, an operator, or a pure call on it (one the walk charged no effect, or
  a call through a value). Not through another read: a key a read computes
  ends the trace, and the inner read is the loop's read per element,
  refused once.
- Nested loops: the innermost loop whose binding the key comes from.
- **The words are the note's**: "`hours_of` reads `Stores` once for each
  element of `stores`"; a note at the key, "its key `s.id` comes from `s`,
  the element `List.map` passes" (`for` binds, or "a part of which `total`
  passes to its recursive call"), one at the list, "one read per store" (the
  keyed read's parameter's name), and one at the recursive call; the help,
  where the keyed read names its set read, "read them together, before the
  loop:", the `let` of the set read over the loop's keys and, where the
  loop is one line, the loop with the read's place taken; where it names
  none, "`hours_of` has no set read; its source can declare one".

### `set`, a keyed read's set form (held by PW0335)

`set hours_of_stores` on `fn hours_of(store: StoreId)` (a clause on a
function, `Domain::SetRead`). Held to the note's shape:
- it names a function visible from the keyed read's module, not itself;
- the keyed read reads a source, and the set read reads the same one (the
  sources holding what each reads; a resource no source holds is the host's
  database's);
- its first parameter is a list of the keyed read's first parameter, the
  key, and the rest are the keyed read's, alike (a reader passed along);
- it answers a list of what the keyed read answers when it finds it (a
  `Result`'s or an `Option`'s first argument) with its key: a record with a
  field of the key's type and one of the answer's, or the answer itself
  where it holds a field of the key's type.
A `set` that does not fit pairs nothing, and is refused with PW0335
(`policy_value`, reason `set_pair_shape`): see the questions.

### `sets`, what one set read costs a source

`sets one_call | chunked(k) | per_key` on a `source`; a source that states
none is `per_key` (a clause left out guarantees nothing, ADR-0207).
`chunked` is an operator of one positional count (`Arg::Positional`,
`chunked(100)`); a value outside the domain is PW0335. `pw build` writes it
to `sources.json` (`SourceClauses.sets`, defaulted for older builds; the
host's `Declared` ignores it until the conformance suite reads it).
`Sets::calls(n)` is the note's budget per set read: 1, ⌈n/k⌉, n.

### PW0406, a set read on a source that cannot batch (a warning)

A call of a set read (one a valid `set` names) whose resource is held by a
source stating `sets per_key`, or none, warns in the declaration that makes
it: "`StoresOpen` reads `Stores` once per store on `StoreData`, which cannot
batch", noting "`StoreData` states no `sets`, so `per_key`: a set read is a
call per key", help "hold `Stores` in a source that batches, or bound the
list". A resource no source holds is the host's own database's, which gives
every guarantee (ADR-0207): one call, no warning (see the questions).

### What the effect walk records, and the grammar

- `effects.rs` keeps each call it resolves, a pure one too
  (`Inferred::calls`: the call, the declaration, what it charged): the call
  graph the rows are inferred over, recorded where the walk resolves each
  callee (ADR-0022), not resolved again. `callbacks` (each lambda written
  as an argument, its call and position) is shared by the walk and these
  rules; `enclosing_callbacks` gives every callback around a call;
  `Inference::performed_of` and `Inference::lexical` give what `run` found.
- `set` and `sets` are clause heads (`POLICY_KEYWORDS`), and like `layout`
  head a clause only where a name follows on their line (`NAMED_HEADS`):
  `set.size()` in a block whose clauses come first is a statement.
- The codes, in the Effects block beside PW0400 to PW0402: PW0405
  `read_per_element` and PW0406 `set_read_per_key`, `Owner::Effects` (the
  block has no track owner; the integrator assigned both).

## Found (corrections first)

1. **The note's migration, "no program changes", holds for the store and
   the feed, and not for kiokun.com.** `pw check` over every reference
   program: the store, the accepted corpus, the feed and the kiokun slice
   read nothing per element. kiokun-site (`examples/kiokun-site/app.pw`)
   reads four times per element, each as kiokun.com does:
   `kiokun.site.Word` asks `search_reading` once for each reading twice
   (app.pw:3133, :3175) and `read_pitch` once for each word (:3168), and
   `kiokun.site.Search` asks `search_reading` once for each reading
   (:3945). Interim, conservative and cheap to reverse (see the questions):
   PW0405 is an error everywhere, and these four, listed with their reasons
   in `bounded::KNOWN_READS_PER_ELEMENT`, are reported as warnings that say
   why they wait. A test holds the list to exactly these (a stale entry
   fails, and the same read in a declaration the list does not name is an
   error). The list empties when kiokun's layer
   (`spikes/own-renderer/server/src/kiokun.rs`) has the set forms and the
   program reads before its loops.
2. **Three generality witnesses of `undeclared_effect` read per element**
   (`List.map(ids, id => Stores.get(id))`), incidentally: their subject is
   the row. They read `Stores.get(home)` now, a parameter, keeping their
   shapes and verdicts (commit 0dd29f80).
3. **The note's example is exactly what the store warns** once `set
   hours_of_stores` is written and `StoreData` states no `sets`: PW0406 on
   `StoresOpen` (hours.pw) and on `Search` (search.pw). The two annotations
   below must land together. The note's position, hours.pw:391:17, is
   392:17 with the `set` line.
4. **`check_latency`'s 2 s ceiling failed at 2.77 s** on this PC while
   other workers' builds held the load average at 76 to 88. The rules cost
   the store's check (52 files, debug) 9 to 24 ms for the program's facts
   and 6 to 11 ms for every unit's checks, about 2% of it.

## For the integrator at merge

- **The store's annotations**, which this track does not write
  (`examples/store` is other workers' now), each as `pw fmt` writes it:
  - `examples/lib/StoreData.pw`, after `    changes      none`:
    `    sets         one_call` (4b measured it: each set read's statements
    the same at both data sizes, for every store at once, `hours#for-stores`
    3 and `fees#for-stores` 1).
  - `examples/store/hours.pw`, after `    host "store:data/hours#for-store"`:
    `    set  hours_of_stores`.
  - No other pair fits yet: `fees_of_stores` has no keyed read beside it,
    and `menus#for-store`, `coverage#for-store` and the composites wait on
    step 4.
  `tests/bounded_reads.rs` applies both in memory and holds the store clean
  with them, and two PW0406 with the pair alone. When they land, the test
  `the_stores_pair_and_its_sources_cost_are_the_integrators_to_add_together`
  should read the store as it is.
- **The generality figure**: 49 / 49 becomes 50 / 50 (`read_per_element`);
  `tests/generality.rs` says so, and `docs/STATUS.md` and `docs/NEXT.md`
  carry the number.
- **Corpus C24**: R-070, R-071, R-072 and A-037 (the numbers were free at
  5da9c055).
- **KNOWN_LIMITATIONS**: kiokun-site's four reads per element (Found 1);
  and the Not claimed below.
- The root `justfile` imports `just/bounded-checker.just` (one line, as each
  track's import was added).

## Not claimed

- **The bound's emission beside the plan** (the note's evidence: each
  page's and command's calls by source, each one's cost, checked against
  `Counted` in the server's tests). Next: `Sets::calls` is the cost per set
  read, and `Inferred::calls` the calls; a page's plan (`page_values`) and a
  command's contract are where it would be written.
- **The rewrite** of a loop's reads into its set read: a later step, as the
  note orders.
- **Writes per element**: a call that only writes is not refused (`for l in
  lines { record(l.item) }`); one whose row reads and writes is refused as a
  read.
- **A key of two values**: the pair takes the keyed read's first parameter
  as its key and the rest as passed along; `read_pitch(shard, word)`, whose
  key is both, has no set form this shape accepts.
- **Keys the trace does not follow**: through another read's answer (the
  note's "keyed by computed values"), through a `use`, or through a
  function value's call.
- **A recursion through a function named as a value** (`List.map(children,
  render)` inside `render`) is a loop of its combinator, where a read by its
  parameter is refused as the named callback's; it is no cycle of the call
  graph, whose edges are calls.
- **The host's loops**: a page's `{#each}` and its instances, and its
  sibling bindings, are the render merge's (the note's), not checked here.
- **A member call that reads** (`x.read()`) is seen only where the walk
  resolves it by its path, its receiver arranged as its first argument;
  `member_effects`, which resolves a member by its receiver's type, records
  no call. No reference program reads through a member.
- **A source's cost under throttling** (DynamoDB's `UnprocessedKeys`), and
  whether a deployment's database gives the `sets` its source states: the
  conformance suite's (step 3).

## Alternatives

- **Run the rules on a second walk of each body** for calls and their
  callees: two resolutions of one call is the recomputation ADR-0022
  forbids; the walk records what it resolved instead.
- **A table of combinator names** (`List.map`, `List.fold`, ..): a table
  describes functions that may not exist (E2C's deletion gate); the
  signature says which parameter is applied to a list's elements.
- **Ship PW0405 as a warning until kiokun-site reads its sets**: no program
  would be refused, a new N+1 in the store or the feed included. The known
  list excuses four named reads and refuses every other.
- **Migrate kiokun-site here**: its layer is another track's, `read_pitch`'s
  key is two values (a design question), and its pages are held byte for
  byte to kiokun.com's.

## Acceptance

Recorded by `just e14-bounded-reads` in `docs/evidence/E14/bounded-reads.txt`:

- `compiler/pw-core/tests/bounded_reads.rs`, 27 tests: PW0405 in each loop
  form (seven combinators and a pipe, a `for` through a `let`, a recursion
  over a list and over a count, a function of the program's passing its
  callback on and calling it in a `for`, a `let`-bound callback, a named
  callback, a helper, a nested loop), each key path, and its controls (a key
  no element's, the set read, a callback applied once, a parameter passed
  along, a helper keyed by no element, a write, a read before the loop);
  ADR-0022's perturbations (renamed, rebound, a decoy); PW0406 on `per_key`
  and on no `sets`, none on `one_call`, `chunked(100)`, the host's database
  or a read no `set` names; the pair's shapes; the clauses' places and
  values; `Sets` and its budget; `source_clauses`, and an older
  `sources.json` read as `per_key`; the store (clean, mutated per element
  and refused, with the two annotations); the feed; kiokun-site's known
  four. And `bounded`'s own: the call graph's cycles.
- The corpus (corpus C24): R-070, R-071, R-072 each emit only PW0405 and
  convey their `@expect-error`s; A-037 and the whole accepted corpus check
  clean; the witnesses `examples/generality/read_per_element/` (generic
  callback with a match arm's key, helper extraction, cross-module, and a
  neighbour), tier 2 in PROFILES; corpus-check OK (37 accepted, 72
  rejected); generality 50 / 50.
- `scripts/bounded_checker_mutations.py`: 35 mutants, each killed by
  `tests/bounded_reads.rs`.
- `just test-compile`: the store, the accepted corpus, kiokun's slice and
  kiokun-site check (kiokun-site with its four warnings).

## Questions for the integrator

1. **kiokun-site's four reads per element** (Found 1): schedule its layer's
   set forms (`search#readings`; a set form of `pitch#of`, whose key is two
   values) and empty `KNOWN_READS_PER_ELEMENT`? Or rule otherwise on the
   interim (an error with the list, as built; a warning everywhere; or
   kiokun-site migrated before this merges)?
2. **The pair's shape needs a code of its own?** Only PW0405 and PW0406
   were assigned; interim PW0335 (`policy_value`: "a policy's value is one
   its domain has"), reason `set_pair_shape`, whose domain is "a set read
   of this keyed read".
3. **A resource no source holds**: interim `one_call` for PW0406 (ADR-0207:
   the host's own database gives every guarantee). Confirm.
4. **A call that reads and writes, per element**, is refused as a read, a
   write-only one not (the note leaves writes for later). Confirm.
