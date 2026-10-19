# ADR-0314: kiokun's search, over the whole dictionary

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
3. **The page, at kiokun.com's address** (B3; amended 2026-10-12, when
   ADR-0309 landed): `page SearchPage(q: Option<String>)` at `route
   "/search{?q}"`. Pleris has no form for a route that answers JSON, so
   `/api/search` is the integrator's, later.
   - Until ADR-0309, the page was at the temporary `route "/search/{q}"`,
     and there was no search box. A box would have needed script and a
     handler that navigates, and `navigate` follows only a command's commit
     (ADR-0280, PW5043), which a search, writing nothing, has not (the
     integrator's ruling of 2026-10-11, Questions).
   - **The search box** (`SiteSearch`, at the top of the word page and the
     search page) is kiokun.com's header field, `Header.svelte:84-127`: a
     plain `<form method="get" action="/search" role="search">` with an
     input named `q`, which **needs no script**. PW5050 holds its fields to
     the route's names. The host reads `+` and `%20` as a space and takes
     the first `q`. The word page's placeholder is its word, as kiokun.com's
     is; the search page's is "Search the dictionary".
   - **No query, or a blank one**, is kiokun.com's empty state ("Use the
     search field above …"), with nothing searched (`performSearch`
     returns on a blank query).
   - **Deinflection behind the box**: a CJK query that is a word leads the
     page with a link to it. That is the query itself where it has its own
     entry, else the first form the deinflection ADR's resolution finds, the
     link going to the query's own address, which shows that form with its
     note. kiokun.com's header does this in the browser (`navigateOrSearch`,
     `resolveWordNavigationUrl`): it goes to the word's page instead of
     searching (Difference 26).
