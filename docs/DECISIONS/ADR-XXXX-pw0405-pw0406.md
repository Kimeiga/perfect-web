# ADR-XXXX: a read is bounded by construction, the checker's half (PW0405, PW0406, PW0407)

Status: proposed by the worker of track `bounded-checker`, for the
integrator's merge. It builds step 2 of the design note "a read is bounded
by construction" (W9's, `docs/DECISIONS/ADR-XXXX-a-read-is-bounded-by-construction.md`
on `origin/track/bounded-reads`), which the integrator accepted on
2026-10-14, ruling the order of work: (1) the host's render merge (its own
track), (2) this, the checker's rules, (3) the per-layer conformance suite,
(4) the store's composites above the seam. Date: 2026-10-14. Milestone: E14.
The integrator ruled the four questions the first build raised on
2026-10-15; each ruling is marked "ruled by the integrator, 2026-10-15"
where it applies, and the four are together under "The integrator's
rulings".
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
    `c.replies`). A parameter passed whole is passed along, not walked, and
    a read keyed by it is not refused: to its own position of the same
    function, or handed on through a cycle of several (`ping` and `pong`
    giving `name` to each other while a count falls), where it is walked
    only if the function it lands in walks that position (`pong` stepping
    `item + 1` before it hands it back). The recursive call is the
    recursion's turn, not a read.
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

### `set`, a keyed read's set form (held by PW0407)

`set hours_of_stores` on `fn hours_of(store: StoreId)` (a clause on a
function, `Domain::SetRead`). Held to the note's shape, by a rule of its own
(PW0407, below):
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
A `set` that does not fit pairs nothing, and is refused with **PW0407**
(`set_pair_shape`): **ruled by the integrator, 2026-10-15** (question 2),
that the pair's shape has a code of its own and each refusal names its rule,
where the first build refused it as PW0335's `policy_value` (reason
`set_pair_shape`). The message says which shape fails, in the clause's own
words (`set cents_of`: `cents_of` answers `List<Int>`, and a set read of
`price_of` answers a list of `Price` with its `Int`), with a note at the set
read it names where it names a function, and PW0335 stays what it was, a
`sets` value outside its domain (`chunked(0)`, `batched`). PW0407 is the next code
free beside PW0405 and PW0406: no code of PW0404 to PW0499 but PW0404's is
registered on `master` or on any `origin/track/*` branch (2026-10-15).

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
every guarantee (ADR-0207): one call, no warning (**ruled by the
integrator, 2026-10-15**, question 3: confirmed).

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
  `read_per_element`, PW0406 `set_read_per_key` and PW0407
  `set_pair_shape`, each `Owner::Effects` (the block has no track owner; the
  integrator assigned the first two on 2026-10-14 and ruled the third on
  2026-10-15).

## Found (corrections first)

1. **The note's migration, "no program changes", holds for the store and
   the feed, and not for kiokun.com.** `pw check` over every reference
   program: the store, the accepted corpus, the feed and the kiokun slice
   read nothing per element. kiokun-site (`examples/kiokun-site/app.pw`)
   reads four times per element, each as kiokun.com does:
   `kiokun.site.Word` asks `search_reading` once for each reading twice
   (app.pw:3133, :3175) and `read_pitch` once for each word (:3168), and
   `kiokun.site.Search` asks `search_reading` once for each reading
   (:3945). The interim, conservative and cheap to reverse, **ruled to stand
   by the integrator, 2026-10-15** (question 1): PW0405 is an error
   everywhere, and these four, listed with their reasons in
   `bounded::KNOWN_READS_PER_ELEMENT`, are reported as warnings that say why
   they wait. A test holds the list to exactly these (a stale entry fails,
   and the same read in a declaration the list does not name is an error).
   The list empties when kiokun's layer
   (`spikes/own-renderer/server/src/kiokun.rs`) has the set forms and the
   program reads before its loops: see Next, which is not built here.
