# ADR-XXXX: kiokun's word page shows Contains and Appears in

Status: accepted under the owner's delegation of 2026-10-02, on the
integrator's rulings for track `kiokun` (docs/PARALLEL.md). Date:
2026-10-10. Milestone: E14, kiokun.com in Pleris, step 1 (the word page).

## Context

- **kiokun.com shows two long lists below a word's words**
  (`[word]/+page.svelte:916-936`):
  - **Contains** (`Contains.svelte`): for a word of several characters,
    the words its builder found inside it (`contains`), each a card with
    its readings and a definition. The page sorts them by where they sit in
    the word (`sortContainsWords`), and `orderContainsWords`
    (`contains-order.ts`) breaks the word down breadth first: the best way
    to write it in the previews, never the whole word in one, by fewest
    pieces, then the better words, then pieces nearer one length, then the
    earlier preview; each piece's own breakdown next; the rest after,
    longest first. Where the word has several forms of one length
    (学生会/學生會), the one-character previews at one place are one card
    (学/學). A card's definition is the first of its characters' component
    glosses, its source marker taken off, else the preview's.
  - **Appears in** (`AppearsIn.svelte`): the Chinese, Japanese and Korean
    words that contain this one (`contained_in_*`), each column ranked by
    `rankAppearsIn` (`appears-in-order.ts`): a word read and defined as
    another shown once; Chinese words before names, Japanese common words
    first, each by frequency rank; Korean as the file lists them.
  - Each list shows ten, and ten more as the reader scrolls near its end.
    Both are loaded in the browser when the page scrolls near them, not
    rendered with the page.
- **The builder caps each Appears in list at 200** (kiokun-data
  `src/simple_output_types.rs`), so a common character's page has up to 600
  of them.
- **The rewrite could not render a long list in its length until
  ADR-0294**: pw-render's scopes were cloned per item. W6 found it in the
  timing sample (さえこ, 128 names, near 218 ms); the integrator fixed it on
  track/shared-scopes. The sample re-timed after it: p50 1.1 ms, p99 3.7 ms
  over 5,938 words (`docs/evidence/E14/kiokun-sample.txt`).
- **The oracle standard** (the integrator's ruling of 2026-10-09): kiokun's
  own code, run locally over a stated sample, every difference named, and a
  committed fixture for CI.

## Decision

1. **The layer reads each entry's four lists** as `Preview` records (`w`,
   `p`, `ct`, `jp`, `jo`, `kr`, `d`, `c`, `fr`), each field the builder
   leaves out read as empty, `false` or `None`.
2. **The rules are ported as kiokun.com writes them**, in `app.pw`:
   `sortContainsWords`, `buildContainsWordForms`, `orderContainsWords` (its
   memoized breakdown kept as a table filled from the range's end back, each
   cursor's best found once, as kiokun.com's memo finds it), `buildContainsCards`,
   `displayDefinition` and `cleanGloss`; `dedupeAppearsIn` and
   `rankAppearsIn`. An equivalent form's lists are merged as kiokun.com's
   loader merges them (`+page.ts:198-217`).
3. **The component glosses are read for Contains' previews too**, as
   `collectSupportKeys` adds each preview's form (`word-character-learning.ts`).
4. **The sections follow Japanese Names**, as on kiokun.com: the first ten
   of each list, and the rest behind a `<details>` disclosure saying how
   many there are ("N more", "N more items").
5. **Both are held to kiokun.com's own code** (`just e14-kiokun-contains`,
   local). The harness imports `contains-order.ts`, `appears-in-order.ts`
   and `character-support.ts` as modules. The page's own functions are not
   modules: `sortContainsWords` and `buildContainsWordForms` in the page's
   script, the cards' in `Contains.svelte`'s, the support characters' in
   `word-character-learning.ts`. Each is taken from kiokun.com's source as
   written, by its name, into a module of its own; a name found other than
   once, or a module importing what the harness does not copy, stops the
   run. Each word's glosses are what kiokun.com's loader would ask for:
   `collectSupportKeys` over the entry, then `buildCharacterSupportData`
   over the app's own game data.
6. **The CI fixture keeps each list's length and its first 20 and last 5
   items, each with its place** (the integrator's ruling of 2026-10-10,
   recorded in the fixture's `trimmed` field): the head, where every
   ordering rule shows, and the tail, where unranked words and names go,
   without the whole lists' dictionary text in the repository (614 KB
   whole, 113 KB trimmed). The whole lists are held locally, over all the
   sampled words, and the rules the sample does not show by the hand
   cases.

## Differences from kiokun.com

Numbered after ADR-0301's 20.

21. **The lists are rendered with the page**, where kiokun.com loads each
    component as the reader scrolls near it. The page is longer: 人's is
    466 KB with its 543 list items, 71 KB without them.
22. **The rest of each list is a `<details>` disclosure**, where kiokun.com
    shows ten more each time the reader scrolls near the list's end. The
    disclosure needs no script; it costs a long list's page 16 to 20 ms
    and about 390 KB (人 and 色, Found 3). Reading the rest on demand, when the
    disclosure opens, is a later step.
