# ADR-0186: a page states its description

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. Found by measuring the store's page with Lighthouse after
ADR-0182–0185 closed the audit's gaps.

## Context

- **What Lighthouse found.** Lighthouse 13.4.1, run through the Chrome
  DevTools MCP server on the development server's store page (snapshot
  mode, desktop): Accessibility 100, Best Practices 100, SEO 75. One audit
  failed, `meta-description`: "Document does not have a meta description".
  This is research context, not evidence: no recorded command produced it.
- **A page could not say what it is to what reads it without showing it.**
  A search engine's result shows a page's description under its title. A
  link's preview, in a chat or a feed, reads Open Graph's `og:title`,
  `og:description` and `og:image`. The language had `<title>` (ADR-0183)
  and nothing else for the head. The host wrote a charset and a viewport
  (ADR-0182).
- **What HTML says** (WHATWG HTML, "Standard metadata names" and the `meta`
  element):
  - **A description is "a free-form string that describes the page"**, fit
    for "a directory of pages, e.g. in a search engine". A document has at
    most one.
  - **Limits on other names.** `color-scheme` is once a document,
    `application-name` once a language and `theme-color` once a media query.
    Every other name may repeat.
  - **Names are case-insensitive**, compared ignoring ASCII case.
  - **Exactly one of `name`, `http-equiv`, `charset` and `itemprop`.** With
    `name` there must be a `content`.
  - **`media` has an effect only on `theme-color`.**
  - **A `<meta itemprop>` is microdata.** It is a property of an item on
    the page, written in the body where the item is, not the page's
    metadata.
- **What Open Graph says** (ogp.me): its properties are written with
  `property`, `<meta property="og:title" content="…" />`. A property may
  repeat, `og:image` for several images, and "the first tag (from top to
  bottom) is given preference during conflicts".
