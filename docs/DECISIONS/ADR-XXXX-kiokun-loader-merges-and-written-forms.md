# ADR-XXXX: kiokun's loader merges, and the header's written forms

Status: proposed by track `kiokun` (W6, `track/kiokun`), the third milestone
of step 1, the word page (docs/PARALLEL.md, "W6's inventory, answered").
Builds on ADR-0286 and on the labels and header milestone (ADR-XXXX,
kiokun's labels and character header). Date: 2026-10-09. Milestone: E14.

## Context

- **kiokun.com's loader reads more than the word's file**
  (`sveltekit-app/src/routes/[word]/+page.ts`):
  - a stub followed keeps its own mnemonic card as the page's (`:438-462`);
  - a stub of several traditional forms shows each one's Chinese words
    (`:464-492`): 当 is 當 and 噹;
  - up to twelve related forms are read, and a simplified form that is the
    same concept as its one traditional form has its words, names and
    cards shown on the traditional page (`:153-280`,
    `mergeEquivalentFormData`, `equivalentTraditionalTarget`);
  - the same check answers the canonical redirect, 308 to the traditional
    page (`:419-425`).
- **The header's written forms** (`buildCharacterHeaderForms`,
  `src/lib/character-forms.ts:205-351`) are the specimens kiokun.com shows
  first: each form of the character, labelled Trad, Simp, HK, JP or KR, with
  its learner meaning.
- **The integrator's answers of 2026-10-09**: a page redirect its query
  decides is the integrator's to build, declared on the query's error case
  with a checked page address; `KiokunError` gains `Moved(String)` now. The
  batched read is accepted.

## Decision

1. **One batched read.** `kiokun:data/entries#read-all(places)` reads each
   place as `entries#read` reads one, in order. The query reads the word's
   file, then a stub's target, then every extra file in one call: the
   stub's traditional variants, then the related forms. Pleris computes the
   places (`shards.subdirectory`, `file_name`).
2. **The loader's rules, in Pleris**, each named for the function it is
   read from:
   - `followed_from`: the stub's card, else the target's, is the primary;
     the variants are the target's card and variants and the stub's, each
     character once (`mergeSemanticMnemonicVariants`).
   - `variant_words`: a stub's traditional variants where it names more
     than one; their Chinese words are added once by id (`mergeUniqueBy`).
   - `related_words`: the canonical character's simplified variants, the
     form it is the simplified form of, and the stub's key, each once, never
     the canonical character, at most twelve. The stub is not read again.
   - `equivalent_target` (`equivalentTraditionalTarget`): no redirect; a
     simplified form of one character whose one traditional variant is the
     target; and both cards' meanings the same concept (`normalizedConcept`:
     the learner gloss, lower-cased, its white space collapsed).
   - `merged_form` (`mergeEquivalentFormData`): the related form's Chinese,
     Japanese and Korean words and names added once by id, and its cards
     added to the variants.
   - `KiokunError.Moved(String)` is declared, and `equivalent_target` decides
     it. The query does not return it until the page can answer it.
3. **The written forms** (`forms_of`):
   - each Han character of the page's entry and each related form's,
     with roles by `addContext`'s rules (traditional, simplified, Hong Kong,
     Japanese, Korean);
   - a form's roles are the ones the page's own entry gives it where it
     gives any, else every context's, ordered as `ROLE_ORDER` orders them;
     then the canonical character, the cards' characters and a
     one-character word, added where missing, with no role ("Form");
   - its language (`languageTagForRoles`), its label (`Traditional / Hong
     Kong / Korean`) and its short roles (`Trad · HK · KR`);
   - its meaning: its card's (`collectMnemonicCards`: the page's cards, then
     the stub's), else kiokun.com's component gloss for it, each with its
     source markers removed. A meaning shows where there are several forms,
     or where the one form's is not the headline's concept
     (`shouldShowCharacterFormMeaning`).
   - The component glosses come from `KIOKUN_APP`'s
     `static/game_data/component_glosses.json`, by a second host op,
     `kiokun:data/support#glosses(characters)`, for the characters kiokun.com
     asks the table for that can be a form (`collectSupportKeys`: the word's
     own, the entry's and the stub's forms, and the cards').
   - A Han character is Unicode's `Script=Han`, as a browser's
     `/^\p{Script=Han}$/u` tests it: the ranges Node 22's engine gives
     (Unicode 16.0, ICU 77.1), written out in decimal. Shards' `is_han`
     is the builder's set, not this one.
4. **An app is its label table and its component glosses together**: one
   named without either is refused at start.

## Found

1. **A hanja form is a code point of its own.** 樂's hanja table gives
   `hanjaForm` U+F914, the compatibility ideograph, not U+6A02, so
   kiokun.com shows four specimens for 樂, one of them Korean only. The
   rewrite does as kiokun.com does. Worth the owner's look: the two render
   alike in most fonts.
2. **A word page is a larger component now** (178 KB, 170 KB of core Wasm,
   from 46 KB), and the server's kiokun tests take about twice as long. Not
   measured per request; a measurement waits for the whole-dictionary run.

## Differences from kiokun.com

Added to ADR-0286's list:

7. **The canonical redirect is not answered yet.** kiokun.com answers 308
   from a simplified character equal in meaning to its traditional form;
   this page shows the simplified character's own entry until the
   integrator's page redirect lands.
8. **`normalizedConcept` without NFKC.** Pleris has no Unicode
   normalization, so two meanings equal only after NFKC (full-width letters,
   ligatures) are not one concept here. kiokun's meanings are English.
9. **The component glosses asked for** are the characters that can be a
   form, not every character kiokun.com asks for (its components and their
   uses): only a form's meaning reads them so far.

## Alternatives

- **One host call per extra file.** Refused: up to fourteen calls for a page,
  each in the query's body until ADR-0283 lands.
- **The host merges entries.** Refused: the merges are the page's rules.
- **Shards' `is_han` for a form.** Refused: kiokun.com's forms read
  `Script=Han`, which includes the CJK radicals and `々`, and shards' set is
  the builder's.

## Acceptance

- **The server's tests, 20**: the earlier 15 and:
  - the written forms of 樂, from the sample and a gloss table a test writes,
    their roles, labels, languages and meanings;
  - a stub's page shows its target's forms and its own (谚, 諺);
  - a stub's card heads the page, and the target's where the stub has none;
  - a stub of several traditional forms shows each one's words, each once;
    a stub of one shows its target's alone;
  - an equivalent simplified form's words on its traditional page, and a
    form of another concept's not.
- **The browser**: the spec reads 人's one form and its roles, with script
  on and off.
- **Mutation controls**: 45 mutants in `scripts/kiokun_word_mutations.py`,
  12 of them this milestone's.
- `just e14-kiokun-word` records them.

## Not claimed

- **The canonical redirect** (Differences, 7).
- **The forms' stroke animations**, which are script.
- **Every related-form case on the whole dictionary**: the tests write the
  cases; a run over every entry is local and not recorded yet.
