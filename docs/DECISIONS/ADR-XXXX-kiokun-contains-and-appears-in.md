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
    disclosure needs no script; it costs a long list's page about 16 ms and
    390 KB (人 and 色, Found 3). Reading the rest on demand, when the
    disclosure opens, is a later step.
23. **The reader's language preferences are not read**: every language's
    column and every card's readings are shown, as kiokun.com shows them
    to a reader who has set none (`stores/languages.svelte.ts`: Chinese,
    Japanese, Korean and Cantonese, all on). A reader who turns one off
    there sees less.

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
   word's page is kept in the shared cache once read (one-minute load 4.8
   to 7.7 on 12 cores, `docs/evidence/E14/kiokun-contains.txt`):

   | | the page | its lists left empty |
   |---|---|---|
   | 人 (Chinese 143, Japanese 200, Korean 200) | p50 43.6 ms, 466 KB | p50 26.3 ms, 71 KB |
   | 色 (142, 200, 200) | p50 43.4 ms, 450 KB | p50 27.6 ms, 69 KB |
   | 陸上貨物運送事業労働災害防止協会 (26 cards) | p50 3.1 ms, 57 KB | p50 1.6 ms, 26 KB |

   A page whose lists are long spends about 16 ms and 390 KB on them; a
   run an hour earlier, at a higher load, measured the same within 2 ms.
   Reading the disclosed rest on demand is a later step (Difference 22);
   the oracle and the tests hold the order whichever way it is sent.
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

## Not claimed

- **The reader's language preferences** (Difference 23).
- **Homophones, the notes, and the other sections kiokun.com loads beside
  these**: the inventory's (ADR-0285).