2. **A correction to the first build: PW0405 refused a mutual recursion
   that walks nothing** (CI's finding, 2026-10-15, after green mutation
   runs of 35 and of 39 mutants). `just ci` failed one test target,
   `what_a_component_does`, whose `ping` and `pong` hand `name` to each
   other whole while a count falls, reading by `name` at the bottom: one
   key, read again at each turn. The rule exempted a parameter passed whole
   only to its own position of the same declaration, so `ping(name, n)`
   calling `pong(name, n - 1)` walked `name`, and the read was refused as a
   recursion over data. A whole parameter makes no new key: it is now an
   edge between positions, walked only where the declaration it lands in
   walks that position (a fixed point over the cycle), and a test and two
   mutants hold it (40 mutants in all). The same run found a true positive
   in the same file: `a_function_passed_by_name_is_compiled_in_and_
   performed_where_named` mapped a reader over its list, incidental to its
   subject (the contract), and reads by a literal key now. The mutation
   script runs `bounded_reads` alone, so no mutant could show either: a
   rule that refuses programs meets every program the repository has only
   in `test-unit`, which is where it was found.
3. **Three generality witnesses of `undeclared_effect` read per element**
   (`List.map(ids, id => Stores.get(id))`), incidentally: their subject is
   the row. They read `Stores.get(home)` now, a parameter, keeping their
   shapes and verdicts (commit 0dd29f80).
