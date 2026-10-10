# ADR-XXXX: kiokun's search, over the whole dictionary

Status: accepted under the owner's delegation of 2026-10-02, on the
integrator's rulings for track `kiokun` (docs/PARALLEL.md; its rulings of
2026-10-10 on search, B1 to B5). Date: 2026-10-10. Milestone: E14,
kiokun.com in Pleris, step 2 (search).

## Context

- **kiokun.com's search is SQL over one FTS5 table**, not ranking code in
  TypeScript. Migration 0005 (`sveltekit-app/migrations/0005_search_index_with_jyutping.sql`)
  creates `dictionary_search USING fts5(word, language, definition,
  pronunciation, reading_search, jyutping_search, is_common, tokenize =
  'porter unicode61')`.
  - `api/search/+server.ts` runs one of two statements. A query holding a
    CJK or Hangul unit runs the CJK statement: each term a word prefix, a
    kana query's reading too, ranked exact, reading, prefix, then common,
    shorter, earlier. Any other query runs the Latin statement: the query
    matched over every column, ranked by definition and romanization.
  - Before it, `getCjkSearchTermsFromData` expands a CJK query into up to
    24 terms: the query, its aliases, each character's script variants,
    and their aliases (`src/lib/generated/search-aliases.json`).
  - After it, the handler groups rows by the page each names (a Japanese
    form's canonical page), picks each group's shown row by a display
    score, joins its pronunciations, and keeps 60, as the search page asks.
  - The search page shows three columns, eight hits each and twelve more
    as the reader asks, each hit linking its page.
- **The index is the builder's CSV**, imported into D1. kiokun-data commits
  it as `output_search_index.csv` at a47956fb (February), with six columns
  and no `jyutping_search`, which the builder now writes (seven, at HEAD).
- **Both SQLites have FTS5**: the workspace's bundled one (libsqlite3-sys
  0.38.1 builds with `-DSQLITE_ENABLE_FTS5`) and Node 22's `node:sqlite`
  (SQLite 3.50.4).

## Decision

1. **The data** (B1): the committed CSV, a47956fb, read by the oracle and
   the rewrite alike, so the comparison is like for like. Asking the owner
   to rebuild it is the integrator's.
2. **SQL in the layer, kiokun-specific** (B2). The program declares
   `source KiokunSearch holds SearchRow, AliasSet; transactions none; reads
   strong; changes none`, and five host operations:
   - `search#cjk(terms, reading, limit)` and `search#latin(query, limit)`
     run kiokun.com's two statements as written at HEAD, over migration
     0005's schema, ported;
   - `search#aliases`, `search#variants` and `search#canonical` read the
     aliases' three tables.
   - The program keeps the rest: the CJK and kana detection, the terms'
     expansion, the reading's katakana-to-hiragana, the CJK limit, the
     grouping, the display score, the pronunciation join, the cut and the
     page.
   - Ranking stays FTS5's: reimplementing the Porter and Unicode tokenizer
     would drift. A program cannot yet declare a full-text index or its
     ranking. A generic platform full-text source waits for a second real
     use (DoorDash's store search is the likely one), and the integrator
     logs that in NEXT.
3. **The page now, at a temporary address** (B3): `page SearchPage(q)` at
   `route "/search/{q}"`, as `/word/{word}` was. Pleris has no form for a
   route that answers JSON, so `/api/search` is the integrator's, later;
   and a page reads no query string yet (ADR-0295), so kiokun.com's
   `/search?q=` waits on the integrator (Questions).
4. **The index, cached** (B5): built once per process from the CSV into
   `target/kiokun-search/kiokun-search-<digest>.db`, the digest of the
   CSV's bytes and the schema, a stale one removed when a new one is built;
   a CSV under 4 MiB is indexed in memory. Nothing is written in either
   repository. The recipe checks the disk before it builds. `rusqlite`
   joins the development server's dependencies from the workspace, where
   pw-materialize already takes it: no new crate.
5. **`is_common` is an integer**, as `output_search_index.sql` writes it.
   Loaded from the CSV as text, kiokun.com's `Boolean(row.is_common)` would
   read `'0'` as true and mark every hit Common. kiokun-data's notes
   describe importing the CSV with the shell's `.import`, which stores text
   (Found 2).
6. **Held to kiokun.com's own code** (B4): `just e14-kiokun-search` runs
   kiokun.com's `GET` handler, copied from HEAD, in Node:
   - over `node:sqlite`, with migration 0005 run as written and the CSV
     loaded as above;
   - with `platform.env.DB` a stand-in for D1's binding (`d1.mjs`): only
     `prepare().bind().all()`, each in D1's documented shape (`{ results,
     success, meta }`), with a test of its own (`d1.test.mjs`, run in CI
     by `scripts/tests/test_kiokun_d1_adapter.py`);
   - with SvelteKit's `json` and `error` a stated stand-in (`json` answers
     a JSON `Response`, `error` throws its status and message), and every
     other import refused;
   - over a deterministic query sample: every 8,000th row's word, its
     romanization, a Japanese row's kana, and its definition's first word.
7. **CI holds a sample** (B5, A): `examples/kiokun/data/search-sample.csv`,
   2,765 rows of the committed CSV whose word holds 人, 学 or 學 and is at
   most two characters long, from cleared sources only:
   - a Chinese row only where a CC-CEDICT or Unihan item gives its
     definition, and a Japanese row as JMdict's;
   - written by `scripts/kiokun_search_sample.py`, read with `git show HEAD`;
   - with hand-made aliases (`search-aliases-sample.json`: 学 and 學, and
     two canonical pages), since kiokun.com's aliases come from the
     builder's mapping and Dong Chinese's pairs, which stay local.
   - The fixture `search-sample.json` is kiokun.com's answers over that
     sample: 97 sampled queries and six hand-made ones.

## Differences from kiokun.com

Numbered after the Contains and Appears in ADR's 24.

25. **No Cantonese romanization search**: the committed CSV has no
    `jyutping_search` column, so a Jyutping query finds what the live site,
    importing a newer CSV, may find. A data difference, until the owner
    commits a current CSV.
26. **The page's address is `/search/{q}`**, not `/search?q=`, until a page
    can read a query string (Decision 3); the header's search box, which
    submits `?q=`, is not built.
27. **A failed search fails the page.** A query FTS5 cannot read (`"`,
    `AND`) is answered with kiokun.com's error, 500, by the endpoint, and
    its page shows "Search is unavailable"; the rewrite's query fails, and
    its page with it. The harness counts each by name.