23. **The reader's language preferences are not read**: every language's
    column and every card's readings are shown, as kiokun.com shows them
    to a reader who has set none (`stores/languages.svelte.ts`: Chinese,
    Japanese, Korean and Cantonese, all on). A reader who turns one off
    there sees less.
24. **The page acknowledges EDRDG**, at its foot, wherever it shows JMdict's,
    JMnedict's or KANJIDIC2's data, as the Group's licence asks of a web
    dictionary "on each screen display"
    (<https://www.edrdg.org/edrdg/licence.html>, read 2026-10-10).
    kiokun.com's pages at HEAD carry no such acknowledgement: the owner's to
    add.

## Found

1. **The oracle found nothing to name**: 5,513 sampled words, 5,081 of
   them with Contains and 53 with Appears in, the same.
2. **A test reader's error, not the server's.** On a machine at load 27,
   the oracle reported the rewrite's text for 煮's Japanese words as
   "に��める" where kiokun.com has "にしめる". The shared test helper
   `fetched_as` decodes each TCP read on its own, so a character a read
   boundary splits comes out as U+FFFD; where boundaries fall depends on
   timing. The kiokun tests read through their own `fetched_whole`, which
   decodes the whole response once and refuses invalid UTF-8. The
   integrator took the shared helper's fix (2026-10-10).
3. **What the lists cost**, each request of a server just started, since a
   word's page is kept in the shared cache once read
   (`docs/evidence/E14/kiokun-contains.txt`, recorded at a one-minute load
   of 34 falling to 9 on 12 cores):

   | | the page | its lists left empty |
   |---|---|---|
   | 人 (Chinese 143, Japanese 200, Korean 200) | p50 46.7 ms, 466 KB | p50 29.3 ms, 71 KB |
   | 色 (142, 200, 200) | p50 49.1 ms, 450 KB | p50 29.4 ms, 69 KB |
   | 陸上貨物運送事業労働災害防止協会 (26 cards) | p50 3.3 ms, 57 KB | p50 1.7 ms, 26 KB |

   An earlier run at a load of 4.8 to 7.7 measured 43.6 and 26.3 ms for
   人, and 43.4 and 27.6 ms for 色. A page whose lists are long spends 16
   to 20 ms and about 390 KB on them. Reading the disclosed rest on demand
   is a later step (Difference 22); the oracle and the tests hold the
   order whichever way it is sent.
4. **The repository's sample has no Contains**: its words are single
   characters. CI holds Contains by hand cases answered by kiokun.com's own
   code (`contains.mjs` run on the same entries), and Appears in by the
   fixture.

## Acceptance

- **The server's tests**:
  - `contains_breaks_the_word_down_and_cards_its_forms`: 学生会's previews
    broken down (学生 + 会, then 学 and 生), carded (会/會, 学/學), the gloss
    found on a card's second form ("assemble (TRAD)", 會's), an on'yomi the
    same as the reading not shown, each card a link; and 甲乙丙, where
    nothing breaks the form down and the word's own order decides a tie;
  - `appears_in_ranks_each_languages_words_ten_at_a_time`: names last, a
    rank of 0 none, a zero-width space not a letter, a word read and
    defined as another once, one without a form none, Japanese common words
    first, Korean as listed, ten shown and two disclosed;
  - `an_equivalent_forms_words_are_shown_on_the_canonical_page`: 后's lists
    merged into 後's, 後面 once, 余's not;
  - `contains_and_appears_in_match_kiokuns_answers_for_the_sample` (CI):
    the fixture's 11 words, each list trimmed (Decision 6);
  - `contains_and_appears_in_match_kiokuns_own_on_a_sample` and
    `word_pages_with_long_lists_are_timed` (local, ignored).
- **The oracle** (`just e14-kiokun-contains`, local): 5,513 words, 0 named,
  0 unnamed differences.
- **Mutation controls**: 24 more in `scripts/kiokun_word_mutations.py`, each
  undoing one rule above, in the program or the layer. The one that
  survived a first check by hand ("Contains keeps the file's order") is
  killed by 甲乙丙. The whole script is recorded by the branch's verify run
  (ADR-0281), as the integrator asked.

## Questions asked

- **The fixture's licence** (W6, 2026-10-10): the fixtures carry dictionary
  text derived from the repository's sample, whose notice names its own
  files alone. The integrator: add `spikes/own-renderer/kiokun-oracle/NOTICE.md`,
  saying each `*-sample.json` there holds text derived from kiokun.com's
  data for the sample's words, under the sample's licences and
  attributions, pointing to `examples/kiokun/data/NOTICE.md`; it covers the
  head's and the examples' fixtures too, which had none. And trim this
  fixture (Decision 6).

## Amendment to ADR-0285: kiokun-data is read at HEAD

**The inventory and the oracles read kiokun-data's HEAD** (the
integrator's ruling, 2026-10-10).