- **What Lighthouse reads** (its `meta-description` audit's source): a
  description that is missing, or whose text is only whitespace, fails
  ("Description text is empty.").
- **How others do it.**
  - **React 19 moves a `<meta>` rendered anywhere into the head**, except
    one with `itemProp`, which "doesn't represent metadata about the
    document but rather metadata about a specific part of the page" and is
    rendered where it is.
  - **Next.js has an object or a function beside the page**: `metadata`, or
    `generateMetadata` with the route's parameters (docs at 16.3.8). It runs
    only on the server, so the metadata is in the first HTML. A layout's and
    a page's are merged shallowly, the later replacing the earlier. Next.js
    14 moved `themeColor` and `colorScheme` out of it, into the viewport's
    configuration.
  - **Svelte has `<svelte:head>`.** Astro and Marko write the document's
    head in the page's template.

## Decision

1. **A page states its metadata with `<meta>` at the top of its view**,
   beside its `<title>`, written as HTML writes it:
   `<meta name="description" content={store.description} />`, or
   `<meta property="og:title" content="{store.name}" />`.
   - **The host writes it into the document's head**, after the title, from
     the page's values: the store's page, a page that binds no query, and
     `pw-render`'s static document. It renders nothing in the body.
   - **It is written as the page is served, and set again by nothing.** What
     reads it reads the served document. A change on the page leaves it as
     it was, as a browser does.
   - **It is a part of the page's head**, numbered after the title, which is
     after every part of the body. Writing it moves no other part's number.
     ADR-0183 numbered the title after every part. It is now after every part
     of the body and before the metadata. So writing a title moves no part of
     the body, and the title, which a host sets again, keeps its number when
     metadata is written.
   - **It is optional.** A title is required (ADR-0183, WCAG 2.4.2). A
     description is not an accessibility requirement, and a page a session
     decides is not one a search engine reads.
2. **PW5034: a page's metadata is the page's, at the top of its view: named
   once as text, its content text and values, and a name HTML allows once
   stated once.** Refused:
   - **a `<meta>` in a view**, which would describe every page that
     composes it;
   - **one inside an element or a block**;
   - **the host's own**: a `charset`, an `http-equiv`, or
     `name="viewport"` in any case (ADR-0182);
   - **one that names nothing**, or names it by a computed value, or by both
     `name` and `property`;
   - **one with any other attribute**, `media` or `lang`, which the host does
     not write. The head writes a name and a content, and an attribute
     written and not served would be lost without a word;
   - **one with no content**, or one of only whitespace;
   - **one whose content is computed in place**: it is text, and the
     values the page reads, `content="{store.name}: its menu"`;
   - **a second of a name HTML allows once**, compared ignoring ASCII case:
     `description` and `color-scheme`, and `application-name` and
     `theme-color`, which with no `lang` or `media` written are once too.
3. **A `<meta itemprop>` is an item's property**, as HTML and React 19 read
   it. It may be written anywhere, in a page or a view, and is written in
   the body where it is. One that also names the page's metadata is
   refused: HTML allows one of `name`, `http-equiv`, `charset` and
   `itemprop`.
4. **What it reads.**
   - **A signal is refused** where the page's parts are planned, by
     ADR-0137's rule: the browser writes no metadata again.
   - **A value a press speculates is not refused**, unlike a title's
     (ADR-0183). Nothing sets metadata again, so there is no moment at which
     it says one thing and the page another.
   - **A member function is refused** where the page is planned, as a
     title's is: a host computes one in a text part of the page's body, or
     in a loop's row (ADR-0125, ADR-0169).
   - **A value that is HTML is refused** when it is rendered. Metadata is
     text.
5. **The store describes itself**: `<meta name="description"
   content={store.description} />`, from the description its query gives.
6. **Corpus C11.**
   - **New fixtures**: R-052, a `<meta>` in a view; A-027, a page's
     description and Open Graph title from the values it reads. Two
     categories are added, one for each.
   - **Generality witnesses**: a GENERAL one, a page that states its
     description twice, and a NEIGHBOUR one, a description and two Open Graph
     images after `<main>`. Generality is 37 / 37.

## A correction to ADR-0183

**A static page that stated its title shipped the browser runtime.** Charter
§14 M7 gate 2 says a static route ships no browser runtime. `pw-render`
decided that by whether a page had any part, and a title is a part. So once
ADR-0183 made a page served at a route state a title, every such page with
nothing else dynamic got the parts manifest and the runtime. The runtime
had nothing to do there.

No test caught it. The two static pages `run.sh` renders are views, which
state no title, and only the store is rendered with the runtime offered.

The binary now ships the runtime only when a part is in the body. A title
and metadata are parts of the head, written as the page is served. Where
the body has a part, the title is among the parts the runtime keeps
current. A test renders both pages, and a mutant holds it.

## Alternatives

- **A function beside the page**, as Next.js's `generateMetadata`. It reads
  the same values the view reads, and the language's view already says what
  the page shows from them. A second place would be one more thing to keep
  in step, and HTML's own element is what a reader of the markup knows.
- **Require a description of every page served at a route**, as PW5029
  requires a title. A title is WCAG's (2.4.2, level A), and a screen reader
  says it first. A description is read by a search engine, which reads public
  pages. The store's page is private, as the cart it shows is a session's,
  and a cart page or an order's status has nothing to say to a search
  engine. Lighthouse's audit is about search results, not about whether a
  page works.
- **Refuse a `name` stated twice, whatever it is.** Two `keywords` and
  `theme-color` per media query are valid HTML, and Open Graph's
  `og:image` repeats on purpose. A rule stricter than HTML refuses programs
  that are right.
- **Carry `media` and `lang`**, so `theme-color` can differ for light and
  dark. That is the next step if a page needs it. Until then, writing one is
  refused, rather than served without it.
- **Set metadata again when what it reads changes**, as a title is. A
  crawler and a link's preview read the document as served, and a browser
  does not read a description again. Setting it would cost a part's patch
  and say nothing to anyone.
- **Write metadata in a view**, merged into the page as Next.js merges a
  layout's. A view describing every page that composes it is the defect
  PW5034 refuses. A shared default belongs to the host, as the charset
  does.

## Acceptance

Recorded by `just e14-metadata` in `docs/evidence/E14/metadata.txt`:

- **`pw-core`, `tests/metadata.rs`**: the part and its place in the head;
  each refusal, with controls; an item's property written where it is; a
  signal refused; a speculated value not refused.
- **`pw-render`, `tests/metadata.rs` and `tests/titles.rs`**: the head's
  metadata, escaped and nothing in the body; a value that is HTML refused;
  the binary's document; and the correction, a static page that states its
  title shipping no runtime.
- **The development server's tests**: the store's page and a signal page
  describing themselves in their heads.
- **`e2e/stores.spec.mjs`**, in three engines: each store's page has its
  own description.
- **The corpus at C11**: `corpus-check`; every rejected fixture emitting only
  its own defect; generality 37 / 37.
- **`scripts/metadata_mutations.py`**: 34 mutants.

## Not claimed

- **A description every page states.** It is optional.
- **`media` and `lang` on metadata.** They are refused.
- **A `<link>` in the head**: a canonical URL, an alternate language, an
  icon. A page cannot state one yet.
- **Structured data as JSON-LD**, a `<script type="application/ld+json">`.
- **Microdata's own rules**: that a `<meta itemprop>` is inside an item, or
  that an `itemref` names an element.
- **What a description says.** Its length, and whether it repeats another
  page's, are a person's to judge.