28. **The rest of each column is a `<details>` disclosure**, where kiokun.com
    shows twelve more each time the reader presses "Show more". It needs no
    script.
29. **No community words**: kiokun.com also searches its users' custom
    words (`/api/custom-words`), which need accounts (step 4).
30. **The page's head** is the rewrite's own: "Search Results | Kiokun",
    not indexed. kiokun.com's search page sets none of its own, so its
    layout's applies; the layout is not ported yet.

## Found

1. **The oracle found nothing to name**: 355 queries over the whole index
   (834,036 rows), the same; 4 of them failed alike (Difference 27).
2. **`is_common`'s type decides kiokun.com's Common badge.** If production
   imported the CSV as kiokun-data's notes say, every hit is marked Common.
   That cannot be checked here: robots.txt keeps the live site from this
   track. For the owner.
3. **The repository's index sample has no Korean rows**: a Korean row's word
   is Hangul and holds none of the sample's characters. Korean search is
   held locally, over the whole index.

## Acceptance

- **The server's tests**:
  - `search_reads_kana_variants_and_canonical_pages`: ヒト finds 人 by its
    reading; 学 finds 學; 学生 is one hit linking 學生, read in Japanese and
    Chinese; eight hits a column and the rest disclosed; nothing found is
    said; `"` and `AND` fail. Each answer is kiokun.com's own.
  - `search_matches_kiokuns_answers_for_the_sample` (CI): the fixture's 103
    queries.
  - `search_matches_kiokuns_own_on_a_sample` (local, ignored).
  - The CSV reader's own test (`kiokun::search`).
- **The scripts' tests**: the D1 stand-in (`d1.test.mjs`, four), and the
  index sample's generator.
- **The oracle** (`just e14-kiokun-search`, local): 355 queries, 0 unnamed
  differences, 4 failed searches counted by name.
- **Mutation controls**: ten in `scripts/kiokun_word_mutations.py`, in the
  program and the layer (the expansion, katakana, canonical pages, the
  display score, the pronunciation order, the definition cut, CJK detection,
  the eight shown, the exact rank, the reading clause), each killed when
  checked by hand; and two in `scripts/kiokun_inventory_mutations.py` for the
  index sample's generator (20 of 20 killed). The word script is run whole
  by the branch's verify (ADR-0281).

## Questions asked

- **The page's address** (W6, 2026-10-10): a page query parameter in the
  language, a development-server seam mapping `/search?q=`, or `/search/{q}`
  with navigate. ANSWER
- **B1 to B5** (W6, 2026-10-10): the integrator's rulings, as Decisions 1
  to 7 record them.

## Not claimed

- **`/api/search`**, the JSON endpoint the header's dropdown reads: no
  endpoint form yet (Decision 3).
- **The header's suggestions and handwriting input**: client features
  (the inventory's).
- **The reading index, homophones and deinflection** (`/api/lookup-reading`):
  the step's next parts.
