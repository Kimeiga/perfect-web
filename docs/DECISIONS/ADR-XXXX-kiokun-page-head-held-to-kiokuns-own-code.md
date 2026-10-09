# ADR-XXXX: kiokun's page head, held to kiokun.com's own code

Status: proposed by track `kiokun` (W6, `track/kiokun`), the word page's SEO
head (step 1, milestone 1e in the integrator's order of 2026-10-09). It also
records the sampled whole-dictionary timing run the integrator ordered
first. Builds on ADR-0286 and the two milestones after it (ADR-XXXX,
labels and character header; ADR-XXXX, loader merges and written forms).
Date: 2026-10-09. Milestone: E14.

## Context

- **kiokun.com's word page describes itself** (`src/lib/seo.ts:163-198`,
  `buildDictionarySeo`; `components/SeoMeta.svelte`): a title, a
  description, robots, a canonical link, Open Graph and Twitter tags, a link
  preview image drawn by `/api/og`, and JSON-LD. Its static shells carry the
  same for 2,322 frequent words, since the page itself renders in the
  browser.
- **Pleris writes a page's metadata from the top of its view** (ADR-0186):
  `<title>` and `<meta>`. A `<link rel="canonical">` in markup is refused
  (PW5035, R-053), and a page writes no script (ADR-0094).
- **The integrator ordered numbers first** (2026-10-09): a sampled run over
  the whole dictionary, with per-request times, before the word page's
  component grew further.

## Decision

1. **The sampled run** (`just e14-kiokun-sample`, LOCAL_ONLY): every 256th
   file of every subdirectory of `KIOKUN_DATA`, 5,938 of 1,485,890, each
   looked up by the word it records through the server's HTTP path, in
   release; each status, and the time to the last byte. The same sample on
   ADR-0286's program is the baseline. A test holds the recipe local.
2. **The head, in Pleris** (`seo_of`), from the entry the loader leaves:
   - **the title**: the word, up to three of its other forms
     (`dictionaryForms`: the character, its Hong Kong, simplified and
     traditional forms, KANJIDIC's and the hanja table's), and its first
     three meanings, `<word> (<forms>) — <meanings> | Kiokun`;
   - **the meanings**: the learner gloss, then the definitions as
     `definitionFragments` makes them: the Chinese words' senses, the
     Japanese words' English glosses, KANJIDIC's English meanings, the Korean
     words' definitions and the hanja table's English meanings; each with its
     tags out and its white space one space (`cleanText`), split at `;` and
     `；`, a leading `2)` or `1.` off, longer than one unit and at most 96, no
     classifier (`CL:`, `classifier:`); each once;
   - **the description**: `<subject> means <meanings>. Character readings,
     definitions, examples, and learning tools across Chinese, Japanese,
     and Korean.` (`Word` for a word of more than one character);
   - **truncated as JavaScript truncates**: at 68 and 158 UTF-16 units, a
     code point past U+FFFF two, cut to one less, trimmed at the end, `…`.
     A cut that would split a surrogate pair stops before it; JavaScript
     would keep half a pair.
   - The head carries the title, the description, `robots` (`index,
     follow`), Open Graph's type, site name, locale, title and description,
     and Twitter's title and description.
3. **kiokun.com's own code is the oracle** (`just e14-kiokun-seo`,
   LOCAL_ONLY). `spikes/own-renderer/kiokun-oracle/seo.mjs` runs kiokun.com's
   `seo.ts` and `character-forms.ts` with Node's type stripping, copied
   from `KIOKUN_APP` into a temporary directory each run and never into this
   repository (Q3), over every 256th entry its loader shows as it is (no
   stub, no character with simplified variants or a simplified-form-of,
   whose pages merge other entries). An ignored server test compares each
   word's served title and description with its answer.
4. **The layer decodes what the head reads**: a gloss's language, KANJIDIC's
   meanings by language (`groups[].meanings`), the hanja table's English
   meanings (`meaningsEn`).

## Found

