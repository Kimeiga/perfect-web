# ADR-XXXX: kiokun's reading index and the word page's Homophones

Status: proposed (track `kiokun`, W6), under the owner's delegation of
2026-10-02. Date: 2026-10-11. Milestone: E14.

## Context

- **The step** (docs/PARALLEL.md, W6's plan, step 2): after search, the
  reading index, homophones and deinflection.
- **kiokun.com's reading index is its search table.** `GET
  /api/lookup-reading?q=&limit=` (`src/routes/api/lookup-reading/+server.ts`
  at kiokun-data `abf71a89`) runs one statement over migration 0005's
  `dictionary_search`:
  - each word, pronunciation and common flag once (`DISTINCT`);
  - the query as an FTS5 string of the `pronunciation` column, its `"`
    doubled, and the pronunciation equal to it, Japanese rows only;
  - common first, then shorter (`LENGTH(word)`), at most `limit` (default 5,
    `Math.min(parseInt(limit), 20)`);
  - then the word equal to the trimmed query is dropped.
  - The builder's CSV holds one Japanese row per gloss, its pronunciation
    the entry's first kana alone (`src/search_index_builder.rs`), so another
    reading of an entry is not indexed.
- **The word page's Homophones are client-only** (`[word]/+page.svelte`, an
  `$effect`):
  - its readings are each Japanese word's kana, each once;
  - each is fetched, `?q=<reading>&limit=20`;
  - each answer keeps the words that are none of the page's own forms (the
    word, and each Japanese word's kanji and kana) and not the reading;
  - each reading with words left is added as a group **when its request
    answers**, so the groups' order is whichever request finished first;
  - the section, "Homophones", shows each group's reading only where there
    are several, and each word as a chip linked to its page, `common` where
    its row is.
- **The `/homophones/japanese` explorer** reads two static JSON files the
  app builds from JMdict and its pitch data (`extract_homophones.mjs`): a
  page of its own, the inventory's, not this step.
- **Deinflection** (`src/lib/utils/deinflect.ts`) is adapted from
  10ten-ja-reader, which is GPL-3.0 (its LICENSE, read 2026-10-11). The
  integrator's ruling and the owner's answer, 2026-10-11, are under Not
  claimed.

## Decision

1. **The reading statement is the layer's** (the search ADR's B2): the host
   operation `kiokun:data/search#reading(reading, limit)` runs kiokun.com's
   statement as written over the same index, and answers `ReadingHit {
   word, pronunciation, is_common }`, held by `KiokunSearch` and granted as
   `database.read<ReadingHit>`.
2. **The program keeps the rules**: the readings (each Japanese word's
   kana, each once, in order); the limit the page asks, 20; the words kept;
   the groups; the reading shown where there are several; the chip marked
   common where `is_common` is not 0. `query Word` asks each reading in its
   own body, as it asks the pitch shards, so the section is read with the
   page and rendered on the server.
3. **kiokun.com's two other drops are implied.** The page drops each of its
   own forms. kiokun.com also drops the reading, twice: the endpoint the word
   equal to the trimmed query, the page the word equal to the reading. The
   reading is one of the page's kana, so among its forms. And trimming
   changes none of kiokun's readings: none of the committed index's 427,860
   Japanese pronunciations. A mutant removing either check was equivalent,
   so neither is written.
4. **An empty reading is asked nothing**, as the endpoint answers it 400 and
   the page's fetch then adds nothing.
5. **The section** is kiokun.com's markup in the rewrite's form: `<section
   id="homophones">` with `<h2>Homophones</h2>`, after the Korean words and
   before the names, as kiokun.com places it; each group a
   `homophone-group`, its reading a `homophone-reading` where there are
   several, each word an `<a class="homophone-chip">` (`common` added) to
   `/word/{word}`.
6. **Held to kiokun.com's own code**: `just e14-kiokun-homophones` runs the
   endpoint, copied from HEAD, in Node over `node:sqlite`, with D1's
   stand-in (`d1.mjs`) and SvelteKit's stated stand-in (`kit.mjs`, shared now
   with the search oracle, as the index loading is in `index.mjs`), and
   transcribes the page's part, named, since an `$effect` is no module. Over
   every 256th entry of each subdirectory that kiokun.com's loader shows as
   it is (no stub, no merged character, as the Contains oracle takes them).
7. **CI holds a sample**: the index sample now also holds the Japanese rows
   read as one of the word sample's readings, 1,610 more rows of JMdict's
   (4,375 in all), which the Homophones test reads: the minimum a committed
   test reads, from a cleared source. The fixture `homophones-sample.json` is
   kiokun.com's answers over the two samples, 21 words.

## Differences from kiokun.com

Numbered after the search ADR's 30.

31. **The groups are in the readings' order**, where kiokun.com's are in the
    order their requests answer, which varies. The oracle compares each
    reading's words and not the groups' order (the integrator's ruling of
    2026-10-11).
32. **The section is rendered with the page**, and needs no script; on
    kiokun.com it appears after the page, when the requests answer, and
    not at all without script.
33. **The order of words kiokun.com's statement does not order** (two common
    words of one length) is SQLite's, here as there, and not compared: the
    oracle compares each reading's words as a multiset, and holds the
    rewrite's to common first, then shorter.

## Acceptance

- **The server's tests**:
  - `homophones_are_each_readings_words_but_the_pages_own`: 人's four
    readings in order, each shown; ひと's words with 一 first and common,
    and no 人; じん's 仁 first, common, the rest not; 色's 食 shown twice,
    common and not, as the endpoint's `DISTINCT` answers; 夢's one group
    shown without its reading; 諺 with no section; a reading two of a page's
    words share is one group (an entry made for it).
  - `homophones_match_kiokuns_answers_for_the_sample` (CI): the fixture's 21
    words.
  - `homophones_match_kiokuns_own_on_a_sample` (local, ignored).
  - `kiokun_is_served_by_the_host…`: the layer grants `ReadingHit`'s read.
- **The scripts' tests**: the index sample keeps a Japanese row read as a
  sample reading, and not a Chinese or Korean one; the sample's readings are
  its Japanese words' kana.
- **The oracle** (`just e14-kiokun-homophones`, local): every 256th entry
  of each subdirectory, 5,938 read, 1,082 with Japanese words, 1,318
  readings looked up: 0 unnamed differences over the whole index.
- **Mutation controls**: seven in `scripts/kiokun_word_mutations.py`, suite
  `homophones` (the own forms kept, a shared reading asked twice, one group's
  reading shown, every chip common, an empty group, a smaller limit, the
  statement's order), each killed, measured by `scripts/mutant_killers.py`.

## Not claimed

- **Deinflection** (`deinflect.ts`, its search-box resolution and the word
  page's 307 from an inflected form). kiokun.com's rules are adapted from
  10ten-ja-reader, GPL-3.0, and this repository is Apache-2.0 and MIT: the
  table, any part of it, or a transcription of it is not committed (the
  integrator's ruling of 2026-10-11). The owner chose, 2026-10-11, rules of
  our own from a cited, cleared grammar reference, held to kiokun.com's
  behaviour as an oracle: its own ADR, next.
- **`/api/lookup-reading` as an endpoint**: Pleris has no form for a route
  that answers JSON (the search ADR's Decision 3); the word page reads the
  statement directly.
- **The `/homophones/japanese` explorer**: a page of its own (the
  inventory's).
- **Alternate readings**: an entry's second kana is not in kiokun.com's
  index, so its homophones are not found there or here.
