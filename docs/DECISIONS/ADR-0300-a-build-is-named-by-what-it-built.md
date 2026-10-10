# ADR-0300: a build is named by what it built, and the decision holds a document to what its build says of its page

Status: accepted under the owner's delegation of 2026-10-02, on the
capability matrix's finding (2026-10-08) and ADR-0132's "Not claimed".
Date: 2026-10-09. Milestone: E14. The first of three steps to the parts two
pages share kept in place (NEXT, the DoorDash app's item 1): a page that
keeps its parts across a navigation must know the next document is of its
build.

## Context

- **The resume decision compared constants with themselves.** Every
  document's resume manifest said `"build": "B1"`, `"document": "cart-doc"`
  and `"scope": "public"` (the server's `resume_manifest`), and the browser's
  decision held a document to `cart-doc` and `Public` (`pw-resume-wasm`'s
  `parse`). ADR-0132 made the handler table the build's and left the rest
  "until a page other than the store's needs it": the feed's and kiokun's
  pages do, and a private cart's page said it was public. Only a handler's
  identity was checked.
- **The decision is built to let an old document resume with new code**
  (the deployment matrix, `pw-resume`'s tests): a handler of the same
  identity, a capture schema migrated, a document of the same shape. So the
  document is held to its page's document schema and scope, never to its
  build's name: a deployment that changed nothing a page shows keeps the
  page working.
- **A build had no name.** `BUILD`, `"B1"`, named every build, so nothing
  could tell a document of one build from a document of the next. A
  navigation that keeps the page (the research of 2026-10-09: SvelteKit's
  version header, Next's deployment id, Turbo's tracked head) needs it.

## Decision

1. **`pw build` names the build by what it built**: FNV-1a, 64 bits, the
   resume artifacts' own hash, over every file it writes, by path and bytes
   in the order written, `b` and sixteen hex digits, written last to
   `build-id`. The same sources build the same name; a change to a
   template, a handler or a page's plan changes it.
2. **A page's plan carries its documents' scope** (`scope`, from
   `resume::page_scope`, which reads `manifest_scope`): `public`,
   `session:`, `user:`, `organization:`, or `private:` where a page is cached
   privately and names no principal. Public pages' plans are unchanged.
3. **A host serves the build it was given, by its name**:
   - it reads `build-id` at start, and refuses a build that names none (one
     written before builds were named, built again);
   - each document's resume manifest names it, beside its page's document
     schema (the template's, which closes over each template its instances
     reach, ADR-0203) and its page's scope;
   - each document it answers, and its handler table, carry it as the
     `pw-build` header;
   - **its handler table says each page's document**: `#build|<name>`, and
     `#page|<template>|<schema>|<scope>` for each page, beside the handlers'
     lines, as no handler's identity begins with `#`.
4. **The decision holds a document to what the build that served the
   runtime says of its page**, never to what the document says (ADR-0132's
   rule): the runtime reads its page's `#page` line and tells the decision
   (`know_document`); a document of another schema is refused (6), its
   parts' places another build's, and a capture of a scope that does not
   flow into the page's is refused (5). Each refusal recovers as a stale
   handler does: the page is read again, once, and the press is not
   replayed. Told nothing, the decision compares neither, as no build has
   spoken.

## Acceptance

- **`compiler/pw-core/tests/build_id.rs`**: the same sources build the same
  name, `b` and sixteen hex digits; a template's text and a handler's body
  each change it.
- **The host's tests**: its build is served by its name on the table and on
  a document, the document's manifest names it with the home page's schema
  and `session:`, the table says the page; a build that names none is not
  served.
- **`pw-resume-wasm`'s test**: a table's `#` lines are no handlers; told a
  public page of schema `s1`, a document of `s2` is refused (6) and a
  session's capture (5); told a session page, a session's and a public
  document resume and a user's does not.
- **Browser, three engines** (`e2e/recovery.spec.mjs`): the runtime is told
  its page's documents by the table; a document of another schema is
  refused at its first press, read again once, and its next press works.
- **`scripts/build_id_mutations.py`**, recorded by `just e14-build-id`.

## Found

- **A runtime-recovery mutant survived on CI** (verify 37985418508, `just
  e14-runtime-recovery`): "the recovery codes are read one place off"
  (ADR-0155). The stale handler's test killed it only while every page was
  planned public: a public page's region is refetched, `refetch-region`,
  which the list read one place off made `none`, and no press acts on
  `none`. With the page's real scope, a session's, its recovery is
  `rerender-private-slot`, read one place off as `refetch-region`, and both
  read the page again. The test, and this ADR's own for a document of another
  schema, now hold the decision's recovery to its name, from the log the
  runtime writes as it binds the press; 3 of 3 mutants killed locally.

## Not claimed

- **Entries' generation is still the host's constant** (`BUILD`): entries
  named by the build's name wait for the store's entries keyed by a user
  (W8's track), which change the same functions.
- **The static store page** `run.sh` renders (`pw-render`'s own output,
  which no host serves: the development server renders `/StorePage.html`
  itself) keeps `store-resume.json`'s constants.
- **A session's own scope**: `session:` is a page's class, not which
  session; a session's id is its cookie, which the runtime does not read.
- **A page open across a deployment** keeps its table and its document, both
  of the old build, and its presses reach the new host: that skew is a
  deployment's (E8), and the next step, a navigation that keeps the page,
  reads the new build's name before it keeps anything.

## Alternatives

- **The decision holds a document to the build's name**: every deployment
  would read every open page again at its next press, where the decision is
  built to let a page whose code and shape are unchanged go on.
- **A content hash of the templates alone**: a changed handler or plan
  would keep the name, and a navigation would keep a page whose code moved.
- **A timestamp or a counter**: the same sources would build two names, and
  nothing could say two builds are one.