1. **The word page's cost is its renderer's, not its component's.**
   - The sample: 5,938 of 5,938 answered 200. At 1c, p50 2.3 ms, p90 5.6,
     p99 13.8, max 311.1 (さえこ); ADR-0286's program on the same sample,
     p50 1.9, p90 4.6, p99 12.6, max 279.9. The Word component grew from
     45,906 to 177,871 bytes for about 0.4 ms at p50, within the machine's
     load (load average 16 to 53).
   - さえこ has 128 Japanese names. The host reads and decodes it in 0.5 ms;
     the query computes all 128 in about 1.3 ms; rendering them takes about
     215 ms. Rendering is quadratic in a list's length: 32 names 16.7 ms, 64
     57.7 ms, 128 218 ms.
   - The cause: `pw-render`'s `Env::with` and `within` clone the whole
     environment for every loop item, the page's whole value included. The
     integrator confirmed it and is fixing it as an ADR of its own (bindings
     shared by `Arc`), with this curve as its evidence. The long lists of the
     next milestone (Contains and Appears in, up to 600 items) wait for it.
2. **kiokun.com's own code can be the oracle for its pure functions.** Run
   by Node over the owner's data, it answers 5,513 sampled words; the
   rewrite matches every title and description. The same harness can hold
   later milestones to kiokun.com's other pure functions.

## Differences from kiokun.com

Added to ADR-0286's list:

10. **No canonical link.** A `<link rel="canonical">` in a page's markup is
    refused (PW5035); kiokun.com's is the requested slug, absolute, on the
    page's origin. A question for the integrator (below).
11. **No link preview image, so no `og:image` and no `twitter:card`.**
    kiokun.com's is drawn by `/api/og` (resvg and CJK fonts), which needs an
    image renderer the owner approves. Pointing at an image that is not
    served would be worse than naming none.
12. **No `og:url`**, which is the canonical address (10).
13. **No JSON-LD.** kiokun.com writes a `WebPage` whose `mainEntity` is a
    `DefinedTerm`; a Pleris page writes no script (ADR-0094). A question for
    the integrator.
14. **A cut never splits a surrogate pair**, where JavaScript's would.

## Alternatives

- **Port the head without an oracle**, held by hand-written cases only.
  Refused once the oracle was possible: kiokun.com's code is the
  definition, and 5,513 words test it better than any list written here.
- **Commit kiokun.com's `seo.ts` as a fixture.** Refused: the owner's source
  stays in kiokun-data (Q3). The recipe copies it into a temporary
  directory each run.
- **Point `og:image` at kiokun.com's live `/api/og`.** Refused: it would
  make every rewrite page depend on the live site and its quota.

## Acceptance

- **The server's tests, 22** (and two ignored, local): the earlier 20 and:
  - the page describes itself as kiokun.com's does: 人's title, description,
    Open Graph's and Twitter's, robots; a stub's title names its other form;
  - the description takes kiokun.com's fragments and length: tags out, both
    semicolons, a number off, a classifier and a one-letter piece left out;
    a long title cut at 67 UTF-16 units with `…`, a character past U+FFFF
    counted twice.
- **The oracle** (`just e14-kiokun-seo`, local): 5,513 words compared, 0
  differences (`docs/evidence/E14/kiokun-seo.txt`).
- **The sample** (`just e14-kiokun-sample`, local):
  `docs/evidence/E14/kiokun-sample.txt`.
- **The browser**: the spec reads 人's title.
- **Mutation controls**: 53 in `scripts/kiokun_word_mutations.py`, 8 of
  them this milestone's; `scripts/kiokun_inventory_mutations.py`'s ci_plan
  mutant re-anchored, its script run whole (9 of 9).

## Not claimed

- **Times at low load**: the integrator asks for the p50 and p99 again
  once the renderer's fix lands, so the two compare like with like.
- **The head of a stub's or a merged form's page against the oracle**: the
  oracle reads one raw entry, and those pages are built from several; the
  server tests hold them.
- **The canonical link, the preview image and JSON-LD** (Differences).

## Questions for the integrator

1. **A canonical address.** kiokun.com's is absolute on the page's origin.
   Should Pleris let a page state its canonical address as a page address
   the compiler checks (as `navigate` and the redirect will), written by the
   host into the head with the deployment's origin? The host knows the
   origin; the program does not.
2. **JSON-LD.** Should a page be able to state structured data, a typed
   value the host serializes into the head's `application/ld+json`, escaped
   as ADR-0097 escapes embedded data? kiokun.com's is one fixed shape a
   page.
