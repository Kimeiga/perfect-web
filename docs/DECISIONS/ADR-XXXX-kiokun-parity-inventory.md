# ADR-XXXX: kiokun.com's parity inventory, every route marked

Status: proposed by track `kiokun` (W6, `track/kiokun`), under the
integrator's rulings of 2026-10-08 (docs/PARALLEL.md, "kiokun.com in Pleris
(W6's plan)"). The track's first ADR. The integrator orders the gaps from
it. Date: 2026-10-08. Milestone: E14, the owner's production target.

## Context

- **The goal.** kiokun.com, the owner's Chinese, Japanese and Korean
  dictionary, is rewritten in Pleris and served in production as Pleris's
  production target. It builds on the kiokun slice (ADR-0037, ADR-0041):
  a word page and a search page, server-rendered, with no script.
- **The ruling.** The track begins with an inventory of every route and
  feature of the SvelteKit app, each marked built in Pleris, partial or
  missing, with what it needs of the language or the platform. Then the
  gaps are built in the order the integrator gives.
- **What was read.** The app's source, in the owner's kiokun-data checkout
  (`sveltekit-app/`), at `abf71a89` plus the owner's uncommitted changes to
  11 files (among them `[word]/+page.svelte` and the untracked
  `src/lib/character-lessons/`). The working tree was read as it stood.
  The builder's source (`src/main.rs`, `src/search_index_builder.rs`), the
  data's layout (`output_dictionary`) and the app's static data were read
  where a row depends on them. Nothing was written into the checkout, and
  no tool of the app's was run.

## Method

1. **The routes are SvelteKit's own.** A route is a directory under
   `src/routes` holding a `+page` or a `+server.ts`: 102 of them, 49 pages
   and 53 endpoints. `scripts/kiokun_inventory.py` reads them from the
   checkout and holds this ADR to them: every route is a row below, and
   every row is a route (`just e14-kiokun-inventory`).
2. **The source was read whole.** Four readers covered the word page and
   its components; search, the home page and the reference pages; accounts
   and study; and the learning and content routes. Each claim cites a file
   and line. The claims this inventory leans on hardest were read again
   here: rendering (`+layout.ts`), the blog's prerendering, the pitch
   accent data, the frequency list, the category tree, the security
   findings below.
3. **The live site was not sampled.** Its `robots.txt` disallows every
   path but a few for all agents, and disallows ClaudeBot, Claude-Web and
   anthropic-ai entirely. The ruling says to respect it, so no request was
   made. The source is the reference.
4. **The app was not run.** Its packages are installed, so running it
   needs no download. But `vite dev` writes `.svelte-kit/` and
   `node_modules/.vite/` into the checkout, which the ruling forbids. A
   copy outside the checkout is the integrator's question (below).
5. **Status.** `built`: Pleris serves the route, at parity, with tests.
   `partial`: Pleris serves it, short of what the row names. `missing`:
   Pleris does not serve it. A route proposed not to be ported is still
   `missing`, and says so.

## Found

1. **kiokun.com renders no page on the server.** `src/routes/+layout.ts:4`
   sets `ssr = false` for the whole app. The Cloudflare adapter routes only
   `/api/*`, `/sitemap.xml` and `/favicon.ico` to functions
   (`svelte.config.js:16-19`). Every page is a static shell whose data the
   browser loads, `/<word>` included. The blog alone is prerendered
   (`blog/+layout.ts`: `ssr = true`, `prerender = true`). 2,322 frequent
   entries get copies of the shell with their own `<title>` and link
   preview (`scripts/write-static-entry-pages.mjs`).
   - So Pleris's server-rendered, script-free pages lose nothing that is
     server-rendered today. With JavaScript off, kiokun.com shows nothing;
     the slice shows the entry.
   - The comments that call the word page "complete and meaningful in SSR
     HTML" (`hooks.server.ts:61`, `[word]/+page.ts:19-21`, `robots.txt:1-3`)
     are stale.
2. **The word page reads more than the entry.** It reads the entry's file
   and, beside it: every traditional variant of a simplified stub; up to
   twelve related forms; the character's components from three static JSON
   files (2 MB, `static/game_data/`) through `/api/character-support`;
   pitch accents from `static/pitch/<hh>.json` (256 files, keyed by a hash
   over UTF-16 code units, not the shard rule's code points); sentence
   corpora (`static/zh_sentences`, `static/kr_sentences`); reels
   (`static/reel-index`, FNV-1a over 256 shards); homophones from the
   search index (`/api/lookup-reading`); artifacts and notes from D1.
3. **The slice shows what kiokun.com does not.** Stroke count, school
   grade, frequency rank, the Korean character's meanings and a Korean
   word's pronunciation are on the slice's page and on no page of
   kiokun.com's.
4. **kiokun.com's search is one FTS5 table over the whole dictionary**
   (`dictionary_search`, built by `src/search_index_builder.rs`: Chinese,
   Japanese and Korean rows, Jyutping since migration 0005). A CJK query is
   expanded to up to 24 script variants and aliases
   (`search-aliases-core.ts:54-82`, from `src/lib/generated/search-aliases.json`,
   608 KB). Hangul takes the CJK path. Groups are not re-sorted after SQL
   ranks them (`api/search/+server.ts:195-266`). The slice's ranking is a
   port of an older version over one shard (ADR-0041's correction).
   - The committed `output_search_index.sql` (February) has no
     `jyutping_search` column, which the builder now writes. The local index
     is older than the builder.
5. **Pitch accent is kiokun's data, not its entries'.** KNOWN_LIMITATIONS
   says "Pitch accent is not in kiokun's entries". That is true of the
   entry files, which carry none (`japanese_words[]` holds `id, kanji,
   kana, sense, frequencyRank`). But kiokun.com shows pitch accents, from
   `static/pitch/`.
6. **kiokun.com's own security findings** (the owner's to act on; none is
   ported):
   - **A stored script injection on the word page.**
     `ArtifactMentions.svelte:51-73` writes an artifact sentence's
     `originalText` through `{@html}` with only the matched word wrapped in
     `<mark>`. The text is stored as a signed-in user sent it
     (`api/artifacts/[id]/sentences/+server.ts:35-59`). A public artifact's
     sentence runs as markup for anyone who opens a word it contains.
   - **Two endpoints write with no session.** `api/uploads` signs an R2
     upload URL for anyone; the legacy `api/notes` reads, writes and deletes
     with none. Neither has a caller in `src`.
   - **`isAdmin` is true when `GOOGLE_CLIENT_SECRET` is absent**
     (`hooks.server.ts:20-24`): in a deployment that lost the secret, every
     request is the administrator.
   - **`api/users` counts private notes** in each user's public count.
   - **`api/mobile-auth/complete` puts the whole `Cookie` header** in the
     redirect's fragment, not the session cookie alone.
7. **Dead code in kiokun.com** (no caller in `src`): `/api/tutor/chat`,
   `/api/tutor/exercise`, `/api/learning-resources/words`,
   `/api/study/searches` (its caller is never mounted), `/api/notes` and
   `/api/uploads` (above); `/demo` and `/test` are developer pages. The
   keyboard shortcut `r` dispatches an event nothing listens for, and
   `/offline` links to a `/privacy` that does not exist.

## The routes

`Pleris today` names what the slice serves, by its evidence
(`docs/evidence/E10/kiokun.txt`); otherwise the row says what the route
needs. "Client" means it needs script in the browser; "accounts" means a
signed-in user's data.

### The dictionary

| route | kind | status | what it is | Pleris today, or what it needs |
|---|---|---|---|---|
| `/[word]` | page | partial | The entry: forms, readings, meanings, characters, words, names, examples, mnemonics, components, history, related words; loader redirects and fallbacks. | Pleris today: `Lookup` (one redirect) and `WordPage` over all 1.49 million entries, 404 for none, no script (ADR-0037, ADR-0041). 4 of the 50 features below are built, 10 partial. |
| `/search` | page | partial | 60 hits in Chinese, Japanese and Korean columns, a reading, up to three definitions, a Common badge; community words. | Pleris today: `SearchPage`, 20 hits over one shard, Chinese and Japanese rows. Needs the whole index, Korean rows, the variant expansion, three columns and the custom words (accounts). |
| `/api/search` | endpoint | partial | JSON search over the FTS5 index, used by the dropdown (8) and the search page (60). | Pleris today: the ranking is the compiled `Search` query, a port of an older version (ADR-0041's correction), over one shard; no JSON endpoint. Needs the current ranking re-derived from `api/search/+server.ts` and an index over every shard (the deployment's). |
| `/api/lookup-reading` | endpoint | missing | Kana to written forms: Japanese rows whose reading is the query. Feeds the word page's homophones and the deinflection fallback. | An exact-reading index beside the search index. |
| `/api/dictionary` | endpoint | missing | Proxies one entry file from raw GitHub, then jsDelivr, cached at the edge; `optional=1` turns a miss into 204. The client app's only path to an entry. | Not needed while pages render on the server: the host reads entries itself. Needed for a client that reads entries (an offline app). |
| `/api/dictionary/[word]` | endpoint | missing | The same proxy, by path. | As `/api/dictionary`. |
| `/api/korean-glosses` | endpoint | missing | Glosses each token of up to eight Korean sentences, by Korean deinflection and entry lookups. | Korean deinflection in Pleris (pure computation) and entry reads. |
| `/api/stroke-data` | endpoint | missing | Proxies hanzi-writer stroke JSON from jsDelivr. | An outbound fetch, or the data served locally; the animation is client. |
| `/api/handwriting` | endpoint | missing | Sends drawn strokes to Google Input Tools' unofficial endpoint (always Japanese: the client never sends a language). | An outbound fetch to a third party; a drawing canvas (client). |
| `/api/character-support` | endpoint | missing | Glosses, a taxonomy and component uses for the asked characters, from three static JSON files. | Static data read by the host; a query over it. |
| `/api/character-lesson/[character]` | endpoint | missing | A character lesson: 14 curated, else saved in R2, else generated by OpenAI (POST, rate-limited) and saved. | Curated lessons are data. Generating needs an LLM call with a secret and blob storage. |
| `/api/og` | endpoint | missing | A 1200×630 PNG link-preview card, drawn with resvg and CJK fonts. | Image rendering (a crate, the owner's to approve) or cards made at build. |
| `/api/frequency` | endpoint | missing | Redirects to `/frequency_list.json`. | A redirect and a static file. |
| `/api/cors-port` | endpoint | missing | Development only: the local CORS server's port. | Proposed not to port: the host reads the data directly. |
| `/phonetic/[char]` | page | missing | Characters that use one as a sound component, with readings from their own entries. | Static data, up to 30 entry reads; server-renderable. |
| `/category/[...path]` | page | missing | A tree of 23,592 characters by meaning, from 81 static index files. | Static data; a rest parameter in a route. |
| `/frequency` | page | missing | The 1,000 most frequent Japanese and Chinese words, paged; the Korean tab is always empty (the list has no Korean). | Static data, server-renderable paging. |
| `/homophones/chinese` | page | missing | Words or characters grouped by pinyin, filtered by the URL; notes per group. | Static data (1.8 MB), filters as a GET form; notes need accounts. |
| `/homophones/japanese` | page | missing | Groups by reading, romaji converted to kana, pitch badges, filters in the URL; notes per group. | Static data (2.3 MB), romaji-to-kana in Pleris; notes need accounts. |
| `/homophones/korean` | page | missing | Hanja words sharing a hangul reading; notes per group. | Static data; notes need accounts. |
| `/japanese-emoji` | page | missing | A written reference of 59 emoji with readings and use. | A static page. |
| `/sitemap.xml` | endpoint | missing | 19 paths and every course and lesson. | A generated list. |
| `/favicon.ico` | endpoint | missing | Redirects to the PNG icon. | A redirect; the slice answers 404. |

### The site's frame and pages

| route | kind | status | what it is | Pleris today, or what it needs |
|---|---|---|---|---|
| `/` | page | partial | Today: search with suggestions and handwriting, the learner's next message, cards due, the lesson to continue, a character of the day, the atlas of demonstrations, reels, the tools. | Pleris today: an empty search page, whose form works with no script. The daily picks are deterministic and server-renderable; Today needs accounts or device storage; the demonstrations are client. |
| `/offline` | page | missing | A page about the iPhone app, with screenshots and a toggled demonstration. | A static page; its `/privacy` link has no page. |
| `/demo` | page | missing | A developer mock of notes, all in page state. | Proposed not to port. |
| `/test` | page | missing | A developer test of the pitch component. | Proposed not to port. |
| `/learning/resources` | page | missing | A page of links. | A static page. |
| `/learning-resources` | page | missing | Redirects to `/learning`. | A redirect. |
| `/learning-resources/japanese` | page | missing | Redirects to `/learning#japanese`, an anchor that does not exist. | A redirect. |
| `/learning-resources/japanese/scripting-japan` | page | missing | A list of a YouTube channel's videos from D1. | A table and a query; whether D1 holds rows is not known from source. |
| `/learning-resources/japanese/scripting-japan/[slug]` | page | missing | One video: an embedded player, its transcript and its words. | A query and an iframe. |
| `/api/learning-resources/videos` | endpoint | missing | The videos of a source, paged. | A query. |
| `/api/learning-resources/video/[slug]` | endpoint | missing | One video with its transcript and words. | A query. |
| `/api/learning-resources/words` | endpoint | missing | The words of a video. | Proposed not to port: no caller. |
| `/blog` | page | missing | The index of four posts, prerendered. | Static prose. |
| `/blog/the-data-engine` | page | missing | A post with five SVG diagrams. | Static prose and diagrams. |
| `/blog/translation-model-bakeoff` | page | missing | A post with tables and downloadable research files. | Static prose; files served. |
| `/blog/semantic-mnemonic-bakeoff` | page | missing | A post over a benchmark's JSON, with filters. | Static prose; filters as a GET form or client. |
| `/blog/japanese-reel-stt-benchmark` | page | missing | A post with a video synced to 20 transcription tracks. | Static prose and video; the sync is client. |

### Accounts and a user's data

| route | kind | status | what it is | Pleris today, or what it needs |
|---|---|---|---|---|
| `/api/auth/[...all]` | endpoint | missing | better-auth: Google sign-in, its callback, sign-out, the session. | The identity track's relying party (ADR-0258) with Google as the provider; the kiokun host has no sessions. |
| `/api/mobile-auth/complete` | endpoint | missing | Hands the iPhone app its session after sign-in. | Only if the iPhone app stays; not as written (Found 6). |
| `/users` | page | missing | Every user, with their note count. | Accounts; a public count of public notes only. |
| `/users/[userId]` | page | missing | One user's notes: search, sort, paging, Markdown. | Accounts; sanitized Markdown (a library). |
| `/api/users` | endpoint | missing | The users and their counts. | A query. |
| `/api/users/[userId]/notes` | endpoint | missing | One user's notes; their private ones to them alone. | A private query. |
| `/api/notes` | endpoint | missing | Legacy notes, with no session on any method, over columns the current table lacks. | Proposed not to port (Found 6). |
| `/api/notes/[character]` | endpoint | missing | One note per user and entry: read, create, edit, delete; an administrator's notes shown to all. | Accounts; commands; an administrator's role. |
| `/api/user/notes` | endpoint | missing | The signed-in user's notes. | A private query. |
| `/api/internal/notes` | endpoint | missing | A machine writes a note, signed with Ed25519; its sender does not exist in the checkout. | A signed request, or a script against the database. |
| `/api/images/[...path]` | endpoint | missing | Serves an uploaded image from R2; its owner may delete it. | The uploads track's blob storage (ADR-0260). |
| `/api/images/upload` | endpoint | missing | Uploads an image (5 MB, four types) for a note or an artifact. | A typed upload (ADR-0260). |
| `/api/uploads` | endpoint | missing | Signs an R2 upload URL for anyone. | Proposed not to port (Found 6). |
| `/custom-words` | page | missing | The latest 100 words users added; filters; delete one's own. | Accounts; a query. |
| `/custom-words/new` | page | missing | Add a word: language, forms, definitions, parts of speech, notes. | A form and a command; the clash with the dictionary checked on the server, which kiokun.com does not do. |
| `/api/custom-words` | endpoint | missing | List and add custom words. | A query and a command. |
| `/api/custom-words/[id]` | endpoint | missing | One custom word: read, edit, delete. | Commands; owner or administrator. |
| `/api/custom-words/by-word/[word]` | endpoint | missing | A custom word by its word: the word page's last fallback. | A query read by `Lookup`'s fallback. |
| `/lists` | page | missing | "My notes": one's own notes, searched and sorted. | A private page. |
| `/study` | page | missing | Review, one card at a time, SM-2; a device deck without an account, the account's deck with one. | SM-2 in Pleris (pure); accounts; a device deck is client storage. |
| `/study/cards` | page | missing | One's cards: filter, sort, reset, delete. | A private page; commands. |
| `/study/decks` | page | missing | Twelve decks to import, labelled JLPT, HSK and TOPIK, which are slices of the frequency list, not the exams' lists. | A static list and a command; the labels are the owner's to correct. |
| `/study/stats` | page | missing | Totals, due today and this week, a forecast, leeches. | A private aggregate query. |
| `/api/study` | endpoint | missing | Card reads and writes, decks, a card's sentence. | Commands and private queries. |
| `/api/study/import` | endpoint | missing | Adds up to 100 words to a deck. | A command. |
| `/api/study/review` | endpoint | missing | Rates a card; SM-2 updates it. | A command over SM-2. |
| `/api/study/searches` | endpoint | missing | Adds a looked-up word to a deck. | Proposed not to port: its caller is never mounted. |
| `/api/study/stats` | endpoint | missing | The statistics, computed over every card. | A private query. |
| `/api/sync/pull` | endpoint | missing | The iPhone app's pull: notes, cards, custom words, artifacts, tombstones since a time. | Only if the iPhone app stays. |
| `/api/sync/push` | endpoint | missing | The iPhone app's push: up to 250 changes, the last write winning. | Only if the iPhone app stays. |
| `/api/learning-settings` | endpoint | missing | One setting: save sentences automatically. | A private query and a command. |
| `/sentences` | page | missing | One's saved sentences. | A private page; a command to delete. |
| `/api/sentences` | endpoint | missing | Save a sentence: each word it holds becomes a card with the sentence. | Commands over several tables in one transaction. |

### Learning

| route | kind | status | what it is | Pleris today, or what it needs |
|---|---|---|---|---|
| `/courses` | page | missing | The four A1 courses: Japanese, Mandarin, Cantonese, Korean, each previewed. | Static data (about 4,570 lines of course modules), server-renderable. |
| `/courses/[language]` | page | missing | A course's units and lessons, with progress. | Static data; progress is device storage today. |
| `/courses/[language]/[lessonId]` | page | missing | A lesson: a dialogue, notes, charts, words, graded activities. | Static data; grading is pure (forms, or client); progress as above. |
| `/courses/japanese` | page | missing | The Japanese course, which the `[language]` page reuses. | As `/courses/[language]`. |
| `/courses/japanese/[lessonId]` | page | missing | A Japanese lesson. | As `/courses/[language]/[lessonId]`. |
| `/learning` | page | missing | Practice as a conversation: reply to an authored message, or explain one, and an AI checks it. | The checks are LLM calls with a secret; the chat state is device storage; a page per check would need none. |
| `/tutor` | page | missing | The same as `/learning`. | As `/learning`. |
| `/api/tutor` | endpoint | missing | Grades an answer: an exact certified answer first, else two OpenAI models in parallel, a third where they disagree. | LLM calls with a secret, structured output, rate limits. |
| `/api/tutor/situation` | endpoint | missing | Grades a reply against numbered criteria, with one OpenAI call. | As `/api/tutor`. |
| `/api/tutor/chat` | endpoint | missing | A free question to the tutor. | Proposed not to port: no caller. |
| `/api/tutor/exercise` | endpoint | missing | Generates an exercise. | Proposed not to port: no caller. |
| `/api/exercise` | endpoint | missing | A sentence-building exercise from a public dataset on GitHub. | An outbound fetch, or the dataset read locally. |
| `/api/translate` | endpoint | missing | Machine translation through Cloudflare Workers AI (`m2m100-1.2b`). | A translation provider: an outbound call with a secret. |
| `/drill` | page | missing | Type a frequent word's reading; adaptive levels. | Static data; grading is pure (forms, or client); state per session or device. |
| `/game` | page | missing | Build a sentence from tiles by dragging; sounds, speech, a dictionary pop-up. | Client: dragging, audio, speech; grading is pure. |
| `/sentence` | page | missing | Reads a sentence: readings, a word-by-word breakdown, a translation; save it. | Segmentation (TinySegmenter and `Intl.Segmenter` today), entry reads, translation; server-renderable from the URL. |
| `/api/sentence/analyze` | endpoint | missing | Segments a sentence and glosses each word. | Segmentation and lookups (the host's or a library's). |
| `/api/sentence/ruby` | endpoint | missing | Readings for Japanese kanji tokens. | As above. |
| `/reel/[id]` | page | missing | A short video with its transcript scrolling in step, a word panel. | Static JSON, a `<video>` from a third-party CDN; the sync is client. |
| `/reel/[id]/compare` | page | missing | Nine transcription tracks of one reel, side by side (one reel has them). | Static JSON and video; low value. |
| `/artifacts` | page | missing | Photos of real text (signs, menus, packaging), filtered. | A query; filters as a GET form. |
| `/artifacts/new` | page | missing | Make an artifact: images, then sentences. | Accounts, typed uploads, a multipart form. |
| `/artifacts/[id]` | page | missing | An artifact: images, its sentences split into words; the owner edits. | Queries, commands, stored segmentation. |
| `/artifacts/published/[slug]` | page | missing | A curated breakdown checked into the app (one exists). | Static data. |
| `/api/artifacts` | endpoint | missing | List and make artifacts. | A query and a command. |
| `/api/artifacts/[id]` | endpoint | missing | One artifact: read, edit, delete. | Commands; owner. |
| `/api/artifacts/[id]/images` | endpoint | missing | Add or remove an artifact's image (R2). | Typed uploads. |
| `/api/artifacts/[id]/sentences` | endpoint | missing | Add, edit or remove a sentence, segmented on the server. | Commands; segmentation. |
| `/api/artifacts/by-word/[wordSlug]` | endpoint | missing | The artifact sentences that contain a word, for the word page. | A query; escaped text, never markup (Found 6). |

## The word page's features

| feature | where in kiokun.com | status | what it is | needs |
|---|---|---|---|---|
| entry read | `[word]/+page.ts:28-81` | built | Reads the word's raw-DEFLATE file by the shard rule. | Built: the host, every shard (ADR-0041). |
| not found | `+page.ts:407` | built | 404 when nothing matches. | Built. |
| headword | `+page.svelte:427` | built | The word as the page's heading. | Built. |
| Japanese names | `JapaneseNames.svelte` | built | Written forms, readings, kinds, translations. | Built (and kinds, which kiokun.com shows as tags). |
| upstream errors | `+page.ts:83-94` | partial | 429 becomes 503, 5xx 502. | The slice reads local files; a remote source would need the mapping. |
| redirect stub | `+page.ts:438-462` | partial | One hop; the stub's mnemonic stays first. | Built: the hop. Missing: the mnemonic merge. |
| canonical redirect | `+page.ts:419-425` | missing | 308 to the one traditional form a simplified character equals in meaning. | A redirect from a page's query (`Lookup` answering a target). |
| variants merged | `+page.ts:464-492` | missing | A stub with several traditional forms shows every form's Chinese words. | More entry reads in `Lookup`; a dedupe by id. |
| related forms | `+page.ts:153-280` | missing | Up to twelve related forms read and, where equivalent, merged. | Entry reads; Pleris computation. |
| deinflection | `+page.ts:353-379`, `deinflect.ts`, `korean-deinflect.ts` | missing | A conjugated Japanese or Korean word redirects to its dictionary form, with how it was conjugated. | Japanese and Korean deinflection in Pleris (about 1,470 lines of rules); `/api/lookup-reading`. |
| custom-word fallback | `+page.ts:381-405` | missing | A word users added, when the dictionary has none. | Accounts' data. |
| conjugation note | `+page.svelte:429-461` | missing | "from → word (how)", and other candidates. | The deinflection; URL parameters. |
| search box | `Header.svelte:84-127` | partial | A combobox with suggestions as one types, and handwriting. | Built: a plain search form. Missing: suggestions and handwriting (client). |
| word hero | `+page.svelte:550-559` | partial | The headword large, with save, practice and share. | Built: the headword. The actions are client and accounts. |
| built-from strip | `WordCharacterStrip.svelte` | missing | Each character of a word, glossed and linked. | The entry's `contains`; server-renderable. |
| false friends | `false-friends.ts:43-63` | missing | A warning when Chinese and Japanese meanings diverge. | Pleris computation over the entry. |
| form specimens | `+page.svelte:582-607` | missing | Each written form (Trad, Simp, HK, JP, KR) with its own meaning. | The entry and related forms. |
| stroke order | `CharacterLearningSections.svelte:848-1201` | missing | An animated stroke order from hanzi-writer data on jsDelivr. | Client; an outbound fetch or local data. |
| learner gloss | `+page.svelte:610-614` | missing | The mnemonic keyword, else the meaning, as a heading. | The entry. |
| HSK and JLPT | `+page.svelte:616-621` | partial | Exam-level badges. | Built: the JLPT level. Missing: HSK (`chinese_char.statistics.hskLevel`). |
| taxonomy | `+page.svelte:632-644` | missing | A category breadcrumb. | Static data (`char_taxonomy.json`). |
| sense highlights | `+page.svelte:645-652` | missing | Up to four other meanings, proper names last. | Pleris computation. |
| readings matrix | `+page.svelte:665-688` | partial | Mandarin by frequency, Cantonese, on'yomi, kun'yomi, Korean. | Built: the Korean reading. Missing: the rest, from the entry. |
| mnemonic | `SemanticMnemonicCard.svelte` | missing | The keyword, the equation and the story, with linked characters. | The entry's `semantic_mnemonic`. |
| mnemonic pictures | `MnemonicVisualGallery.svelte` | missing | A picture for the meaning and for each sound. | Images from the entry or a static manifest. |
| sound mnemonics | `PronunciationMnemonicLayer.svelte` | missing | Per reading: a strategy, a cue, anchor words. | The entry. |
| components | `CharacterLearningSections.svelte:304-441` | missing | The character's equation and parts, by role, with a phonetic series link. | The entry and static glosses. |
| component strokes | `CharacterLearningSections.svelte:443-847` | missing | A part's strokes coloured in the whole. | SVG from the entry's stroke data. |
| used in | `ComponentUses.svelte` | missing | Characters that use this one as a part. | Static data. |
| similar characters | `SimilarCharacters.svelte` | missing | Characters sharing parts. | Static data. |
| historical forms | `HistoricalEvolution.svelte` | missing | Oracle, bronze and seal images, attributed. | The entry's images (a third-party host). |
| scholarly comments | `CharacterLearningSections.svelte:1577-1595` | missing | Sourced comments. | The entry. |
| character lesson | `CharacterMiniLesson.svelte` (uncommitted) | missing | A curated or generated lesson for one character. | Curated data; generation is an LLM call. |
| Chinese words | `+page.svelte:707-778` | partial | Forms, pinyin, Jyutping, numbered definitions. | Built: forms, pinyin, definitions. Missing: Jyutping, numbering, speech, save. |
| Chinese examples | `SenseExampleList.svelte` | missing | A sentence per definition, with ruby. | The entry; expanding is client or a details element. |
| Chinese corpus | `ChineseSentenceExamples.svelte` | missing | Sentences from a static corpus where the entry has none. | Static data, a hash rule. |
| Japanese words | `JapaneseWords/WordEntry.svelte` | partial | Written forms with tags and a common mark, readings. | Built: forms and readings. Missing: tags, the common mark, the search-only forms dropped. |
| pitch accent | `components/PitchAccent.svelte` | missing | High and low morae, and the pattern's name. | Static data (`static/pitch/`), its own hash rule. |
| Japanese senses | `JapaneseWords/Sense.svelte` | partial | Glosses, part of speech, field, dialect, notes, labelled. | Built: glosses. Missing: the tags and their labels (`japanese-labels.json`). |
| Japanese examples | `Sense.svelte:66` | missing | Example sentences with readings. | The entry; readings need segmentation. |
| conjugation table | `ConjugationTable.svelte`, `conjugate.ts` | missing | A verb's forms. | Pleris computation; showing it on demand is a details element or client. |
| Korean words | `+page.svelte:805-861` | partial | Hangul, hanja, part of speech, numbered definitions, examples. | Built: hangul, hanja, definitions. Missing: part of speech, numbering, examples, glosses. |
| Korean corpus | `KoreanSentenceExamples.svelte` | missing | Sentences from a static corpus, romanized, glossed. | Static data; Korean glosses. |
| homophones | `+page.svelte:379-420` | missing | Other words read the same, for each kana reading. | `/api/lookup-reading`'s index. |
| contains | `Contains.svelte`, `contains-order.ts` | missing | The words and characters within, ordered breadth first. | The entry's `contains`; paging instead of infinite scroll. |
| appears in | `AppearsIn.svelte`, `appears-in-order.ts` | missing | Words containing this one, in three languages, by frequency. | The entry's `contained_in_*`. |
| notes | `Notes.svelte` | missing | Users' Markdown notes on the entry, with images. | Accounts, uploads, sanitized Markdown. |
| found in artifacts | `ArtifactMentions.svelte` | missing | Artifact sentences containing the word. | Accounts' data; escaped (Found 6). |
| reels | `ReelsSection.svelte` | missing | Short videos saying the word. | Static data, FNV-1a; thumbnails on a third-party CDN. |
| SEO head | `seo.ts:163-198`, `SeoMeta.svelte` | missing | Title, description, canonical, Open Graph, Twitter, JSON-LD. | Server-rendered head; Pleris can render it where kiokun.com cannot (Found 1). |

## The site's features

| feature | where in kiokun.com | status | what it is | needs |
|---|---|---|---|---|
| header and tab bar | `Header.svelte`, `AppTabBar.svelte` | missing | Four destinations, a phone tab bar, the review count. | Static navigation; the count needs accounts or device storage. |
| theme | `ThemeToggle.svelte` | missing | Light and dark, following the device until chosen. | `prefers-color-scheme` in CSS; a choice needs a cookie or client. |
| language filter | `LanguageToggle.svelte` | missing | Show or hide each language's sections. | A cookie or query parameter, or client. |
| speech | `SpeakButton.svelte` | missing | Speaks a word in its language. | Client (`speechSynthesis`). |
| save to review | `SaveToStudy.svelte` | missing | Adds a word to the device deck or the account's. | Accounts, or client storage. |
| share | `ShareButton.svelte` | missing | The platform's share sheet, else copy. | Client. |
| keyboard shortcuts | `KeyboardShortcuts.svelte` | missing | `/` to search, `?` for help, `h` home. | Client. |
| sentence bar | `SentenceBar.svelte` | missing | A sentence's words, each opening its entry. | Server-renderable links from the URL. |
| error page | `+error.svelte` | partial | The status and a way home. | Built: the word page's 404. Missing: one page for every error. |
| service worker | `service-worker.ts` | missing | Caches entries and static data for offline use. | Client; a cache policy. |
| web app manifest | `static/manifest.json` | missing | Installable, with icons and shortcuts. | Static files. |
| robots | `static/robots.txt` | missing | Crawlers kept off the dictionary; AI crawlers off all of it. | A static file; whether word pages are indexed once server-rendered is the owner's. |
| WebMCP tools | `webmcp-tools.ts` | missing | Tools offered to an in-browser agent. | Client. |

## Summary

`scripts/kiokun_inventory.py` counts the rows above
(`docs/evidence/E14/kiokun-inventory.txt`):

- **Routes: 102** (49 pages, 53 endpoints). **0 built, 4 partial, 98
  missing.** The partial four are the slice's: `/`, `/[word]`, `/search`
  and `/api/search`'s ranking.
- **Features: 63** (50 of the word page, 13 of the site). **4 built, 11
  partial, 48 missing.** All four built, and ten of the partial, are the
  word page's.
- **Proposed not to port: 9 routes**, dead or developer pages: `/demo`,
  `/test`, `/api/cors-port`, `/api/notes`, `/api/uploads`,
  `/api/tutor/chat`, `/api/tutor/exercise`, `/api/learning-resources/words`,
  `/api/study/searches`. And 3 more unless the iPhone app stays:
  `/api/sync/pull`, `/api/sync/push`, `/api/mobile-auth/complete`. The 98
  missing count them.

## What the gaps need, by kind

1. **Data the host has, rendered on the server** (no new rule): most of the
   word page (readings, Jyutping, tags and labels, examples, mnemonics,
   components, contains, appears in, history, comments, the SEO head), and
   the loader's canonical redirect, variant merge and related forms. Several
   entry reads per page, and Pleris computation over them.
2. **The whole dictionary's search**, from the builder's rule
   (`search_index_builder.rs`) and the current `/api/search`, with the
   alias table; and the exact-reading index. The index is the deployment's
   (ADR-0041 §3).
3. **Pure computation, ported to Pleris**: Japanese and Korean
   deinflection, romaji to kana, SM-2, course and drill grading, false
   friends, the orders of `contains` and `appears in`, pitch's and reels'
   hash rules.
4. **Static data the app ships** (`sveltekit-app/static/`, 
   `src/lib/generated/`, the course modules): frequency, categories,
   homophones, pitch, glosses and taxonomy, sentence corpora, reels, courses,
   blog posts.
5. **Accounts**: Google sign-in, notes, review cards, custom words, saved
   sentences, artifacts, settings; private queries and commands, as the
   feed's are.
6. **Script in the browser**: speech, stroke animation, suggestions as one
   types, handwriting, the game, video sync, device storage, share.
7. **Services outside**: OpenAI (the tutor, lessons), Workers AI
   (translation), Google Input Tools (handwriting), jsDelivr and raw GitHub
   (stroke data, a public exercise dataset), cdn.cosmos.so (videos).
8. **New libraries**: image rendering for link cards, sanitized Markdown,
   text segmentation. Each is a crate the owner approves.

## Proposed order (the integrator rules)

1. **The word page from the entry** (kinds 1 and 3): every section the
   entry alone carries, the loader's redirects and merges, the SEO head.
   It is what real users open most, and it needs no new rule.
2. **Search over the whole dictionary**, held to kiokun.com's current
   `/api/search` re-derived from its TypeScript, Korean included; then the
   reading index, homophones and deinflection.
3. **The static-data pages**: frequency, categories, phonetic series,
   homophones, emoji, the blog, courses as content, the sitemap, robots,
   the manifest, the error page.
4. **Accounts and a user's data**, once the questions below are ruled.
5. **Script in the browser**, as the runtime grows to it.
6. **Services outside**, each with its secret, after the owner's approval.

## Alternatives

- **Sample the live site for the inventory.** Refused: its `robots.txt`
  disallows this agent.
- **Run the SvelteKit app in the checkout.** Refused: it writes into the
  checkout. A copy outside it is asked below.
- **Inventory by what a browser shows rather than by source.** It would
  miss endpoints, dead code and the data each feature reads; the source
  names all of them, with lines.
- **Mark the slice's rows built where it shows more.** Refused: built
  means at parity, and the slice lacks most of what each of those rows
  shows.

## Consequences

- The track's next work is the integrator's order. Nothing beyond the
  registrations (PW60, `KIOKUN_PORTS`) and this inventory's check is built.
- The inventory is held to the app by a recorded command. When the owner
  adds or removes a route, `just e14-kiokun-inventory` fails until this ADR
  says how it stands.

## Acceptance

- `scripts/tests/test_kiokun_inventory.py`, 9 tests, in `just ci`: a route
  is a directory with a page or an endpoint; an inventory naming every route
  once passes; a route left out, a route the app lacks, a route listed
  twice and a row without a status are each found; feature rows are counted
  by status; the script fails on a problem; the recipe is local.
- `scripts/kiokun_inventory_mutations.py`: 9 mutants, each undoing one of
  those, each killed.
- `just e14-kiokun-inventory` (local: it reads the owner's checkout):
  `docs/evidence/E14/kiokun-inventory.txt`, the inventory against the app
  at its commit, OK, and the mutation controls.

## Not claimed

- **No row was measured in a browser.** Each comes from the source.
- **The live site's behaviour** where it differs from the checkout's
  working tree: the owner's uncommitted changes were read, and what is
  deployed may be older.
- **Which migrations the live D1 database has.** Seven hand-written
  migrations sit outside Drizzle's journal; which were applied is not
  known from source. So whether `/learning-resources` has rows is unknown.
- **The iPhone app** (`ios/`), which calls the sync endpoints, is not
  inventoried.

## Questions for the integrator

1. **Rendering.** kiokun.com renders no page on the server (Found 1). I
   propose that the rewrite render every page on the server and work with
   script off, as Pleris does, with script added where a feature needs it.
   Is parity measured by content and behaviour, not by rendering?
2. **The kiokun host.** The slice runs on its own host
   (`spikes/kiokun/server`): GET only, no sessions, no runtime in the
   browser. Accounts, commands and client features are the development
   server's (`pw-dev-server`), with the DataLayer, identity and uploads.
   Does kiokun move onto that host, as a second program beside the feed and
   the store, or does its host grow them?
3. **Static data outside `output_dictionary`.** Pitch, glosses, taxonomy,
   categories, homophones, corpora, reels and the alias table live in the
   checkout's `sveltekit-app/static/` and `src/lib/generated/`, and the
   courses in its TypeScript. May the host read them, read-only, through a
   second variable (`KIOKUN_APP`, the checkout's `sveltekit-app`)? And may
   the courses' data be converted once into files the track commits, or
   must it be read from the checkout too?
4. **The search index.** kiokun.com's is one FTS5 table, rebuilt by the
   builder. Should the rewrite's be the host's in memory (as the slice's is,
   over one shard), a SQLite or PostgreSQL table built from the builder's
   output, or built by the host from the entries by the builder's rule?
   The local `output_search_index.sql` is older than the builder (Found 4).
5. **Running the SvelteKit app here.** May I copy the app outside the
   checkout (a copy-on-write clone, `cp -c`, of `sveltekit-app` with its
   installed packages, into my scratchpad) and run it there against
   `output_dictionary`, for parity measurement? Nothing is downloaded and
   nothing is written into the checkout.
6. **Script in the browser.** Speech, device storage, clipboard, canvas
   and share are not in the platform package. Each is a platform
   operation, a language question.
7. **Services outside.** OpenAI, Workers AI, Google Input Tools, jsDelivr
   and a video CDN. Is there a capability for an outbound request with a
   secret, and is each service the owner's to approve?
8. **What the slice shows more of** (Found 3): keep it, or match
   kiokun.com?
9. **kiokun.com's security findings** (Found 6) are the owner's site's,
   not Pleris's. Will you relay them? The stored script injection on the
   word page is live wherever artifacts are public.
10. **A small correction.** The rulings cite "charter §13.5's split" for
    local-only work; §13.5 is "macOS-specific concerns". The recipe's
    comment cites the ruling instead.