- **Why**: the evidence names a commit ("kiokun.com's source: <commit>"),
  which is true, and the run reproducible, only if what was read is that
  commit. The owner works in the checkout. While this milestone was built
  it held uncommitted changes to `[word]/+page.svelte` and untracked
  character-lesson and character-narrative routes. The inventory guard
  read the working tree and failed on the new narrative route. The
  Contains oracle copied the page from the tree; its 19 new lines did not
  touch the functions taken, but the evidence called the commit clean.
- **What changed**:
  - `scripts/kiokun_inventory.py` reads the routes with
    `git ls-tree -r --name-only HEAD -- src/routes`: the commit, not the
    index (`ls-files` would read files staged and not committed) and not
    the tree.
  - `scripts/kiokun_app_source.py` copies each file an oracle runs with
    `git show HEAD:./PATH`. It prints the commit and each file's digest as
    committed.
  - Each names the owner's uncommitted work under its paths as "not
    read"; that is information and fails nothing. The data files git does
    not track (`output_dictionary`) are out of the rule's scope and are
    recorded as before.
  - The four oracle recipes (head, examples, moves, Contains) copy through
    it, the Contains oracle's game data too.
  - Tests: a route staged or untracked is not a route and is named; a
    copy is the commit's whatever the tree holds; a file not committed is
    refused. Five mutants in `scripts/kiokun_inventory_mutations.py`
    (14 of 14 killed).
- **What it changed in ADR-0285**: its table had been made from the
  working tree. `/api/character-lesson/[character]` and the
  `CharacterMiniLesson.svelte` feature were the owner's uncommitted work
  then and are still, so they come out until the owner commits them. At
  `abf71a89`: routes 101 (49 pages, 52 endpoints), 0 built, 4 partial, 97
  missing; features 62, 4 built, 11 partial, 47 missing.
- **What it does not change**: the rewrite's server still reads
  `KIOKUN_APP`'s label table, component glosses and pitch data from the
  tree, as a deployment reads its files. The oracles read the committed
  ones, so a difference between the two would show as a difference.

## The repository's kiokun data, cleared

**The repository is public, and committed data is the minimum a committed
test reads, from cleared sources only** (the integrator's rulings of
2026-10-10). A source is cleared when its terms were read at the source and
permit its redistribution here.

- **Provenance from the builder's source at kiokun-data HEAD (`abf71a89`)**,
  never from its output. The sample, a byte-for-byte copy since 4a5cbae,
  held much more than tests read:
  - Dong Chinese's own content: its character glosses, statistics, pinyin
    frequencies, Shuowen text, comments and variant tables;
  - Baxter–Sagart's reconstructions, CHISE's IDS, images and scans;
  - KANJIDIC2's codes and references (SKIP, Four Corner, De Roo,
    Spahn–Hadamitzky, Morohashi) and its pinyin and Korean readings;
  - JPDB's ranks and the owner's AI-authored mnemonics.
- **The sample is regenerated** by `scripts/kiokun_sample.py`, which reads
  kiokun-data's output and writes each entry with only the fields its
  `FIELDS` names, each with its source. The output's 28 files were identical
  to the committed ones, so nothing but fields left out changed.
  - It keeps CC-CEDICT's words (only the items Dong Chinese's export tags
    `cedict`), CC-Canto's Jyutping, Unihan's Cantonese, Hong Kong and Korean
    forms, JMdict with its Tatoeba examples and their ids, KANJIDIC2's
    levels, readings and meanings, JMnedict and KRDICT.
  - Chinese previews keep their form alone: their readings and definitions
    cannot be traced to one source per item.
  - `MANIFEST.txt` lists each file, field and source, and `NOTICE.md` each
    source's rights holder, licence, URL and the date it was read.
  - The files are distributed under CC BY-SA 4.0.
  - ADR-0037's "copied byte for byte" no longer holds.
- **Tests that read a field left out moved onto entries made for them**:
  - the character header's gloss, HSK and JLPT levels and frequency-list
    Mandarin (the JLPT badge sits beside the gloss, as on kiokun.com);
  - the written forms and a stub's forms (Dong Chinese's variants);
  - the page head's gloss-led title.
  - The hand-made entries hold kiokun's deciding fields for each word. The
    mutant "Mandarin keeps the readings the words do not read" is killed
    there.
  - The browser spec reads the sample's cleared fields alone: no gloss or
    level badges, and the title from the definitions.
- **The fixtures** hold only the fields their tests compare, each fixture's
  `provenance` naming each field's source.
  - `examples-sample.json` carries each sentence's Tatoeba id, and its test
    requires one per example.
  - Fields from uncleared sources are held locally by the same recipes.
- **EDRDG's acknowledgement**: the word page acknowledges JMdict, JMnedict
  and KANJIDIC2 at its foot wherever it shows their data, in the licence's
  sample wording, linked (Difference 24).

## Not claimed

- **The reader's language preferences** (Difference 23).
- **Homophones, the notes, and the other sections kiokun.com loads beside
  these**: the inventory's (ADR-0285).
