# ADR-0037: the kiokun slice — a second application, on one real shard

Status: accepted under the owner's instruction of 2026-09-24 ("build a small
kiokun slice as a proof app, such as entry lookup plus search over one shard").
Decisions marked **(ruling needed)** were made without a ruling and are offered
for reversal. Date: 2026-09-25. Milestone: E10.

## Context

Every compiled artifact so far came from one program, the store demo. A
language that has compiled one program has not shown it is general, and the
owner's question was whether perfect-web could rewrite kiokun.com: a
Chinese–Japanese dictionary with 1.49 million entries in 28 shards, searched on
the server.

## Decision

### 1. The slice

`examples/kiokun/` is a second program with four modules:

| module | what it holds |
|---|---|
| `dictionary` | types: `Entry`, `ChineseWord`, `JapaneseWord`, `JapaneseSense`, `SearchHit` |
| `Entries`, `Index` | the data layer, supplied by the deployment (`host` bindings) |
| `kiokun.page` | `Lookup`, `Search`, and three pages |

`Lookup` follows one redirect in Pleris, which kiokun.com's `+page.ts` does in
TypeScript. `Search` asks the index for twenty hits. `pw build` compiles the
program to two audited components and three templates, and the build is
committed as `docs/evidence/E10/kiokun/`, held byte for byte by
`evidence_is_current`.

### 2. The data layer is the deployment's, as it is on kiokun.com

kiokun.com matches and ranks in SQLite FTS5 on its server. The slice keeps that
split: `spikes/kiokun/server` supplies `kiokun:data/entries#get` and
`kiokun:data/index#search` over one shard, in memory. The ranking reimplements
kiokun's `/api/search` over this shard. It does not claim parity with FTS5's
tokenizer or with kiokun's full alias table.

### 3. The data is kiokun's, in kiokun's format (ruling needed)

`examples/kiokun/data/han-1char-3/` holds 28 files copied byte for byte from
kiokun's build output by `scripts/kiokun_sample.py`, with a SHA-256 manifest.
The host reads kiokun's raw-DEFLATE layout with kiokun's shard rule, so the same
loader serves the sample and the whole shard (`KIOKUN_DATA`).

The content derives from CC-CEDICT and JMdict (CC BY-SA 4.0) and Tatoeba
(CC BY 2.0 FR). `examples/kiokun/data/NOTICE.md` attributes it and scopes those
licences to these files. **Owner decision needed:** whether share-alike data may
live in this repository. The alternative is to keep only the script and fetch
the sample.

`miniz_oxide` 0.9.1 is added to the slice's host for the DEFLATE, verified on
crates.io (MIT OR Zlib OR Apache-2.0).

### 4. Pages are static, and `Option` stays out of templates

The template language has `{#if}` and `{#each}`, no else, and no option. A
missing value blocks a render. So the host renders `EntryPage(entry)` for
`Some` and answers 404 with `NotFound(word)` for `None`, as kiokun.com's
SvelteKit app uses its error page. The three pages carry no script, and they
work with JavaScript disabled.

### 5. Lists are keyed by the source's own identity

`#each` over data that can change requires a key (PW5011), and the checker is
right to require one. Keying a list of definitions by their text would refuse
to render as soon as two senses shared a gloss, and JMdict's do. So each
Chinese word is keyed by CC-CEDICT's `_id`, each Japanese word by JMdict's `id`,
and each sense by `<id>:<index>`.

## Findings, each fixed or refused

1. **The platform package depended on the store example.**
   `packages/pw-platform-web/device.pw` imported `domain.{ Location }` from
   `examples/domain.pw`. Every program using the platform had to include the
   store's domain, and the kiokun program was the first that did not.
   `Location` is now declared in `device.pw`, and the store, the generality
   fixtures and `Estimates` import it from there. The platform package now
   checks alone (17 files). The trusted-contract hash changed, and that test
   comment says why.
2. **The template IR wrote interpolated attribute strings out verbatim.**
   `href="/{hit.target}"` rendered as `href="/{hit.target}"`. The route
   checker reads such strings as links to declared routes, but the template IR
   has no part that renders one. It now emits a `Blocked` part naming the
   attribute and the supported form (`href={value}`), so the render refuses
   instead of linking to a literal `{…}`. Interpolated attributes need a new
   part kind, in the IR, the renderer and the runtime's patching, and are not
   built.
3. **The checker passed a match with a missing case.** A match over a call,
   or over `Option` or `Result`, was never analysed for exhaustiveness,
   although ADR-0011 requires `pw` to reject an incomplete match. The
   backend's refusal of one while compiling `Lookup` found it. The checker
   now refuses it with PW0305, and fixing it found four more ways a match was
   proven exhaustive when it was not
   ([ADR-0038](ADR-0038-every-match-typed.md)).
4. **The first search design grouped by entry and language.** That gave two
   hits for `人`, ordered by whichever Japanese row came first. Now there is
   one hit per entry, the best row by score and then commonness, with its
   languages combined, as kiokun groups them.

## Acceptance

- `spikes/kiokun/server` tests:
  - kiokun's shard rule against kiokun's own layout;
  - the sample is exactly one shard;
  - the compiled `Lookup` follows the shard's real redirects;
  - search ranks by kiokun's rules;
  - the pages render with no script.
- **The whole shard.** 17,597 entries. 16,921 were looked up and rendered by
  the compiled `Lookup` and `EntryPage`. All 103 in-shard redirects were
  followed. 676 stubs name another shard, and each correctly returned `None`.
  0 failures.
- `e2e/kiokun.spec.mjs`, in Chromium, Firefox and WebKit, 15/15:
  - lookup, redirect and 404;
  - search through the form, then following a result;
  - the same with JavaScript disabled.
- Evidence: [kiokun-2026-09-25.md](../evidence/E10/kiokun-2026-09-25.md).

## Consequences

The answer to "could it rewrite kiokun.com" is more specific now; see the
evidence file's list of what the slice did not need and what the full site
would.

## Amendment, 2026-09-25: Korean words, names and the character

Later the same day the slice's `Entry` gained three things kiokun's entries
carry and its page did not show:
- `korean`, from `korean_words`: hangul, hanja, pronunciation and
  definitions;
- `names`, from `japanese_names` (JMnedict): written forms, readings, kinds
  and romanizations;
- `character: Option<Character>`, from `chinese_char`, `japanese_char` and
  `korean_char`. It holds strokes, school grade, the former JLPT level and
  frequency, each an `Option<Int>`, and the hanja table's hangul readings and
  meanings.

`WordPage` renders them with `{#if}`, `{#each}` and nested `{#match}`
(ADR-0042). The compiled `Lookup` carries the larger record unchanged. All
17,597 entries of the whole shard render.

Found while doing it: kiokun's build lists some records twice in one entry.
JMnedict's name 5345360 is in 釋 under both 釈 and 釋. The renderer refused
the page, because two items shared a loop key. The host now reads each record
once: an exact repeat is dropped, and a different record with a repeated id
is keyed `<id>:<n>`.

**Not done: Korean search.** kiokun.com's search index has Korean rows
(164,941 of 834,036). They are keyed by hangul, with a romanized reading
from its own `hangul_to_romanization`. The slice's index and ranking cover
Chinese and Japanese. Adding Korean faithfully needs that romanization and
kiokun.com's ranking for hangul queries ported and held to references, as
ADR-0041 did for the rest. Pitch accent is not in kiokun's entries.
