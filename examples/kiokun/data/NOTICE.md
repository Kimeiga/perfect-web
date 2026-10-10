# The kiokun sample: sources and licences

`han-1char-3/` holds 28 dictionary entries for the words of kiokun.com's
shard `han-1char-3` that this repository's tests read. They were read from
kiokun-data's build output (`output_dictionary/`, made by kiokun-data's
builder, identical there to the files first copied here at 4a5cbae) and
written by `scripts/kiokun_sample.py` with **only the fields a committed test
reads, from the sources below** (the integrator's ruling of 2026-10-10).
`han-1char-3/MANIFEST.txt` lists each file with its SHA-256, each field kept,
and the source each field is made from. Each field's source is read from
kiokun-data's builder at its commit `abf71a89` (`src/main.rs`), not inferred
from the output.

**These data files, and only these, are distributed under the Creative
Commons Attribution-ShareAlike 4.0 International licence (CC BY-SA 4.0),
<https://creativecommons.org/licenses/by-sa/4.0/>**, as the sources' terms
allow: CC BY-SA 4.0 material as it is; CC BY-SA 2.0 KR and 3.0 material under
a later version with the same elements (BY-SA 2.0 §4(b), 3.0 §4(b)); CC BY 2.0
FR material within a BY-SA adaptation; the Unicode License's material with
its notice. The rest of the repository, the code reading these files
included, is under its own licence (`LICENSE`, Apache-2.0).

What was done to every source: extracted by kiokun-data's builder into
kiokun's entry format; then trimmed by `scripts/kiokun_sample.py` to the
fields named in the manifest, for 28 words.

## The search index's sample

`search-sample.csv` holds 2,765 rows of kiokun-data's search index CSV
(`output_search_index.csv`, committed there at a47956fb), written by
`scripts/kiokun_search_sample.py`: the rows whose word holds 人, 学 or 學 and
is no longer than two characters, read with `git show HEAD:…`. A Chinese row
is kept only where its definition is an item's that Dong Chinese's export
tags `cedict` (CC-CEDICT) or `unicode` (Unihan) for that traditional form; a
Japanese row is JMdict's. Its romanizations are the builder's. It is
distributed under CC BY-SA 4.0, as the entries are.

`search-aliases-sample.json` is not kiokun's: four script relations written
for the tests (学 and 學, and two Japanese forms' canonical pages), in the
shape of kiokun.com's `search-aliases.json`, which stays local.

## The sources

Each was read at its primary source on **2026-10-10**.

- **CC-CEDICT**: Chinese words, their pinyin and definitions
  (`chinese_words`, `contained_in_chinese[].w`).
  - Rights: the CC-CEDICT project, maintained at MDBG, continuing Paul
    Denisowski's CEDICT (1997).
  - Licence: CC BY-SA 4.0. Read at
    <https://www.mdbg.net/chinese/dictionary?page=cc-cedict>.
  - It reaches kiokun-data through Dong Chinese's export of its dictionary
    (`data/chinese_dictionary_word_2025-06-25.jsonl`); only the items that
    export tags `cedict` are kept. Dong Chinese's own content is not here.
- **CC-Canto readings**: the Jyutping of Chinese words
  (`chinese_words[].items[].jyutping`).
  - Rights: Pleco Software, which created CC-Canto and added Cantonese
    readings to CC-CEDICT.
  - Licence: CC BY-SA 3.0, "the same Creative Commons Attribution-ShareAlike
    3.0 license as CC-CEDICT", <https://creativecommons.org/licenses/by-sa/3.0/>.
    Read at <https://cantonese.org/about.html>.
  - Through MarvNC's Yomitan conversions (`data/cantonese/cc-cedict-canto`,
    `data/cantonese/cc-canto`).
- **Unihan** (the Unicode Character Database): a character's Cantonese
  reading (`chinese_char.cantonese`, kCantonese), its Hong Kong form
  (`chinese_char.hkChar`, kSemanticVariant with kHKGlyph), its Korean
  compatibility form (`korean_char.hanjaForm`, kCompatibilityVariant), and a
  Korean reading where KANJIDIC2 has none (`korean_char.readings`, kHangul).
  - Rights: © 1991-2026 Unicode, Inc.
  - Licence: the Unicode License v3, <https://www.unicode.org/license.txt>,
    which permits redistribution with its copyright and permission notice;
    the files' own terms point to <https://www.unicode.org/terms_of_use.html>.
    Read at both.
  - Files: `data/unihan/Unihan_Readings.txt`, `Unihan_Variants.txt`,
    `Unihan_IRGSources.txt` (Unicode 17.0.0).
- **JMdict**: Japanese words, their readings, senses and glosses
  (`japanese_words`, `contained_in_japanese`).
  - Rights: the Electronic Dictionary Research and Development Group (EDRDG).
  - Licence: CC BY-SA 4.0, "the property of the Electronic Dictionary
    Research and Development Group, and are used in conformance with the
    Group's licence". Read at <https://www.edrdg.org/edrdg/licence.html>.
    Project: <https://www.edrdg.org/wiki/index.php/JMdict-EDICT_Dictionary_Project>.
  - Through jmdict-simplified 3.6.1 (`data/jmdict-examples-eng-3.6.1.json`).
- **Tatoeba**: the example sentences JMdict's senses carry, each with its
  Tatoeba sentence id (`japanese_words[].sense[].examples`, `source.value`).
  - Rights: each sentence's author, credited at
    `https://tatoeba.org/sentences/show/<id>`.
  - Licence: CC BY 2.0 FR, Tatoeba's default for sentences. Read at
    <https://tatoeba.org/en/terms_of_use>. No audio is here.
- **KANJIDIC2**: a character's JLPT level, grade, frequency, stroke counts,
  on'yomi, kun'yomi and English meanings (`japanese_char`), and the English
  meanings of its Korean entry (`korean_char.meaningsEn`).
  - Rights: EDRDG. Licence: CC BY-SA 4.0, as JMdict. Project:
    <https://www.edrdg.org/wiki/index.php/KANJIDIC_Project>.
  - **Korean readings** (`korean_char.readings[].hangul`, where KANJIDIC2 has
    them): Charles Muller's, who keeps his copyright and permits their use
    under EDRDG's licence (its section 8).
  - None of KANJIDIC2's other contributed fields is here: no SKIP codes
    (Jack Halpern), Four Corner or Morohashi data (Urs App), De Roo codes,
    Spahn–Hadamitzky descriptors, or pinyin (Wittern and Yasuoka).
  - Through jmdict-simplified 3.6.1 (`data/kanjidic2-en-3.6.1.json`).
- **JMnedict**: Japanese names (`japanese_names`).
  - Rights: EDRDG. Licence: CC BY-SA 4.0. EDRDG's licence names ENAMDICT, the
    names file; JMnedict is its successor (the integrator's reading).
  - Through jmdict-simplified 3.6.1 (`data/jmnedict-all-3.6.1.json`).
- **KRDICT** (세계인이 누리는 한국어 학습사전): Korean words, their hanja, parts
  of speech and English definitions (`korean_words`, `contained_in_korean`).
  - Rights: the National Institute of Korean Language (국립국어원).
  - Licence: CC BY-SA 2.0 KR ("크리에이티브 커먼즈 저작자표시-동일조건변경허락
    2.0 대한민국"), <https://creativecommons.org/licenses/by-sa/2.0/kr/>, for
    its text unless stated otherwise; its media are licensed per file, and
    none is here. Read by the integrator on 2026-10-10 on KRDICT's Open API
    registration page; the export's own notice points to
    <https://krdict.korean.go.kr/kor/kboardPolicy/copyRightTermsInfo>.
  - Through Lyroxide's Yomitan export (`data/krdict-en`). Its Korean IPA
    (`data/krdict-ipa`), whose terms are not stated, is not here.
- **kiokun-data's builder**: each entry's structure: its `key`, the
  characters' own forms, and `redirect` and `simplified_form_of`, which name
  the entry a word's page is. The builder computes these two from the
  variant relations in Dong Chinese's export; they are kept as the entries'
  structure, the relations themselves not (the integrator to confirm). MIT,
  © Hakan Alpay (`kiokun-data/LICENSE`).

## Left out

Never committed here, whatever a test reads (the integrator's ruling of
2026-10-10): Dong Chinese's own content (its items tagged `dong-chinese`,
its character glosses, statistics, pinyin frequencies, Shuowen text,
comments and variant tables); Baxter–Sagart's Old Chinese reconstructions;
JPDB's frequency ranks; CHISE's IDS; images and scans; KANJIDIC2's codes and
dictionary references; the Korean IPA; the Gemini translations of Korean
examples; and the owner's mnemonics, which the owner rules on. A test that
reads one of them runs locally only, against a kiokun-data checkout.