4. **The note's example is exactly what the store warns** once `set
   hours_of_stores` is written and `StoreData` states no `sets`: PW0406 on
   `StoresOpen` (hours.pw) and on `Search` (search.pw). The two annotations
   below must land together. The note's position, hours.pw:391:17, is
   392:17 with the `set` line.
5. **`check_latency`'s 2 s ceiling failed at 2.77 s** on this PC while
   other workers' builds held the load average at 76 to 88. The rules cost
   the store's check (52 files, debug) 9 to 24 ms for the program's facts
   and 6 to 11 ms for every unit's checks, about 2% of it.
6. **The pair's shape cannot tell which field is the key** (2026-10-15,
   probing PW0407's neighbours). "A record with a field of the key's type"
   is satisfied by any field of that type: `fn slot_of(store: Int)` with `set
   slots_of`, `slots_of(stores: List<Int>) -> List<Slot>` and `Slot { day:
   Int, open: Bool }` checks clean, though no field of `Slot` is the store.
   The shape the note states is held as stated, and the ruling concerned the
   code, so it is not changed; a tighter one names the field (the key
   parameter's own name) or declares it, and would bring the multi-value key
   of Next with it.

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
- **The generality figure**: master's 54 / 54 (train-7) becomes 56 / 56
  (`read_per_element` and `set_pair_shape`); `tests/generality.rs` says so,
  and `docs/STATUS.md` and `docs/NEXT.md` carry the number.
- **Corpus C25**, for `docs/CORPUS.md` (the corpus version's number is the
  integrator's to give as trains merge, so this track writes no entry):
  R-075, R-076, R-077 (PW0405, one per loop form), R-078 (PW0407) and A-037,
  with the generality witnesses `read_per_element` and `set_pair_shape`.
  Master's NEXT 6c holds R-070 to R-074 as C24; this track's first build
  named R-070, R-071 and R-072, which collided, and renumbered at the merge
  of train-7 (R-075 to R-079 are this track's, R-079 free).
- **KNOWN_LIMITATIONS**: kiokun-site's four reads per element (Found 1);
  and the Not claimed below.
- The root `justfile` imports `just/bounded-checker.just` (one line, as each
  track's import was added).
- **Shared files this track changed**, each in a commit of its own that says
  why: `compiler/pw-core/tests/what_a_component_does.rs` (ADR-0283's: one
  test's named function reads by a literal key, Found 2), the three
  `undeclared_effect` witnesses (Found 3), `tests/generality.rs` (the
  figure) and `tools/corpus-check/src/main.rs` (the categories).

## Not claimed

- **The bound's emission beside the plan** (the note's evidence: each
  page's and command's calls by source, each one's cost, checked against
  `Counted` in the server's tests). Next: `Sets::calls` is the cost per set
  read, and `Inferred::calls` the calls; a page's plan (`page_values`) and a
  command's contract are where it would be written.
- **The rewrite** of a loop's reads into its set read: a later step, as the
  note orders.
- **Writes per element** (**ruled by the integrator, 2026-10-15**, question
  4: confirmed): a call that only writes is not refused (`for l in lines {
  record(l.item) }`), and is not claimed bounded either: a write per
  element is as many round trips as the list is long, and no rule here
  counts it; one whose row reads and writes is refused as a read. The note
  leaves writes for a later step.
- **A key of two values**: the pair takes the keyed read's first parameter
  as its key and the rest as passed along; `read_pitch(shard, word)`, whose
  key is both, has no set form this shape accepts (Next).
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
- **PW0335 for the pair's shape**, as the first build had it: a policy's
  value is one its domain has, and the domain of `set` is "a set read of
  this keyed read", which the policy table cannot state, so the refusal
  said "a policy's value" of what is a rule about two functions. The
  integrator ruled it a code of its own on 2026-10-15 (PW0407).

## Acceptance

Recorded by `just e14-bounded-reads` in `docs/evidence/E14/bounded-reads.txt`
(the recipe's local run is not committed: evidence of record comes from the
verify run at the train). This track's own runs, each over the tree it names:

- `compiler/pw-core/tests/bounded_reads.rs`, 30 tests: PW0405 in each loop
  form (seven combinators and a pipe, a `for` through a `let`, a recursion
  over a list and over a count, a function of the program's passing its
  callback on and calling it in a `for`, a `let`-bound callback, a named
  callback, a helper, a nested loop), each key path, and its controls (a key
  no element's, the set read, a callback applied once, a parameter passed
  along, directly or through a mutual recursion, a helper keyed by no
  element, a write, a read before the loop); a call that reads and writes
  refused as a read; ADR-0022's perturbations (renamed, rebound, a decoy);
  PW0406 on `per_key` and on no `sets`, none on `one_call`, `chunked(100)`,
  the host's database (alone, and beside a source that cannot batch) or a
  read no `set` names; the pair's shapes, PW0407's own facts (code, symbol,
  invariant, span, note) and a bad `sets` value still PW0335; the clauses'
  places and values; `Sets` and its budget; `source_clauses`, and an older
  `sources.json` read as `per_key`; the store (clean, mutated per element
  and refused, with the two annotations); the feed; kiokun-site's known
  four. And `bounded`'s own: the call graph's cycles.
- The corpus (corpus C25): R-075, R-076, R-077 each emit only PW0405 and
  R-078 only PW0407, and convey their `@expect-error`s; A-037 and the whole
  accepted corpus check clean; the witnesses
  `examples/generality/read_per_element/` (generic callback with a match
  arm's key, helper extraction, cross-module, and a neighbour) and
  `examples/generality/set_pair_shape/` (a reader the set form drops,
  another module's set form of another source, and a neighbour of the
  shapes a set form may take), tier 2 in PROFILES; corpus-check OK (37
  accepted, 78 rejected); generality 56 / 56.
- `scripts/bounded_checker_mutations.py`, run whole: **40 of 40 mutants
  killed** by `tests/bounded_reads.rs` alone, none "does not build"
  (17:03 to 17:11Z, 2026-10-15, at 7da0d2ca; the first build's run of 6b290a8f
  killed 35 of 35, and a run at 2a186b63 39 of 39, before CI's finding).
- The reference programs, `pw check`: the store (52 files), the accepted
  corpus (84), the feed (23) and kiokun's slice (27) with no diagnostics;
  kiokun-site (25) with its four warnings; `scripts/mutant_programs.py`: 237
  mutants of a program's source, 0 failing.
- `just ci` at 7da0d2ca (17:11 to 17:28Z, 2026-10-15), `ci: OK`: 337 test
  binaries, 2914 tests passed and none failed (`check_latency` among them, at
  a quiet load; `what_a_component_does` 8 of 8), `test-compile` (the store,
  the accepted corpus, kiokun's slice and kiokun-site check, kiokun-site
  with its four warnings) and `mutant-programs` (237 mutants, 0 failing).
  The first `just ci`, at 2a186b63, failed `what_a_component_does` (Found 2).
  The push is chained to a second `just ci` over the commit that holds this
  text.

## Next: closing kiokun-site's four reads (not built here)

Ruled by the integrator, 2026-10-15 (question 1): the four stay warnings
through `KNOWN_READS_PER_ELEMENT` as the interim, and closing them is this
list, which waits on kiokun's layer and on one design decision of the pair's
shape. Nothing below is built by this track.

1. **`search#reading` has a set form that fits the shape as written.**
   `search_reading(reading: String, limit: Int)` answers `List<ReadingHit>`;
   a set form `search_readings(readings: List<String>, limit: Int) ->
   List<ReadingFound>` (`ReadingFound { reading: String, rows:
   List<ReadingHit> }`, the record the program already builds) is given a
   list of the key and the same `limit`, and answers each key with its rows.
   Kiokun's layer (`spikes/own-renderer/server/src/kiokun.rs`, beside
   `kiokun:data/search#reading`) gains `search#readings`; the program names it
   with `set search_readings` and reads before its three loops
   (`kiokun.site.Word` twice, `kiokun.site.Search` once, the repair PW0405
   prints). The two `search_reading` entries of the list go in the same
   commit; the test that holds the list exact fails on a stale entry.
