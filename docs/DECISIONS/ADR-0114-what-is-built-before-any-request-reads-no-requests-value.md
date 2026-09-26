# ADR-0114: what is built before any request reads no request's value

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §9.3).

## Context

Charter §9.3 derives static generation from "public, deterministic,
build-known". `placement build` makes a declaration's output a file produced
before any request exists, and served as it is to every reader (`reuse_of`,
`Reuse::Build`). A-013 is such a page, and it takes no parameter.

A parameter is supplied by a request. On 2026-09-26, at 7b2689f, this
checked:

```pleris
page ShopInfo(id: StoreId) {
    placement build
    view { <main><h1>Store {id}</h1></main> }
}
```

When the file is built there is no `id`, and no one value could be right for
every reader of the one file.

## Decision

**What is built before any request reads no request's value** (PW5026). A
declaration placed at `build` does not read its parameters. Each parameter
read is reported once, found by lexical binding. A build-placed page that
takes a parameter and never reads it is not refused: one file can serve
every value.

**(ruling needed)** Whether a build-placed page may take a parameter at all,
and whether a build may enumerate a parameter's values, as some frameworks'
static generation does. Neither exists in the language.

## Acceptance

- **`compiler/pw-core/tests/built_pages.rs`**, 1 test, failing at 7b2689f:
  a build-placed page rendering its `id`, reported once for two reads. Its
  controls are the same page not reading it, and one placed at the origin.
- **Corpus.** No fixture's diagnostics change. A-013, the corpus's
  build-placed page, takes no parameter. The store, kiokun and the accepted
  corpus check clean.
- **Mutation controls:** `scripts/built_page_mutations.py`,
  `just e10-built-pages`, 3 mutants.
