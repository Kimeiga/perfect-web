# ADR-XXXX: kiokun's deinflection, our own rules

Status: proposed (track `kiokun`, W6), under the owner's delegation of
2026-10-02 and the owner's answer of 2026-10-11. Date: 2026-10-11.
Milestone: E14.

## Context

- **kiokun.com reads an inflected Japanese word back to its dictionary
  form.**
  - On its word page: a word with no file (`[word]/+page.ts:340-379`) runs
    `findWordsWithDeinflection` (`src/lib/utils/search-navigation.ts:185-340`)
    and answers `307` to `/<form>?from=<word>&conj=<how>&alt=<others>`.
  - The target page shows "from → word (how)" and the others as chips
    (`+page.svelte:427-460`).
  - The search box does the same in the browser (`navigateOrSearch`), for
    CJK input.
- **Its rules are adapted from GPL-3.0 code.**
  - `src/lib/utils/deinflect.ts` begins "Adapted from 10ten-ja-reader".
  - 10ten-ja-reader's LICENSE is GPL-3.0 (read at its source, 2026-10-11).
  - This repository is Apache-2.0 and MIT. The integrator ruled that the
    table, any part of it, and any transcription of it are not committed.
  - The owner chose, 2026-10-11, rules of our own: written from a cited,
    cleared grammar reference, in our own structure, reason labels and
    algorithm, with kiokun.com's behaviour as an oracle and each
    disagreement named.
- **The resolution around the rules is kiokun.com's own code** (MIT, the
  owner's), read for this ADR:
  - each candidate is tried, fewest reasons first;
  - its entry is fetched, following redirects (up to three);
  - its entry's Japanese parts of speech are checked against the
    candidate's class (`validatePosMatch`);
  - a hiragana candidate whose entry fails is retried through the reading
    index (`/api/lookup-reading?limit=10`), each written form checked the
    same way;
  - at most five forms are kept.

## Decision