2. **`pitch#of`'s key is two values, which the shape does not accept.**
   `read_pitch(shard: String, word: String)`: PW0405 keys a host read by
   every parameter (the element `w` gives both, `pitch_shard(speak_of(w))`
   and `speak_of(w)`), where the pair takes the first parameter alone, the
   shard, as its key and the rest as passed along. A set form given the
   loop's words, `read_pitches(words: List<String>)`, is refused ("takes
   `(List<String>)`, and a set read of `read_pitch` takes `(List<String>,
   String)`"); one given the shards and the word reads one word's readings
   in each shard, which is no read the loop makes. The options, for an ADR
   with the kiokun track:
   - **The layer derives the shard from the word**: `read_pitch(word)`, one
     key. Cheapest, and it moves `pitch_shard`'s hash (a port of
     `PitchAccent.svelte`, written in the program today) into the host's
     Rust: app-specific Rust, against the steering of 2026-10-15. Not
     recommended.
   - **The pair's shape learns a key of several values**: the set form is
     given a list of the record the key parameters form
     (`read_pitches(keys: List<PitchKey>)`, `PitchKey { shard, word }`), and
     answers each record with its readings. The shape's rule changes from
     "the first parameter" to "the parameters that key the read", which are
     all of a host read's (as PW0405 already has it) minus a declared
     reader passed along: a clause or a convention to decide, and
     `fits`/`keyed` to change together, with tests and mutants as here.
     Recommended: it is the general form, and `search_reading`'s `limit`
     (a bound, not a key) shows a read already has parameters that are not
     keys.
3. **Then `KNOWN_READS_PER_ELEMENT` is empty** and the constant, the
   `known` path and the test `a_known_read_is_known_only_where_the_list_names_it`
   go with it: PW0405 is an error everywhere already. Until then each entry
   is a read a person decided to wait on, with its reason in the warning.

## The integrator's rulings, 2026-10-15

The four questions the first build raised, each **ruled by the integrator,
2026-10-15**, and where each is built or held:

1. **kiokun-site's four reads per element**: they stay warnings through
   `bounded::KNOWN_READS_PER_ELEMENT`, which a test holds exact, as the
   interim. Closing them needs kiokun's layer to gain set forms and a
   two-value key (`read_pitch`'s (shard, word)), which the pair's shape does
   not accept yet: written as Next above, not built here.
2. **The pair's shape gets a code of its own**, each refusal naming its rule:
   PW0407 `set_pair_shape`, the next code free beside PW0405 and PW0406
   (checked on `master` and on every `origin/track/*` branch), instead of
   PW0335. Built: the code, `bounded::pair`, the test
   `a_set_that_does_not_fit_is_the_pairs_own_rule_and_a_bad_sets_value_is_a_policys`,
   the corpus program R-078, the generality witnesses
   `examples/generality/set_pair_shape/`, and two mutants.
3. **A resource no source holds counts as `one_call` for PW0406**
   (ADR-0207's default): confirmed. Held by
   `a_set_read_on_a_source_that_batches_is_not_warned`, with a source that
   cannot batch beside it, and a mutant.
4. **A call that reads and writes per element is refused as a read; a
   write-only call per element is not**: confirmed, and a write-only call per
   element goes under Not claimed. Held by
   `a_call_that_reads_and_writes_per_element_is_refused_as_a_read` and
   `what_is_no_read_per_element_is_accepted`, and a mutant.
