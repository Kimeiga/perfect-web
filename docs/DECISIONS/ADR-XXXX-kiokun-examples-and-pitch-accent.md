# ADR-XXXX: kiokun's examples and pitch accent

Status: proposed by track `kiokun` (W6, `track/kiokun`), the word page's
1d (step 1), its sections without long lists. Contains and Appears in wait
for the renderer's shared scopes (the integrator's order of 2026-10-09).
Builds on ADR-0286, ADR-0288, ADR-0289 and the page head's ADR (ADR-XXXX).
Date: 2026-10-09. Milestone: E14.

## Context

- **kiokun.com shows a sense's examples** (`SenseExampleList.svelte`): a
  Chinese sense's from its reading's `definitionExamples` record for the
  sense's text (`[word]/+page.svelte`'s `matchedExamples`); a Japanese
  sense's by `japaneseExamplesForSense` (`src/lib/japanese-examples.ts`); a
  Korean sense's and a Korean word's own. The first is shown; a chevron
  shows the rest.
- **kiokun.com shows a Japanese word's pitch accent** beside its first
  reading (`PitchAccent.svelte`), from its own data, `static/pitch/<hh>.json`
  (256 files, 4.1 MB), which the entries do not carry.
- **The oracle standard** (the integrator's ruling of 2026-10-09): every pure
  function of kiokun.com's the rewrite reimplements is held to kiokun.com's
  own code where it can run alone.

## Decision

1. **Examples, as `SenseExampleList` shows them**: those with a text; the
   label `Example` or `Examples`; the first shown and the rest behind a
   `<details>` disclosure that says how many ("Show 2 more examples for this
   definition"), which needs no script. A view, `SenseExamples`, renders
   them in each place:
   - a Chinese sense's: the first record whose `definition` is the sense's
     text, each example written in its simplified form, else its
     traditional, with its English;
   - a Japanese sense's: `japaneseExamplesForSense`, each example's Japanese
     sentence by `land` or `lang`, with its English, none without a Japanese
     text;
   - a Korean sense's, and a Korean word's own after its senses.
   The layer decodes `definitionExamples`, each sense's `examples`, and a
   Korean word's definitions with their examples beside their texts.
2. **Pitch accent** (`PitchAccent.svelte`):
   - a host op, `kiokun:data/pitch#of(shard, word)`, reads
     `KIOKUN_APP/static/pitch/<shard>.json` when a page first asks for the
     shard, and keeps it. A word's readings come back in the file's order: a
     serde map would sort them, and kiokun.com takes the first reading where
     the one asked for is not there (`Object.values`, whose order is the
     file's for keys that are not integers). A shard is two hexadecimal
     digits; a missing file, or no app, is no data.
   - In Pleris: the shard is `simpleHash` over the word's UTF-16 units,
     not the entries' rule over code points; the word is the one
     `WordEntry.svelte` speaks (its first written form, else its first
     reading) and the reading its first; the accent is the reading's where
     the data has it, else the first reading's, and none where it is `null`;
     the morae are its characters, a small ゃゅょャュョ joined to the one
     before; each high or low by the accent (`renderPattern`); the pattern
     平板, 頭高, 中高 or 尾高 (`pitchType`, `countMorae`), and the title
     `<pattern> (accent: <n>)`.
   - `source KiokunEntries` holds `PitchReading`, and the layer grants
     reading it.
3. **Held to kiokun.com's own code where it runs alone**:
   `just e14-kiokun-examples` (LOCAL_ONLY) runs `japaneseExamplesForSense`,
   copied from `KIOKUN_APP` into a temporary directory, over every 256th
   entry its loader shows as it is, and compares each word's Japanese
   section; it writes the CI fixture for the repository's sample, which a CI
   test holds. The Chinese and Korean examples and the pitch rules live
   inside Svelte components, which cannot run alone; hand-written cases hold
   them.

## Found

1. **Two hashes over a word.** The entries' files are placed by a hash over
   code points (kiokun-data `src/main.rs`); the pitch shards by one over
   UTF-16 units (`PitchAccent.svelte`'s `charCodeAt`). For a word past
   U+FFFF the two differ, and a test holds the pitch rule to the shard its
   own hash names, with the other shard's data as the control. This is
   kiokun.com's own inconsistency, held as it is: the rewrite reads its
   data where kiokun.com wrote it. The integrator passes it to the owner
   (2026-10-09), with the label table's.
2. **The oracle found nothing to name** for the Japanese examples: 1,082
   sampled words, the same.

## Differences from kiokun.com

Added to ADR-0286's list:

15. **An example is not a link** to kiokun.com's sentence reader, `/sentence`,
    which the rewrite does not serve yet.
16. **An example's text has no readings over it**: kiokun.com writes a
    Chinese example's pinyin as ruby by its word breaks, a Japanese one's
    kana by its own analysis (`/api/sentence/ruby`), and glosses each Korean
    word (`/api/korean-glosses`). Each needs segmentation or a server
    analysis that step 2 brings.
17. **The rest of the examples are a `<details>` disclosure**, where
    kiokun.com animates a chevron; the content and its count are the same.
18. **Pitch is rendered with the page**, where kiokun.com loads its shard when
    the reading scrolls near (`IntersectionObserver`).
19. **Not yet**: the Chinese and Korean sentence corpora kiokun.com loads
    where a word has no examples (`ChineseSentenceExamples`,
    `KoreanSentenceExamples`), a Japanese word's homophones link and its
    conjugation table.

## Alternatives

- **Pitch data committed as a fixture.** Refused: the owner's data stays in
  kiokun-data (Q3); the tests write their own shards.
- **The pitch rules in the host.** Refused: they are the page's.
- **Compile kiokun.com's Svelte components to run them as oracles.** Not
  taken: it would need the app's build, which writes into the checkout, or a
  clone; the clone comparison (Q5) is where whole pages are compared.

## Acceptance

- **The server's tests, 28** (and three ignored, local): the earlier 23 and:
  - a sense's examples shown first and the rest disclosed: a Chinese
    sense's by its text, in its simplified form, a record for another sense
    not shown, one without a text left out; a Korean sense's and word's;
  - a Japanese sense's examples, by `lang` and by `land`, one without a
    Japanese text left out;
  - the Japanese examples against kiokun.com's answers for the repository's
    sample (the CI fixture);
  - pitch accent: 尾高, 中高, 頭高 with a small kana, 平板, the first reading
    in the file's order where the word's is not there, nothing for `null`
    or for a word not in the data;
  - a pitch shard is the hash over UTF-16 units.
- **The oracle** (`just e14-kiokun-examples`, local): 1,082 words, no named
  difference, 0 unnamed (`docs/evidence/E14/kiokun-examples.txt`).
- **The browser**: the spec reads 人's first Japanese example, script on and
  off.
- **Mutation controls**: 64 in `scripts/kiokun_word_mutations.py`, 11 of them
  this milestone's (7 pitch, 4 examples); the Korean placeholder's
  re-anchored on the senses.

## Not claimed

- **Pitch on the whole dictionary against kiokun.com**: the rules are in a
  component; the hand cases hold them, and the clone comparison will hold
  the page.
- **The examples' readings and links, the corpora, homophones and the
  conjugation table** (Differences).