4. **The index, cached** (B5; publication and retention amended
   2026-10-16 by ADR-XXXX-kiokun-cache under the integrator's delegation):
   the original flat-file path and automatic stale deletion below describe
   the initial implementation. The amended policy retains published
   versions while readers may hold them, builds privately, closes SQLite
   before directory publication, and reuses only validated winners.
   Originally built once per process from the CSV into
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
   (Found 2). An assumption, logged in ASSUMPTIONS.md through the
   integrator; the oracle runs both loadings once and records how much
   they differ.
8. **A search does not fail on what a reader types** (the integrator's
   ruling of 2026-10-11). `search#latin` binds each whitespace-separated
   term of the query as an FTS5 string, its inner `"` doubled, joined as
   FTS5's implicit AND (`as_words`). So `AND`, `NEAR`, `"`, `*`, a comma or a
   romanization's `-` is a word, not syntax. For a query of plain words this
   is what kiokun.com's query means already, and the oracle holds every
   other query to it. The ranks still compare the query as typed. The CJK
   statement quoted its terms already.
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
   4,375 rows of the committed CSV whose word holds 人, 学 or 學 and is at
   most two characters long, and since the reading index ADR, the Japanese
   rows read as one of the word sample's readings, from cleared sources
   only:
   - a Chinese row only where a CC-CEDICT or Unihan item gives its
     definition, and a Japanese row as JMdict's;
   - written by `scripts/kiokun_search_sample.py`, read with `git show HEAD`;
   - with hand-made aliases (`search-aliases-sample.json`: 学 and 學, and
     two canonical pages), since kiokun.com's aliases come from the
     builder's mapping and Dong Chinese's pairs, which stay local.
   - The fixture `search-sample.json` is kiokun.com's answers over that
     sample: 165 sampled queries (every 97th row's) and nine hand-made ones.

## Differences from kiokun.com

Numbered after the Contains and Appears in ADR's 24.

25. **No Cantonese romanization search**: the committed CSV has no
    `jyutping_search` column, so a Jyutping query finds what the live site,
    importing a newer CSV, may find. A data difference, until the owner
    commits a current CSV.
26. **The search box searches, and leads to the word** (Decision 3).
    - kiokun.com's header, with script, sends a CJK query that is a word
      straight to the word's page, and a single CJK character always.
    - Here the box sends every query to the search page, which needs no
      script, and leads with the word's link above the results.
    - kiokun.com's suggestions as one types, its handwriting, its simplified
      to traditional lookup and its sentence reader are not built.
    - (Until ADR-0309 this Difference was the page's temporary address,
      `/search/{q}`; that is closed.)
27. **What kiokun.com fails on, the rewrite answers** (Decision 8). kiokun.com
    binds the query as FTS5's query language, so a query FTS5 reads as
    syntax is a 500 from its endpoint, and its page shows "Search is
    unavailable". The oracle's sample found four:
    - `bingmaweidong, liangcaoxianxing` (a comma);
    - `ime-ji`, `monsuta-` and `maikuroa-ruenue-`, romanizations whose
      long-vowel `-` FTS5 reads as a column filter;
    - and `"`, `AND` and `*` fail as well.
    The rewrite answers each as words. The harness counts each by name ("a
    search kiokun.com fails, answered"); a page that failed on one would be
    a difference.
28. **The rest of each column is a `<details>` disclosure**, where kiokun.com
    shows twelve more each time the reader presses "Show more". It needs no
    script.
29. **No community words**: kiokun.com also searches its users' custom
    words (`/api/custom-words`), which need accounts (step 4).
30. **The page's head** is the rewrite's own: "Search Results | Kiokun",
    not indexed. kiokun.com's search page sets none of its own, so its
    layout's applies; the layout is not ported yet.

## Found

1. **The oracle found nothing else to name**: of 355 queries over the whole
   index (834,036 rows), 351 have the same answers. kiokun.com fails the
   other 4, and the rewrite answers them (Difference 27).
2. **`is_common`'s type decides kiokun.com's Common badge, and its order.**
   Over the oracle's 355 queries, loaded as text, 331 answers differ from
   the integer loading's. Every hit is marked Common: 6,542 of 6,542, where
   2,183 are as integers. And `ORDER BY is_common DESC` no longer orders
   anything. If production imported the CSV as kiokun-data's notes say, its
   search is like the text loading. That cannot be checked here: robots.txt
   keeps the live site from this track. For the owner.
3. **The repository's index sample has no Korean rows**: a Korean row's word
   is Hangul and holds none of the sample's characters. Korean search is
   held locally, over the whole index.

## Acceptance

- **The server's tests**:
  - `search_reads_kana_variants_and_canonical_pages`: ヒト finds 人 by its
    reading; 学 finds 學; 学生 is one hit linking 學生, read in Japanese and
    Chinese; eight hits a column and the rest disclosed; nothing found is
    said. Each of these answers is kiokun.com's own.
    - The rewrite's own answers, where kiokun.com answers 500: the four
      inputs of Difference 27, `"` and `*` find nothing; `AND` finds what
      `and` finds; `renho-` finds 人和, read レンホー; `person, people` finds
      the three rows holding both words.
  - `search_matches_kiokuns_answers_for_the_sample` (CI): the fixture's 174
    queries. Nine of them, the hand cases above, are kiokun.com's 500s,
    each answered.
  - `search_matches_kiokuns_own_on_a_sample` (local, ignored).
  - `the_search_box_sends_its_query_to_the_search_page`:
    - the word page's form, a GET to `/search` naming `q`, its placeholder
      the word;
    - `/search`, `?q=`, `?q=%20` and `?q=+` show the empty state and no
      hits;
    - `+` is a space;
    - 人 leads to its page, and a Latin query leads nowhere, even one with
      its own entry (OK);
    - 食べた leads to its address, shown as 食べる (past).
  - The layer's own tests (`kiokun::search`): the CSV reader; a query as
    words, whatever it holds; and an index answering what FTS5 cannot read
    raw, plain words finding what they found raw.
- **The scripts' tests**: the D1 stand-in (`d1.test.mjs`, four), and the
  index sample's generator.
- **The oracle** (`just e14-kiokun-search`, local): 355 queries, 0 unnamed
  differences, kiokun.com's 4 failed searches answered, counted by name; and
  `is_common` loaded both ways (Found 2).
- **Mutation controls**: twelve in `scripts/kiokun_word_mutations.py`, in the
  program and the layer (the expansion, katakana, canonical pages, the
  display score, the pronunciation order, the definition cut, CJK detection,
  the eight shown, the exact rank, the reading clause, a Latin query bound
  as FTS5's syntax, a term's quote not doubled), each killed; four for the
  box (a blank query searched, a word's own entry or an inflected query
  leading nowhere, a Latin query with an entry leading to it), measured
  with `mutant_killers.py` and confirmed through the suite's cover. Reading
  a Latin query back as a word is equivalent, since it has no kana ending
  and its one reading, a bare stem and る, names no entry; and two in `scripts/kiokun_inventory_mutations.py` for the
  index sample's generator (20 of 20 killed). The word script is run whole
  by the branch's verify (ADR-0281).

## Questions asked

- **The page's address** (W6, 2026-10-10): a page query parameter in the
  language, a development-server seam mapping `/search?q=`, or `/search/{q}`
  with navigate.
  - The integrator, 2026-10-11: keep `route "/search/{q}"` with a form
    submitted by navigate, name the temporary route and the form's need for
    script, and keep `q` a page parameter.
- **The form by navigate cannot be written** (W6, 2026-10-11). `navigate`
  follows a command's commit (ADR-0280, PW5043). A command a handler calls
  declares `idempotent_by` (PW0338), whose record commits with its writes
  (PW0348). And kiokun's sources take no transactions. Asked: (a) navigate
  without a command, (b) a stand-in store, or (c) no box until a route reads
  its query part.
  - The integrator: **(c)** (PARALLEL.md, 1e84f9a6). A link and a GET form
    already express a navigation that commits nothing, without script, and
    ADR-0280 keeps `navigate` for a commit's consequence. A stand-in store
    is a fake write per search. Typed page query parameters
    (`route "/search{?q}"`) are the integrator's next work (Decision 3).
- **A search must not fail on what a reader types** (the integrator,
  2026-10-11): Decision 8 and Difference 27.
- **Both loadings of `is_common`** (the integrator, 2026-10-11): run once,
  locally (Found 2).
- **B1 to B5** (W6, 2026-10-10): the integrator's rulings, as Decisions 1
  to 7 record them.

## Not claimed

- **`/api/search`**, the JSON endpoint the header's dropdown reads: no
  endpoint form yet (Decision 3).
- **The header's suggestions and handwriting input**: client features
  (the inventory's).
- **The reading index, homophones and deinflection** (`/api/lookup-reading`):
  the step's next parts.
