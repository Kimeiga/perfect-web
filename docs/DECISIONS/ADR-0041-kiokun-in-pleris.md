# ADR-0041: kiokun's shard rule and ranking, in Pleris

Status: accepted under the owner's instruction of 2026-09-25 ("ok do all of
that"). Decisions marked **(ruling needed)** were made without a ruling and are
offered for reversal. Date: 2026-09-25. Milestone: E10.

## Context

The kiokun slice (ADR-0037) compiled two things from Pleris. The lookup follows
one redirect. The search passed its query to the host and returned the host's
ranking. kiokun's logic, its shard rule and its ranking, was Rust in the host.
`docs/NEXT.md` named moving them into Pleris as the test of the language's
computation, and ADR-0039 and ADR-0040 built what that needed.

## Decision

### 1. The shard rule is Pleris

`examples/kiokun/Shards.pw` states kiokun's rule:
1. count the Han characters;
2. hash `h = h * 31 + codepoint` in 32 bits;
3. name the shard;
4. write the subdirectory as two hex digits.

`pw build` compiles it to two components, and neither imports anything:
- `shards.Place` (5 KB) places one word;
- `shards.Places` is the same rule over a list.

The host places every file it loads with `Places`, a thousand words a call,
and any other word with `Place`.

### 2. A lookup reaches every shard

kiokun's files sit at `<subdirectory>/<file name>.json.deflate`, whatever shard
the word is in. So the host reads a word outside the loaded shard on demand,
where the compiled rule says it is, and caches it. With a kiokun-data checkout,
the slice's `/<word>` answers for all 1.49 million entries kiokun has. A stub
whose target is in another shard is followed.

A file name is not always its word. kiokun's `create_safe_filename`
(kiokun-data `src/main.rs`) writes `_` for each of these:
- the nine characters `/ \ : * ? " < > |`;
- every control character.

The subdirectory is still the word's own hash, so several words can share one
file name. The host handles it in three places:
- a file whose name holds `_` is read by the `key` it records;
- a lookup takes a file only if the file records the word looked up;
- a word holding `/` is reached as `%2F`. `/` still separates path segments,
  and no word can name a path outside kiokun's directory, because the file
  name escapes every separator.

Search stays over the loaded shard. kiokun searches in its database, and an
index over every shard is the deployment's to build.

### 3. The ranking is Pleris; the index is the deployment's

`kiokun.page.Search` ranks in four steps:
1. score each candidate row;
2. keep one hit per entry, with the languages it matched in;
3. order the hits;
4. keep twenty.

The index does what kiokun.com's SQLite FTS5 does:
- returns candidate rows, a superset of every row that scores;
- folds case;
- splits definitions into words;
- knows which entry a stub names.

That is kiokun.com's division: the database retrieves and the application
ranks.

### 4. The Rust versions stay, as references

The Rust functions are the rule and the ranking kiokun.com has:
- `shard::shard_of` and `shard::subdirectory`;
- `data::Index::search`.

They are compiled only into the host's tests now. Every compiled answer is
compared with them.

## Found while building it

1. **The E9 value relations kept a callee's uninstantiated `T`.**
   - `infer.rs` typed `let ys = List.filter(xs, ..)` with `filter`'s declared
     result, `List<T>`.
   - The value relations skip every name already bound, so `ys` stayed
     `List<type parameter 0>`.
   - `List.sort_by(ys, compare)` was then refused (PW0605), over a mismatch
     the typer itself had made.

   E9 claims generic callables are instantiated per call, and this let
   binding was not. Such a binding is now dropped and the call solved. A
   regression test covers it.
2. **A statement keyword could name a value.** `let query = ..` parsed.
   Then every use of `query` in an expression parsed as a `query ..`
   statement, which the checker reads as a value of no known type. The
   program checked and meant something else, and the backend was the first
   to refuse it. The parser now refuses a binding or parameter named with a
   statement keyword (PW0009). **(ruling needed)** The alternative is
   contextual keywords, which would keep such names usable.
3. **`List.group_by` was missing.** Grouping rows by entry needed runs of
   equal keys, as views that copy nothing. The key is a `String`, and a run is
   adjacent elements, as Rust's `chunk_by` is. ADR-0040's table is amended.
4. **A refusal said "this expression".** The backend now names every kind of
   expression.
5. **kiokun's file names are not its words.** The compiled rule's first run
   over every file found 32 files outside the subdirectory their names hash
   to, and the first fix was a guess from the data: read `_` as `/`. The guess
   was wrong for a 33rd file, `핏대(가) 서다[나다/돋다/오르다]`, whose name
   hashes to the same subdirectory under either spelling:
   - `31 ≡ -1 (mod 32)`, and `_` is `/` + 48;
   - so two escapes at positions of opposite parity cancel in the hash's low
     eight bits, which are all the rule reads.

   That guess would have stored the entry under a word kiokun does not have.
   kiokun's builder source states the escape, and the host now follows it
   (§2).
6. **A component call per word made loading three times slower.** Each call
   is a fresh instance, about 20 µs. Placing 1.49 million files through
   `Place` took the whole shard's load from 9.5 s (kiokun's Rust rule) to
   32.6 s. `Places` answers a thousand words a call, and the load took 8.6 s
   and 10.4 s in two runs (`docs/evidence/E10/kiokun.txt` records the 8.6 s).
   Per-call instantiation is ADR-0032's, and remains a known cost.

## Acceptance

- `spikes/kiokun/server` tests:
  - `Place` and `Places` agree with kiokun's rule on every sample word and on
    2,500 generated words;
  - `Search` agrees with kiokun's ranking, hit for hit and field for field, on
    every query derived from the sample's index;
  - an escaped file name is read by its key. A lookup of another word with
    the same file name and subdirectory is not answered from that file, and
    `%2F` reaches a word holding `/`;
  - the pages, as before.
- On a kiokun-data checkout, ignored in CI:
  - the compiled rule agrees with kiokun's on every word kiokun has;
  - every word is found in the subdirectory the rule names, with the 33
    escaped names read by their keys;
  - `Search` agrees on every query derived from the whole shard;
  - the whole shard looks up and renders;
  - each of the 33 escaped words is served from its file through the route.
- The E10 oracle runs 300 cases each:
  - `Search` against a Rust statement of its ranking, over an index generated
    around each query;
  - `Place` and `Places` against the rule.

  A reference that keeps twenty-one hits is caught.
- Mutation controls (`scripts/kiokun_mutations.py`, `just e10-kiokun-mutants`):
  twelve mutants. Each undoes one piece of this ADR:
  - `group_by`;
  - the two defects found (items 1 and 2);
  - kiokun's rule and ranking in Pleris;
  - the host's escape handling and batching.

  Each must fail a test.
- The browser spec, in three engines.