1. **The facts come from three cited pages**, each CC BY-SA 4.0 (the
   integrator's rulings of 2026-10-11):
   - English Wikipedia, "Japanese conjugation", revision **1379736007**
     (2026-10-11T17:16:29Z): the verb groups, the bases (mizenkei,
     ren'yōkei, kateikei, meireikei, ishikei, onbinkei), and the bases the
     negative, passive, causative, desiderative, past, gerund, conditional
     and hortative follow; する's and 来る's bases.
   - Wiktionary, "Appendix:Japanese verbs", revision **92311215**
     (2026-08-25):
     - the onbin of each godan ending, and the exceptions 行く (いっ) and
       問う/請う (うた, うて);
     - ます and たい on the continuative; たら on the onbin;
     - the potential (godan e-row + る, one-row られる);
     - する's さ and せ; 来る's こ and き;
     - the causative's contracted causative-passive (読ませられる → 読まされる);
     - 〜そう on the continuative ("evidential");
     - the continuative used alone;
     - and the hybrid する of 愛する (愛さない, 愛そう, 愛せ, 愛せば).
   - Wiktionary, "Appendix:Japanese adjectives", revision **91406689**
     (2026-06-28): an i-adjective's く, かった, くて, ければ, かったら, さ, and
     良い's よく.
   - **Facts only**: a conjugation fact is the language's. The arrangement,
     names, reason labels and algorithm below are the rewrite's, and no
     table or prose is copied from either site or from 10ten.
2. **The structure** (`examples/kiokun-site/app.pw`, "Deinflection"):
   - **A word class** has a code (JMdict's part of speech), a group (what an
     entry's part of speech is checked against), its dictionary ending, and
     its bases. A base is a named text that takes the place of the ending.
     - The godan classes come from one function of the ending's row: a-row
       for the irrealis and the voices, i-row for the continuative, e-row for
       the hypothetical and the imperative, o-row and う for the volitional,
       and the onbin te and ta.
     - 行く (v5k-s) and 問う (v5u-s) differ only in te and ta.
     - The one-row verbs and 来る (in kana and in kanji) come from one
       function of a stem.
     - する after a noun (vs) has an empty ending, so its candidate is the
       noun. する itself is vs-i, and する after one kanji (vs-s) adds the
       sa-row bases.
     - An i-adjective's bases come from its stem. いい's stem is よ.
   - **A closing** closes a word on one of its bases:
     - for verbs: past, te form, たら, たり, ば, volitional, imperative, ず,
       〜そう, and a one-row verb's bare stem;
     - for adjectives: past, te form, たら, ば, the adverb in く, さ, 〜そう;
     - for ます's forms: ました, ません, ませんでした, ましょう, まして, ましたら.
   - **An auxiliary** follows a base and makes a word of its own class:
     - ない (an adjective);
     - ます (its own class);
     - たい (an adjective);
     - the potential, the passive and the causative (one-row verbs);
     - the contracted causative-passive される;
     - ている and てる (one-row verbs);
     - てしまう and ちゃう (godan う verbs);
     - すぎる.
3. **The algorithm** (`deinflected`):
   - Each closing that a word ends with gives a candidate. So does each
     class an auxiliary makes, the word being its dictionary form.
   - Then, for up to six rounds, each candidate whose class an auxiliary
     makes is read back through that auxiliary to the word it followed.
   - A candidate is a word, its class, and its reasons, innermost first. A
     word and class found once is not found again.
   - A katakana word is read in hiragana first, then as written.
   - A bare stem is read back no further.
   - The candidates of a dictionary class with at least one reason are kept,
     fewest reasons first.
   - Where two take as many steps:
     - a bare stem comes before the forms it can also be;
     - the potential comes before the voices;
     - the one-row voices come before the godan and する voices.
     That is kiokun.com's order where the oracle found two.
4. **The resolution** is kiokun.com's, over the host's reads (`resolved`):
   - the candidates' files are read in one batch, each redirect followed one
     hop;
   - each candidate is kept where its entry's parts of speech include its
     group;
   - a hiragana candidate whose entry does not fit is looked up by reading
     (`search#reading`, limit 10, the reading itself left out, as the
     endpoint answers), and each written form's entry is checked;
   - at most five forms.
5. **The page answers 200 with the first form's entry, at the address
   asked** (the integrator's ruling (b) of 2026-10-11):
   - the note "asked → form (how)" at the top of the page;
   - the others linked to their pages ("Could also be:");
   - the address typed kept, with script on or off. No address is replaced
     by script, so a reload keeps the note.
   - The page's head is the form's: its title and description.
   - Why not kiokun.com's 307:
     - A deinflection is a guess among candidates, not a move. RFC 9110
       §15.4.8: a 307 means the resource "resides temporarily under a
       different URI", and no resource has moved.
     - A redirect costs a round trip before the first byte.
     - The reader keeps the address they typed, and a shared link carries
       their question.
     - This is MediaWiki's answer: `/wiki/UK` answers 200 with United
       Kingdom's article, a "(Redirected from UK)" note, and a canonical
       link to `/wiki/United_Kingdom`.
   - **On RFC 6596 §3**, which asks that a canonical's target hold the
     referring page's content: the entry is the same, and the note and the
     alternatives are that address's own navigation, as on MediaWiki's page.
6. **Held to kiokun.com as a black box**: `just e14-kiokun-deinflection`
   runs `findWordsWithDeinflection` in Node.
   - It is taken by name from `search-navigation.ts` with the functions it
     calls, and imports `deinflect.ts` and `korean-deinflect.ts` as they are.
   - They are copied from HEAD, run with `--experimental-transform-types`
     (they use enums), **never read, never committed**.
   - Its two reaches outside get stand-ins: entry files are named by
     kiokun's shard rule and answered from the data, and the reading index
     is kiokun.com's own endpoint.
   - The inputs are the rewrite's own conjugator's forms (`inflect.mjs`)
     of every 128th entry's word, from the same cited facts. Only forms
     with no file of their own are asked, since kiokun.com deinflects only
     on a 404.
   - The comparison: the same first form, and the same forms in all, in any
     order.
7. **CI holds a sample**: `examples/kiokun/data/deinflection/`, 27 entries
   with their key and JMdict's Japanese words alone (from cleared sources).
   The fixture `deinflection-sample.json` holds kiokun.com's answers for
   501 forms of their words, over that sample and the index sample.

## Differences from kiokun.com

Numbered after the reading index ADR's 33.

34. **An inflected address answers 200, not 307** (Decision 5). kiokun.com's
    `/食べる?from=…&conj=…&alt=…` addresses, copied from its address bar, show
    the entry without the note here: a route ignores names it does not
    declare (ADR-0308).
35. **No canonical link yet.** kiokun.com states one on every page. Here a
    page cannot state one until the integrator builds it:
    - `<link rel="canonical" href={WordPage(page.word)} />` at the view's
      top, written absolute from the deployment's configured origin (the
      ruling of 2026-10-09, PARALLEL's "1e's questions answered", Q2);
    - ADR-0293's Differences 10 and 12;
    - ADR-0189, which refuses one in the body (PW5035).
    When it lands, the word page states it always: self-referencing on
    `/word/食べる`, and naming `/word/食べる` on `/word/食べた`.
36. **A する verb JMdict marks vs-s is read as a する verb for する's own
    forms** (供した, 供しない, 愛した → 供する, 愛する). kiokun.com finds those
    only through the godan 供す and the noun 供. The oracle counts each by
    name.
37. **いい beside よい**: a form on よ (よかった) is also read back to いい
    (adj-ix), whose forms the grammar builds on よ. kiokun.com reads it as よい
    alone, and finds nothing where only いい has an entry. The oracle counts
    each by name.
38. **The reasons are the rewrite's own words** ("negative, past",
    "causative, passive or potential, past"), where kiokun.com shows its
    own labels. They are not compared.
39. **A candidate's redirect is followed one hop**, where kiokun.com follows
    up to three. The oracle found no form where that differs.
40. **A word in romaji is not read back.** kiokun.com converts an
    all-romaji word to hiragana first. The rewrite's addresses are written
    forms, and its search reads romaji.

## Found

1. **The oracle**, every 128th entry of the whole dictionary: 11,732 entries
   read, 189 words, 3,873 inflected forms.
   - 0 unnamed differences.
   - Named: 22 forms of vs-s verbs read as する verbs (Difference 36), and 12
     いい beside よい (Difference 37).
2. **Before the named rules**, the first comparison found 73 differences,
   each kind then taken from the cited facts or named:
   - kiokun.com reads a katakana word in hiragana too (がさ入れ beside
     ガサ入れ);
   - it reads the contracted causative-passive (募らされる → 募る);
   - it reads the hybrid する of 愛する;
   - it reads 〜そう, and a one-row verb's bare stem (手をつけ → 手をつける);
   - and it orders equal readings potential first, one-row voices before
     godan ones.
   - A general short causative 〜す (書かす) was tried and taken out: it made
     forms kiokun.com does not (募らした → 募る).

## Acceptance

- **The server's tests**:
  - `an_inflected_word_shows_the_dictionary_form_it_is`, over entries made
    for it: 食べた, 食べなかった, 食べさせられた, 書いて, 書ける, 書かれる,
    行った, 勉強しました, 来なかった, 高くなかった, よかった, each shown with its
    note; はいた looked up by reading to 履く and 吐く, past a noun はいる whose
    entry does not fit; たべた shown as たべる, whose stub names 食べる;
    ほげほげ not found; 書く its own page.
  - `deinflection_matches_kiokuns_answers_for_the_sample` (CI): 501 forms,
    each of Differences 36 and 37 counted.
  - `deinflection_matches_kiokuns_own_on_a_sample` (local, ignored).
- **The scripts' tests**: the deinflection sample keeps an entry's key and
  JMdict's words alone, and the committed one holds nothing else (a mutant
  in `kiokun_inventory_mutations.py`).
- **The oracle** (`just e14-kiokun-deinflection`, local): Found 1.
- **Mutation controls**: eighteen in `scripts/kiokun_word_mutations.py`,
  suite `deinflection`, measured by `scripts/mutant_killers.py` against its
  two tests, which are its cover. Each is killed:
  - a godan く's onbin, 行く's;
  - a one-row hypothetical, 来る's irrealis;
  - an adjective's past, いい's よ;
  - 愛する's sa-row;
  - the negative and ます not auxiliaries;
  - one round of reading back;
  - katakana read only as written;
  - the candidates unordered;
  - any entry fitting;
  - no reading lookup;
  - a redirect not followed;
  - one form kept;
  - a bare stem taking two steps;
  - the inflected address not found.

  The hand test gained a noun はいる and a stub たべる for two of them, which
  first survived. One in `kiokun_inventory_mutations.py` (27 of 27).

## Not claimed

- **Korean deinflection** (`korean-deinflect.ts`): kiokun.com's own, its
  source not yet checked.
- **The search box's deinflection** (`navigateOrSearch`): there is no search
  box until a route reads its query part (the search ADR).
- **The conjugation table** (`ConjugationTable.svelte`): the inventory's.
- **Forms kiokun.com reads that the oracle's inputs do not ask**: the
  inputs are the rewrite's own conjugator's forms. A form kiokun.com knows
  and the rewrite does not is found only when an input asks it.
