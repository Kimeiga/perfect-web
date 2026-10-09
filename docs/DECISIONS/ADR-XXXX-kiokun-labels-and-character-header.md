# ADR-XXXX: kiokun's labels for JMdict's codes, and the character header

Status: proposed by track `kiokun` (W6, `track/kiokun`), the second
milestone of step 1, the word page (docs/PARALLEL.md, "W6's inventory,
answered"). Builds on the word page's first milestone (ADR-0286, kiokun's
word page on the development server). Date: 2026-10-09. Milestone: E14.

## Context

- **The first milestone showed JMdict's codes** (`n`, `ctr`, `rK`) where
  kiokun.com shows its table's labels, and no character header.
- **kiokun.com's labels are its app's data**:
  `sveltekit-app/src/lib/japanese-labels.json`, read by `japaneseLabels.ts`.
  The integrator ruled that the app's own data is read through `KIOKUN_APP`,
  read-only, and never copied here (Q3).
- **kiokun.com's table and its entries disagree.** 53 of the table's 266 flat
  keys use `_` (`adj_na`), none uses `-`, and the entries carry JMdict's `-`
  (`adj-na`), so kiokun.com shows `adj-na`. The integrator ruled on
  2026-10-09 that the rewrite shows the label the table means (the word page
  ADR's Differences, 4; ADR-0286).
- **The character header** is `[word]/+page.svelte`'s block under "Character
  Header": the written forms, the learner gloss with its levels, the
  mnemonic keyword's line, a taxonomy breadcrumb, sense highlights, the
  actions, and the readings.

## Decision

1. **The labels are read by the layer and looked up in Pleris.**
   - `kiokun:data/labels#japanese` answers the table's `labels` (`pos`,
     `misc`, `field`, `dial`, `head_info`) as `Label { kind, code, text }`,
     read once when the server starts, from `KIOKUN_APP`. An app named
     without its table is refused at start. With no app named, there is no
     table, and every code is shown as it is, as kiokun.com shows a code its
     table lacks.
   - `label` reads the code as written, else the code with each `-` read as
     `_`, else shows the code. A sense's part of speech, field, misc and
     dialect are each read from their own table (`Sense.svelte`).
   - A form's note reads `getHeadInfoLabel`'s map (`iK` → `ikanji`, `rK` →
     `rkanji`, the search-only `sK` and `sk` as irregular) and then the
     `head_info` table.
   - `source KiokunEntries` now holds `Label` too, and the layer grants
     `database.read<Label>`.
2. **The character header, where the entry has a Chinese or a Japanese
   character** (`chinese_char` or `japanese_char`):
   - **The learner gloss** (`learnerGlossForEntry`): the mnemonic's keyword,
     else its meaning, else the character's gloss; then
     `normalizeLearnerGloss`: `(trad/jp)`, `(trad)`, `(simp)` and `(jp)`
     removed in that order with the space before each, ignoring ASCII case,
     and the rest trimmed. Pleris has no regular expressions, so `without`
     finds each marker by position.
   - **The levels beside it**: `HSK n` and `Nn`, each where the level is
     not 0, shown only with a gloss, as kiokun.com nests them.
   - **The mnemonic's dictionary meanings** ("Mnemonic keyword · Actual
     meanings: ..."), where the card has both a keyword and them.
   - **The readings** (`buildHeaderReadings`): Mandarin, the character's
     readings by frequency that its words read, or all of them where its
     words read none, else its readings in older sources, else its words'
     readings; Cantonese; on'yomi and kun'yomi from KANJIDIC's groups, or
     its older flat list where the file has no groups; the hanja table's
     Korean readings. Each once and in order, joined by ", " (Japanese by
     `、`), each where it has a reading, separated by `·`.
   - The layer decodes `chinese_char`, `japanese_char`, `korean_char` and
     `semantic_mnemonic` into the fields the header reads. A record field
     the world names in kebab case (`chinese-char`) is named so by the host.
3. **A Chinese word whose forms are empty is shown as the word looked up**
   (`trad || simp || word`), as kiokun.com shows it.

## Found

1. **A host record's field is named as the world names it.** The layer named
   a field `chinese_char` and the query trapped: "the host's record has no
   field `chinese-char`". WIT names are kebab case, and the engine looks a
   field up by its WIT name. The layer now writes `chinese-char`. Single-word
   fields never showed it.
2. **A word's page is in the shared cache once read** (`cache shared`, `key
   word`), so a test that swapped the layer under a running server read the
   old page. Each test that changes the data uses a server of its own.
3. **A page's document carries every block's template for the browser**,
   shown or not, so a test that a block is absent reads the markup before
   the scripts.

## Differences from kiokun.com

Added to the word page's list (ADR-0286):

5. **A label the table means is shown** where kiokun.com shows the raw code
   (`adj-na`): the integrator's ruling of 2026-10-09.
6. **The header has no written-form specimens, taxonomy breadcrumb, sense
   highlights or actions yet.** The specimens need the related forms the
   loader reads (`loadRelatedCharacterForms`); the breadcrumb and highlights
   need the character-support data (`static/game_data/`, through
   `KIOKUN_APP`); the actions are script and accounts. Each is a later
   milestone or step.

## Alternatives

- **Map codes to labels in the host.** Refused: the lookup is a rule of the
  page, and the ruled fallback (`-` read as `_`) is Pleris's to state.
- **Commit the label table as a fixture.** Refused: it is the owner's app's
  file (Q3). The tests write a small table of their own.
- **Show the extras the slice showed** (strokes, grade, frequency). Still
  hidden until the owner rules (Q8).

## Acceptance

- **The server's tests, 15** (`server/src/tests/kiokun.rs`), the first
  milestone's 11 and:
  - a code is shown by kiokun.com's label for it, from a table a test
    writes: a part of speech, `adj-na` read as `adj_na`, a code in neither
    spelling as it is, a form's note `(rare)`; with no app, the code itself;
  - an app without its label table is refused;
  - the header for 人 and 樂: gloss, levels and each reading;
  - the header's rules, each with its control: no header without a
    character; the keyword before the meaning; the dictionary meanings with
    a keyword and not without; Mandarin from older sources and from the
    words; a level of 0 not shown; the markers removed, ignoring case; a
    reading once; KANJIDIC's flat list; a word with empty forms.
- **The browser, in three engines**: the spec reads the header (gloss,
  levels, Mandarin and Cantonese) with script on and off.
- **Mutation controls**: `scripts/kiokun_word_mutations.py`, 33 mutants,
  18 of them this milestone's.
- `just e14-kiokun-word` records all three.

## Not claimed

- **The labels on the whole dictionary**: the tests write their table; the
  owner's is read only where `KIOKUN_APP` is set, locally.
- **The header's specimens, breadcrumb, highlights and actions** (above).
- **Exact text parity**, until the clone is compared (Q5).
