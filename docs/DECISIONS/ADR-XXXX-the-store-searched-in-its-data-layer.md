# ADR-XXXX: the store's stores and dishes, searched in its data layer to one meaning

Status: proposed by track `store-search` (W9, `track/store-search`), under
the integrator's rulings of 2026-10-11 (docs/PARALLEL.md, "W9: the DoorDash
customer side, search now, store hours after W8", and its correction of the
same day, "no search form before the route reads its query part"). Date:
2026-10-11. Milestone: E14, the app layer (NEXT, the DoorDash customer app).
Builds on ADR-0218 (the data layer's seam), ADR-0298 (the store on
PostgreSQL), ADR-0246 (the feed on PostgreSQL), ADR-0303 (layouts) and
ADR-0207 (a source states what it guarantees).

## Context

- **A delivery app starts from a search.** The store's program had a list
  of stores (ADR-0192) and each store's menu, and no way to find a store or
  a dish by what it is called. NEXT names it: "stores and dishes, by name
  and cuisine, run in the database or a declared search source".
- **The ruling's rules** (docs/PARALLEL.md, W9): the search runs in the
  store's data layer, as its operation, in memory and on PostgreSQL, to one
  stated meaning tested on the same fixtures; case is ignored, every term
  must match, results rank by a stated rule; no input fails the page; the
  catalogue is public; the page sits in `StoreLayout`.
- **The address.** A search's text belongs in the query string, a typed
  page query parameter the integrator builds next. Until then the page is
  at `route "/search/{q}"`, reached by its address. It has no search box:
  `navigate` follows only a committed command (ADR-0280), and a search
  commits nothing (the ruling's correction, W6's finding).
