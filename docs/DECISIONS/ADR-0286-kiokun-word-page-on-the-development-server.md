# ADR-0286: kiokun's word page, a third program on the development server

Status: accepted at its merge, 2026-10-09, under the owner's delegation of
2026-10-02; proposed by track `kiokun` (W6, `track/kiokun`), the first
milestone of step 1 in the integrator's order (docs/PARALLEL.md, "W6's
inventory, answered"), and reviewed (docs/PARALLEL.md, "W6's word page
(milestone 1a), reviewed"). Date: 2026-10-09. Milestone: E14, the owner's
production target.

## Context

- **The order.** The parity inventory (ADR-0285, kiokun's parity inventory)
  was answered on 2026-10-08. Step 1 is the word page from the entry. Every
  page is rendered by the server and works with script off (Q1). kiokun is
  a third program on the development server, beside the store and the feed,
  with a read-only data layer (Q2). The slice's own host stays as
  ADR-0037's and ADR-0041's evidence.
- **What the slice had.** A word page whose host joined each list into one
  string before Pleris saw it (`spikes/kiokun/server/src/data.rs`). It
  showed five things kiokun.com's page does not (Q8), and none of
  kiokun.com's Jyutping, numbering, common marks, labels or notes.
- **The reference** is kiokun.com's word page as its source states it:
  `sveltekit-app/src/routes/[word]/+page.svelte` and `+page.ts`, and the
  components it renders (`JapaneseWords/WordEntry.svelte`, `Sense.svelte`,
  `Definitions.svelte`, `JapaneseNames.svelte`).

## Decision

1. **A program of its own**, `examples/kiokun-site/app.pw`
   (`module kiokun.site`), built with kiokun's shard rule
   (`examples/kiokun/Shards.pw`, ADR-0041) and the record it names.
   - **The entry is kiokun's, as its build writes it.** `Entry` declares the
     fields the page reads, each list as the file lists it: a Chinese word's
     readings and their senses; a Japanese word's forms, readings and
     senses, with their tags, glosses, notes and labels; a Korean word's
     definitions; a name's translations and kinds.
   - **Where the file is, and whether it is the word's, is Pleris.** The
     subdirectory is `shards.subdirectory` (ADR-0041). The file name is
     kiokun's `create_safe_filename` (kiokun-data `src/main.rs`), ported:
     `/ \ : * ? " < > |` and every Unicode Cc character become `_`. A file
     answers only for the word its `key` records.
   - **A stub's `redirect` is followed one hop** (`+page.ts:438-462`),
     under the word asked for; the stub stands where its target is missing.
   - **What the page shows of the entry is Pleris**, each rule naming the
     component it is read from:
     - a Chinese reading only where it has senses; `simp / trad` where they
       differ; `[pinyin]`, `[jyutping]`; its senses as a list;
     - a Japanese word's written forms without JMdict's search-only ones
       (`sK`), each with its notes and a star where common, or its
       readings where it has none; its first reading; its senses, alone
       where there is one and numbered where there are more; a gloss's type;
       a sense's notes; its part of speech, field, misc and dialect labels;
     - a Korean word's hangul, `[hanja]`, part of speech and definitions,
       without kiokun's placeholder "Sentence";
     - a Japanese name's forms joined by `、`, each translation's texts
       joined by ", ", its kinds named as `JapaneseNames.svelte` names them.
   - **Each list a page keys is keyed by its source's id**, or by its place
     in the source's list where the source gives none (`<id>:<n>`).
   - **What kiokun.com does not show is not shown** (Q8): the stroke count,
     school grade, frequency rank, the Korean character's meanings and a
     Korean word's pronunciation. The owner rules on them with this page's
     first review.
   - **`source KiokunEntries`** states the guarantees (ADR-0207): `reads
     strong`, `transactions none`, `changes none`. The host holds the layer
     to them before it serves (ADR-0246).
2. **A read-only kiokun layer**, `spikes/own-renderer/server/src/kiokun.rs`,
   chosen at a `TRACK SEAM (kiokun)` in `from_build_with` where the
   program imports `kiokun:` (the integrator's ruling of 2026-10-08).
   - It supplies one operation, `kiokun:data/entries#read(subdirectory,
     file)`: the file inflated and decoded into `Entry`, or none.
   - It reads one of kiokun's files and nothing else: a subdirectory is two
     lowercase hexadecimal digits, a name one path segment, never `.` or
     `..`, whose file name `<file>.json.deflate` is at most 255 bytes
     (`NAME_MAX` on macOS and Linux), since kiokun's build wrote none
     longer (the integrator's review, below).
   - It reads `KIOKUN_DATA` where the deployment names it, and the
     repository's sample otherwise (ADR-0037). It stages nothing.
   - A record whose id repeats in its list is kept, as kiokun.com shows each,
     and keyed `<id>:<n>`.
   - `miniz_oxide` 0.9.1 inflates, the crate the slice's host already takes,
     at the version in `Cargo.lock`. No crate is new to the workspace.
3. **The route is `/word/{word}` for now.** kiokun.com's is `/{word}`, and a
   one-segment parameter route would take the host's own files
   (`/pw-runtime.mjs`). The integrator ruled (2026-10-08) that the host's
   paths move under `/_pw/` and that a literal segment beats a parameter; it
   builds that, and this page moves to `/{word}` when it lands.
4. **Built for the browser suite** by `spikes/own-renderer/kiokun-site.sh`
   into `dist-kiokun`, held to its runtime and its sources as `dist-feed` is,
   and served on `KIOKUN_PORTS`, one host per engine.

## Found

1. **A host call reached through a helper is not imported.** The first
   draft read entries in a function the query called. `pw build` refused
   it: "`Word` calls `kiokun:data/entries#read` and the world
   `kiokun-site-word` does not import it". `contract.rs`'s `host_calls` reads
   the declaration's own body, and the backend lowers every function the
   export reaches. The checker passed the program; the build refused it.
   The query now calls the host in its own body. A question for the
   integrator (below).
2. **A fold's empty seed needs its type written.** `List.fold(xs, [], ..)`
   was refused by the backend ("an empty list whose element type nothing
   fixes yet"); `let none: List<T> = []` fixes it. The checker passed it.
4. **A word too long to be a file name was an error, not a 404** (the
   integrator's review of 2026-10-09). The layer passed any one segment to
   the file system, and a file name over 255 bytes is refused there
   (ENAMETOOLONG, which Rust's `std` reports as `InvalidFilename`, not
   `NotFound`): `/word/` and 243 `a`s answered 503, the read's error. The
   layer now refuses such a name, and the word is a 404, as any word kiokun
   lacks; a word at the limit is read. The test makes every subdirectory, as
   a whole build has, since a missing one answers `NotFound` first.
3. **The renderer's part markers are in every served page**
   (`<!--pw:s0-->`), so a test reads a page's text through them, not its
   raw markup.

## Differences from kiokun.com

kiokun.com's word page, as its source states it, is the reference. Each
difference here is stated with its evidence; any other the clone comparison
finds is a defect.

1. **Rendered by the server, and shown with script off.** kiokun.com sets
   `ssr = false` (`src/routes/+layout.ts:4`) and shows nothing without
   script. Ruled (docs/PARALLEL.md, "W6's inventory, answered", Q1).
2. **The route is `/word/{word}`**, not `/{word}`, until the host's paths
   move under `/_pw/` (Decision 3).
3. **The title is `<word> | Kiokun`.** kiokun.com's is `buildDictionarySeo`'s
   (`src/lib/seo.ts:163-198`), with its description, canonical link, link
   preview and JSON-LD; the SEO head is a later milestone of step 1.
4. **Japanese labels are JMdict's codes** (`n`, `ctr`), where kiokun.com
   shows its table's labels (`japaneseLabels.ts`); the next milestone reads
   the table through `KIOKUN_APP`. There, by the integrator's ruling (Q4 of
   the review), a code is read as written, else with each `-` read as `_`,
   else as itself: kiokun.com's table keys 53 codes with `_` while its
   entries carry JMdict's `-`, so kiokun.com shows `adj-na` where its table
   means `na adj.`. That is kiokun.com's defect, not matched.

## Alternatives

- **Keep the slice's host and grow it.** Refused by Q2: the development
  server already has the runtime, sessions and commands later steps need.
- **Decode in the host into the page's shapes**, as the slice's host
  joined strings. Refused: what the page shows is the program's to decide,
  and its rules then sit beside the component they are read from.
- **Compute the file's place in the host.** Refused: ADR-0041 moved
  kiokun's shard rule into Pleris, and the escape rule belongs with it.
- **`/w/{word}` for good.** Refused by the integrator: it breaks every
  kiokun.com link.

## Consequences

- kiokun is served by the same host as the store and the feed, from the
  same build machinery. Later steps (search, accounts, browser features)
  build on it.
- Japanese labels are JMdict's codes (`n`, `ctr`, `uk`) until kiokun's
  label table is read through `KIOKUN_APP` (the next milestone).

## Acceptance

- **The server's tests, 11** (`server/src/tests/kiokun.rs`), on the
  repository's sample and on files a test writes:
  - the host chooses the kiokun layer, which grants reading entries alone;
  - each language's words, as kiokun.com's page shows them;
  - what kiokun.com does not show is not shown;
  - one Japanese sense alone and many numbered;
  - a stub's redirect followed, under the word asked for;
  - a word kiokun lacks is a 404, with its control;
  - a file answers only for the word it records: `a/b/b` and `a|b"b` share
    `47/a_b_b.json.deflate`;
  - what kiokun.com leaves out, left out: a reading with no senses, a
    search-only form, the placeholder "Sentence", each with its control;
  - the layer reads no path but one of kiokun's files;
  - a record listed twice is shown twice, each keyed once;
  - a word too long to be a file name is a 404, and one at the limit is
    read.
- **The browser, in three engines** (`e2e/kiokun-site.spec.mjs`): the page
  with script on, with no console error; the same with script off; a stub
  followed; a 404; the extras not shown.
- **Mutation controls** (`scripts/kiokun_word_mutations.py`): 15 mutants,
  each undoing one rule, in the program, the layer or the seam.
- `just e14-kiokun-word` records all three in
  `docs/evidence/E14/kiokun-word.txt`.

## Not claimed

- **The rest of the word page.** This milestone is the words sections.
  Still to come in step 1: kiokun's labels for Japanese tags; the
  character header (forms, readings matrix, HSK and JLPT); examples;
  pitch accent; mnemonics and components; contains and appears in; the
  loader's canonical redirect, variant merge and related forms; the SEO
  head (the title here is `<word> | Kiokun`, not kiokun.com's).
- **kiokun.com's URL**, `/{word}`, until the host's `/_pw/` lands.
- **The whole dictionary.** The tests and the browser run on the sample;
  a run over every entry is local and not yet recorded.
- **Exact text parity.** The sections' text follows the source's rules;
  no page of kiokun.com's was rendered beside it yet (the clone, Q5).

## Questions for the integrator

1. **A host call through a helper** (Found 1). Should `host_calls` follow
   the functions a declaration calls, as the backend does? Or should the
   checker refuse what the build will refuse? Today the checker passes the
   program and the build refuses it.
2. **The empty seed** (Found 2). Should the checker give `[]` its type from
   the fold's other arguments, or report it where the build would?

## The integrator's answers

Its review of 2026-10-09, relayed by message:

1. **A host call through a helper**: the contract is wrong and the build is
   right. A component's imports are what its compiled code calls, the
   export's body and every Pleris function it reaches, transitively.
   ADR-0283, the integrator's; the reads may then move into a helper.
2. **The empty seed**: ADR-0284, the integrator's. **Corrected at the
   merge**: the answer first relayed, that the checker's acceptance stood
   (ADR-0065 §2), was wrong. The checker bound the accumulator to a list of
   anything whole, so the function fixed nothing, and it passed ill-typed
   folds. Now the function fixes it, and the backend gives the seed the
   fold's solved type. Until then `let none: List<T> = []` stays.
3. **Ports**: 7341 is the track's, 200 from W5's 7141.
4. **kiokun.com's label table**: not reproduced (Differences, 4).
5. **Fixed before the merge**: a word too long to be a file name (Found 4).