- **A store has no cuisine.** Neither `domain.pw`'s `Store` nor the store's
  rows (`StoreRow`, the `stores` table) hold one. W8 changes both until it
  merges (a store's delivery zone, `0004_addresses.sql`).
- **PostgreSQL's case is a collation's.** `lower(text)` follows the
  database's `LC_CTYPE`: under `C` it lowers ASCII alone, and under a libc
  locale it is the platform's (macOS's and glibc's differ).

## Research

- **DoorDash's search** (read 2026-10-11):
  - DoorDash's engineering post on its in-house search engine
    (careersatdoordash.com, 2024-02-27; its pages refused this session's
    fetches with 403, so read through ByteByteGo's summary, "How DoorDash's
    In-House Search Engine Achieved a 50% Drop in Latency", and search
    excerpts of the post): stores, items and categories are separate
    document types ("namespaces"), and an item is the child of its store
    in the index, joined to it, so that an item is not shown when its
    store is closed. A query planning and understanding service rewrites
    the query (misspellings, synonyms, location) before the index is
    searched; geo-radius is a filter, and ranking is "geo-personalized".
  - "How DoorDash leverages LLMs for better search retrieval"
    (careersatdoordash.com, read through search excerpts): a query is
    segmented against a controlled vocabulary of DoorDash's own taxonomy,
    and its vertical intent (a restaurant or food item, or a retail or
    grocery item) predicted before retrieval.
  - Its web search's address is a path segment,
    `doordash.com/search/store/{query}/`, whose results depend on the
    delivery address set (third-party scrapers' documentation, Apify's
    `dz_omar/doordash-scraper`; DoorDash documents none).
  - So: **stores and dishes are both results, and a dish is shown under
    its store**, which is where it is ordered. The rest (query
    understanding, synonyms, misspellings, personalised and distance
    ranking, filters) is DoorDash's machinery, and none of it is claimed
    here.
- **PostgreSQL 18** (postgresql.org/docs/18, read 2026-10-11):
  - §23.2.2.1, standard collations: `pg_c_utf8` "sorts by Unicode code
    point values ... For the functions `lower`, `initcap`, and `upper`, it
    uses Unicode simple case mapping ... only available for encoding
    `UTF8`." `pg_unicode_fast` uses full case mapping.
  - §9.7.1, `LIKE`: `_` matches any single character and `%` any
    sequence; "the default escape character is the backslash but a
    different one can be selected by using the ESCAPE clause".
  - `unicode_version()` answers the Unicode version of the server's
    tables: `16.0` on 18.6, here (Homebrew) and on CI (`postgres:18.6`).
- **Rust 1.97.1**: `char::to_lowercase` is the full mapping (SpecialCasing's
  unconditional entries; U+0130 alone lowers to two characters), and
  `char::UNICODE_VERSION` is `(17, 0, 0)`.
- **No crate and no extension.** Accent and stem folding would need one on
  either side (`unaccent`, a stemmer), or the two layers would differ.

## Decision

### 1. The meaning

- **The terms** are the text's words, split at Unicode's white space
  (`str::split_whitespace`), each lowered, once, in `search.rs`, whichever
  layer matches them.
- **Lowered** is Unicode 16.0's simple lowercase mapping, a character at a
  time: in memory, `char::to_lowercase`'s first character (the simple
  mapping of U+0130, `i`), keeping the 28 capitals Unicode 17.0 added (three
  Latin letters and Beria Erfe's, `NEWER_THAN_POSTGRES`), which PostgreSQL
  18's tables leave alone; on PostgreSQL, `lower(.. COLLATE pg_c_utf8)`,
  whatever the database's default collation. No final sigma, no `ß` as
  `ss`, no accent and no stem folded.
- **A record matches** where every term is part (a substring) of one of
  its fields: a store's name, cuisine or description; a dish's name,
  category or description.
- **A store is listed** where it matches, or holds a dish that does; its
  dishes listed under it are those that match.
- **Its tier** says where its terms were found: 0, every term in its name;
  1, each in its name or its second field (a store's cuisine, a dish's
  category); 2, each in any of its three.
- **The order.** A store that matched ranks by its tier; one listed for its
  dishes alone ranks after every store that matched, by 3 and its best
  dish's tier. Ties by name, then id, each compared code point by code
  point (Rust's `str` order). A store's dishes by tier, then name, then id.
- **The cut.** The first 20 stores are listed, each with its first 5
  dishes; how many more of each are counted and said.
- **The bound.** A text of more than 100 characters (Unicode scalar values,
  as given, before it is trimmed) is the declared error `TooLong`; one with
  no term is `Blank`.

### 2. Where it runs

- **Each layer answers what matched, with its tier**, through
  `Rows::search(terms)`: in memory, its default, `search::scan`, over the
  layer's stores and menus; on PostgreSQL, `PgRows`' own, `search::
  on_postgres`, two statements: the dishes that match, then the stores
  that match or hold one. Each term is a `LIKE` pattern, `%`, `_` and `\`
  escaped (`search::pattern`) with `ESCAPE '\'`, the patterns bound as one
  `text[]` parameter, never written into the statement.
- **What follows is `search.rs`'s alone**: the order, the cut and the
  program's value (`search::found`), so the two layers cannot rank
  differently.
- **The operation** is `store:data/search#find(text)`, registered beside
  the catalogue's (`store.rs`'s `catalog`, so a query's reads and a
  command's both hold it), under `database.read<Stores>` and
  `database.read<Menus>`, which a node already grants. It answers the
  program's `Result<SearchFound, SearchError>`: `blank` and `too-long`
  read no rows.
- **A store's cuisine is search's own**: `search::cuisine_of` in memory,
  the table `store_cuisines` on PostgreSQL (`migrations/store/0010_search.sql`,
  the store's first number after W8's), the same rows: 47 "Coffee", 48
  "Café, Bakery". The stores' own rows are left to W8 until it merges.

### 3. The program (`examples/store/search.pw`)

- `fn find(text) -> Result<SearchFound, SearchError>`, `host
  "store:data/search#find"`.
- **`public query Search(q) -> SearchShown`**: shared, keyed by its text,
  fresh for 30 seconds, and dropped by any store's `MenuChanged` or
  `InventoryChanged` (`invalidates_on MenuChanged(_), InventoryChanged(_,
  _)`). It says what it found in a sentence ("2 stores match “coffee”.",
  "No stores or dishes match “x”.", "Type a store, a dish or a cuisine to
  search for.", "That search is too long: 100 characters at most."), and
  the counts past the cut in words.
- **`session page SearchPage(q)`**, `route "/search/{q}"` (TEMPORARY),
  `layout StoreLayout`: its title "Search: {q}", `noindex, follow`; the
  sentence as a status; each store by its name, linked to its page, its
  cuisine and description; its dishes as a list named "Dishes at {store}",
  each linked to the store's page, with its category, description and
  price. Session, as `StoreLayout` is (PW5046); the catalogue it reads is
  public.
- **When the query part lands**, the route clause becomes `/search{?q}`
  and the home page gains `<form method="get" action="/search">` with
  `name="q"`; `q` stays the page's parameter.

## Found

- **A menu change told no page but the store's** (a host fix, found by W9,
  its own commit, `125f9c43`, for the integrator's review at the merge).
  The menu's route (`broadcast_menu`, E7-P's `/command/menu`) dropped what
  `MenuChanged` reaches, the kept search among it, and discarded the list:
  an open search page never saw a dish added (the spec's first run, three
  engines). A command's drops are queued for telling (ADR-0219); the
  route's were not. Now what it drops that the store's page does not read
  is queued too, at a line marked `TRACK SEAM (store-search), found by W9,
  a host fix`.
  - **Shown by** `tests/store_search.rs`'s
    `an_open_search_page_is_told_of_a_menu_change` (a search page of
    session `a` sent the dish found when session `b` adds it), the spec's
    "a dish added while the page is open reaches it" in three engines, and
    the script's mutant "a menu change's route tells no page reading a
    search".
  - **Two mutants re-anchored**: `menu_changed_mutations.py`'s "a menu
    change drops every store's menu, by the query's name" and "a menu
    change drops nothing", on the closure's first line, which now keeps
    what it dropped for telling.
  - **Why the store's own page is left out.** It is sent the menu's
    patches by `broadcast_menu` itself, the dish inserted in its category's
    list, so a telling would read it a second time for nothing.
    `a_menu_change_tells_the_stores_page_once` shows the dish's row reaches
    an open store page exactly once. Measured here, though: with the
    exclusion taken away the row still arrives once (the telling found
    nothing left to send), so the exclusion saves a second reading of
    every open store page per change; it does not prevent a double
    insertion, and no mutant is planted on it.
- **Rust's Unicode is newer than PostgreSQL's**: Rust 1.97.1's tables are
  Unicode 17.0 (`char::UNICODE_VERSION`), PostgreSQL 18.6's 16.0
  (`unicode_version()`). Of every scalar value but NUL, 28 lower in Rust
  and not in PostgreSQL: U+A7CE, U+A7D2 and U+A7D4 (Latin capitals added in
  17.0) and U+16EA0 to U+16EB8 (Beria Erfe's capitals), each listed by
  `on_postgres_every_character_is_lowered_alike` when the hold is taken
  away. **Held back** means neither layer lowers them: in a query or in a
  name, each is matched as written, on both layers alike, so its capital
  finds only its capital. Nothing is refused. The test fails when either
  side's Unicode version moves, which is when the hold is looked at again.

## Alternatives

- **Match in Rust on both layers** (the PostgreSQL layer reading every
  store and menu, `Rows::stores` and `Rows::menu`, and matching as memory
  does). One meaning by construction, but the database would match
  nothing: not a search "in the database", and every search a read of the
  whole catalogue. Refused for the ruling's `LIKE`.
- **ILIKE, or `lower()` under the database's collation.** Each follows the
  database's `LC_CTYPE`, so the meaning would be the deployment's. Refused
  for `pg_c_utf8`, which holds Unicode's simple mapping in any UTF-8
  database.
- **Full case mapping** (`pg_unicode_fast`, Rust's `str::to_lowercase`).
  Rust's applies the final sigma by context and PostgreSQL's would have to
  be shown to; simple mapping keeps a field's length and needs no context.
- **Full-text search** (`tsvector`, SQLite's FTS5 as W6's kiokun uses):
  stems and dictionaries the in-memory layer cannot match without a crate.
- **A store's cuisine in `StoreRow` and the `stores` table**, and in
  `domain.pw`'s `Store`. The right home once W8 merges (a question below),
  but `StoreRow`, `stores` and `Store` are W8's to change until then.
- **A dish matched by its store's fields too** ("harbor scone" finding
  Harbor Coffee's scone). It lists every dish of a store whose name
  matches, unless a rule says which terms a dish's own fields must hold.
  Not now: each record is matched by its own fields, and "harbor scone"
  finds nothing, stated below.
- **A search form now**, by `navigate` or a placeholder command: refused
  by the ruling's correction.

## Acceptance

- **The meaning, on both layers** (`search.rs`'s tests): one fixture of six
  stores (names with accents, Greek, a sharp s, a dotted capital I, `%`,
  `_`, `\` and quotes, two stores of one name, a store with seven dishes
  in reverse order) and one table of 23 searches, each answered alike in
  memory and on PostgreSQL: blank; `%`, `_`, `\` and quotes; mixed case,
  non-ASCII, several terms, and none matching; the order and the ties.
  Beside it: each tier; a dish's tier by field; stores and dishes ordered;
  the bound in characters; the cut and the counts; the operation's
  declared cases in memory; the seed searched in memory and through
  PostgreSQL's layer, a cuisine changed in the database found there; the
  migration's cuisines memory's.
- **Every character lowered alike**: each of the 1,114,111 scalar values
  but NUL lowered by PostgreSQL and by `search::lower`, none differing;
  without the hold, 28 differ (Unicode 16.0 against 17.0); each character
  held is one Rust's tables lower.
- **The page served** (`tests/store_search.rs`), in memory and on
  PostgreSQL: the stores and dishes in their order, linked, in the layout;
  case and accents; no text failing the page (blank, white space, `%`, `_`,
  `\`, quotes, markup, past the bound and at it); a menu's change reaching
  a search kept since; an open search page told of it, and an open
  store page told of it once.
- **The page in three engines** (`e2e/store-search.spec.mjs`, on its own
  hosts, `STORE_SEARCH_PORTS`): the list and a dish's link to its store;
  case, accents and several terms; no text failing the page; a dish added
  while the page is open reaching it; the audit's rules, found, empty and
  blank.
- **Mutation controls** (`scripts/store_search_mutations.py`, 30 mutants,
  recorded by `just e14-store-search`; numbers in the report below).

## Not claimed

- **No search box.** The page is reached by its address until a route
  reads its query part.
- **No accent or stem folding**, no synonyms, misspellings, or query
  understanding: "crepe" does not find "Crêpe", "strasse" does not find
  "Straße". A KNOWN_LIMITATIONS item (merge notes).
- **A record is matched by its own fields.** A term in a store and another
  in its dish ("harbor scone") find nothing.
- **No ranking by distance, rating, popularity or what the reader
  ordered**; no filter by the reader's address until W8's `Reach` (below).
- **No paging.** Past the cut a count is said; the rest are not shown.
- **One Unicode version.** The meaning is Unicode 16.0's simple mapping,
  held to PostgreSQL 18's tables by a test that fails when either side's
  tables move.
- **Scale.** Both statements read every row of `menu_items` and `stores`;
  `LIKE '%term%'` uses no index. A trigram index (`pg_trgm`) is an
  extension, not added.
- **The cuisine is seed data.** Nothing changes it; in memory it is a
  constant table, as the stores themselves are.

## Questions for the integrator

1. **A store's cuisine, after W8 merges**: may it move into `StoreRow`, the
   `stores` table and `domain.pw`'s `Store` (shown on the home page and the
   store's page too), and `store_cuisines` be dropped by a later
   migration? Until then it is search's own table.
2. **The seams.** The ruling allowed one marked line where each layer
   registers operations. `store.rs` has two (the `Rows::search` default,
   four lines under one marker, and the registration), `store_pg.rs` two
   (`PgRows`' override and the migration's entry), `main.rs` three (`mod
   search`, the test module `tests::store_search`, and the menu route's
   telling, Found above), each marked `TRACK SEAM (store-search)`. Also
   changed, each as small: the PW63 block in `codes.rs` (no code registered), `just/store-search.just`'s import in
   the justfile, `e14-store-search` in `ci_plan.py`'s `NEEDS_DATABASE`,
   `STORE_SEARCH_PORTS` and their hosts in `playwright.config.mjs`, and four
   values in `store-values.json` (the static render reads every page's).
3. **`ESCAPE '\'` is PostgreSQL's default.** Written as the ruling asks, it
   changes nothing; a mutant dropping it would survive, so the script has
   none.

## Merge notes

- **KNOWN_LIMITATIONS** (the integrator's ruling of 2026-10-11): the
  store's search folds no accent and no stem, and matches each record by
  its own fields. Its meaning is Unicode 16.0's simple lowercase mapping
  (PostgreSQL 18.6's tables) where Rust 1.97's are 17.0's: the 28 capitals
  17.0 added (U+A7CE, U+A7D2, U+A7D4, U+16EA0 to U+16EB8) are matched as
  written on both layers, never lowered;
  `on_postgres_every_character_is_lowered_alike` fails when either
  version moves.
- **`menu_changed_mutations.py`**: its two mutants of the menu route's
  drop are re-anchored on the closure's first line, which now keeps what
  it dropped for telling.
- **Regenerated with the program**: the store's IR (`run.sh`), the resource
  graph (`just materialize`: `Search` invalidated by `MenuChanged(_)` and
  `InventoryChanged(_, _)`, `SearchPage` reading `Search(q)` and its
  layout's cart), the WIT (`just e8-wit`: `search#find` among the store's
  imports, 31 worlds). W8's branch regenerates the same files: regenerate
  them again after both.
- **After W8 merges** (W9, in its own commits): the search page becomes
  `user`, as `StoreLayout` will be; the home page gets its search form once
  the query part lands (one marked line); results may be filtered to the
  stores that reach the reader's address, through W8's `Reach`. The
  fixture's `INSERT INTO stores` relies on W8's new columns' defaults.
- **Migration 0010** is the store's next after W8's 0003-0009, renumbered
  at the merge if W8 needs more.
- **STATUS / NEXT**: the search's next steps are the query part's route
  and form (the integrator's), the cuisine's home (question 1), and
  filtering by `Reach`.

## Report

- **Built**: `examples/store/search.pw` (the query, the page, its types);
  `spikes/own-renderer/server/src/search.rs` (the meaning, both layers'
  matching, the order, the cut, the operation); migration 0010; the seams
  listed in question 2; `tests/store_search.rs`;
  `e2e/store-search.spec.mjs`; `just/store-search.just`
  (`e14-store-search`); `scripts/store_search_mutations.py`.
- **Tests, local** (this Mac, PostgreSQL 18.6, rebased on `839a289b`):
  `search.rs`'s 13 tests and `tests/store_search.rs`'s 6, green in memory
  and on PostgreSQL; the server's whole suite in memory, 409 passed; the
  spec in three engines, 15 of 15; `pw-core`'s build, data-source, title
  and evidence tests after the counts moved.
- **Mutants**: 30 planted; the record is CI's verify (below).
- **Verify**: VERIFY-PLACEHOLDER.
- **Not claimed**: as above.
