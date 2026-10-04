# ADR-0183: a page states its title

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14, the rest of the audit's ninth gap: WCAG 2.4.2, which ADR-0182
left open.

## Context

- **Every store's page was titled "Store".** The host wrote it, because a
  page could not say what it is.
  - WCAG 2.4.2 (level A) asks that a title identify its page, and names
    this failure F25: one title for many pages.
  - A screen reader says the title first. A person with three stores open in
    three tabs could not tell them apart.
  - axe's `document-title`, which Lighthouse runs, asks only that a title be
    there. So the store passed it (ADR-0182).
- **A demo page was titled by its declaration's name**, `BindPage`.
- **How others do it.**
  - **An element in the markup, moved to the head.** React 19 renders a
    `<title>` written anywhere in a component tree into `<head>`. Svelte has
    `<svelte:head>`. Astro and Marko write the document's head in the
    page's own template.
  - **A function beside the page**: Next.js's `generateMetadata`, Remix's
    `meta`, Qwik's `head`.
  - None of them requires a title.

## Decision

1. **A page states its title with `<title>` at the top of its view**, beside
   `<main>`, written as text and the values the page reads:
   `<title>{store.name}</title>`. This is HTML's own element, kept where
   HTML puts it (charter §8.2, ADR-0004).
   - **The host writes it into the document's `<head>`**, from the page's
     values. It renders nothing in the body.
   - **It is a part.** The page's plan names it, and a change to what it
     reads is set as a text part's is. The browser's runtime sets it as
     `document.title`, and only when its text changes (ADR-0182).
   - **Its text is collapsed and trimmed**, as the browser's
     `document.title` reads it. So the title a page is served with and the
     title a change sets are the same text.
   - **It is numbered after every other part of the page**, wherever it is
     written. Writing one moves no other part's number.
   - **A page that states none is titled by its host, as before.** The
     store's shell says "Store", for the benchmark's copy. The demos' pages
     are titled by their names.
2. **PW5029: a page served at a route states its title.** Refused:
   - a page with a `route` and no `<title>`;
   - one whose title is only whitespace.
3. **PW5030: a title is the page's: once, at the top of its view, written as
   text and values.** Refused:
   - a `<title>` in a view, which would title every page that composes it;
   - one inside another element, or inside a block;
   - a second one;
   - one holding an element, or a value computed in place.

   Only `<title>` itself is a title: `<Title>` is a view (ADR-0072).
4. **A title says what the page is, from its parameters and its queries'
   values.** Refused at build:
   - **One that reads a signal**, by ADR-0137's rule. The browser renders no
     title again from one.
   - **One that reads a value a press speculates**, such as the cart. The
     browser renders no title again from a speculation. While a press was
     pending, the title would say what the value was, and the page around
     it what it is about to be.
5. **The store states its name**: `<title>{store.name}</title>`. Store 47's
   page is "Blue Bottle", and store 48's is "Harbor Coffee".
6. **Test 14's audit reads it.** A rule of the audit's own checks that the
   title names the page's heading. That is F25's test, which presence
   alone does not make.
7. **Corpus C9.** The corpus is the specification, and examples come before
   features:
   - **New fixtures**: A-025, a page that states its title; R-047, a page
     served at a route that states none (PW5029); R-048, a title in a view
     (PW5030). corpus-check gains their three categories.
   - **Fixtures given a title**, since their pages are served at routes:
     R-023, the link rule's two generality witnesses, and its rule
     fixture. Their old texts are kept in `examples/history/C9/`.
   - **Generality witnesses** for the two invariants: generality is 33 /
     33.

## Alternatives

- **The host titles a page with its `<h1>`.** That makes two statements of
  one thing, one of them implicit. A page whose heading is not its title,
  such as a search page's, could not say so.
- **A `title` clause beside `route`.** A clause is a declaration. A title is
  rendered from the page's values, which is what a view does.
- **A `<head>` the page writes.** Nothing beyond a title is asked for yet. A
  description, for search, can take the same path when it is.
- **Every page states its title, routed or not.** That is right for every
  page that is a document, and it is the next step, with the router. Today
  the pages without a route are served by hosts that name them: the demos,
  the kiokun slice and the benchmark's store. The benchmark's store is
  frozen (ADR-0156), and ten of its tasks' patches are written against the
  first and last lines of its view.
- **A title read from the speculated cart, re-rendered by the browser.**
  The browser's renderer would need a title of its own, for a store whose
  title reads no cart. Refused until a page needs one.

## Acceptance

Recorded by `just e14-titles` in `docs/evidence/E14/titles.txt`:

- `pw-core` (`tests/titles.rs`):
  - a page's title is a part of the document, numbered after the rest, and
    the plan names it;
  - writing one moves no other part;
  - PW5029 and PW5030, each case with controls;
  - a title that reads a signal, or a value a press speculates, is refused.
- `pw-render` (`tests/titles.rs`):
  - a title renders nothing in the body;
  - its text is collapsed as a browser reads it, a missing value or HTML is
    refused, and a template that states none has none;
  - the binary writes it into its document's head, escaped.
- The development server:
  - a store's page is titled by its name, escaped;
  - a store's program that states none is titled "Store";
  - a changed title is set as text, and one that did not change is not.
- The browser, in Chromium, Firefox and WebKit (`e2e/accessibility.spec.mjs`):
  - each page is titled by what it is;
  - a change to its title is set, and set once;
  - the audit's F25 rule holds in every state.
- `scripts/titles_mutations.py`: 16 mutants.

Also:
- **The corpus's tests**: `corpus-check`, the C9 history test, and
  generality at 33 / 33.
- **R-023's C0 excuse is retired.** Its old text declared no route, so its
  link was dead relative to nothing. A-025 now declares a route in the
  program every old text is checked in, and the old text is caught for its
  invariant again.
- **Three older mutants are re-anchored** where this moved their code.
- **The store's page plan is regenerated** (`just e10-component`).

## Not claimed

- **A page without a route is titled by its host**, as before.
- **A store renamed while its page is open.** The store has no change event
  yet; its query is refreshed by its `freshness`. So its title, like its
  heading, changes when the page's values are next read.
- **A title from a signal, or from a speculated value**: refused, as above.
- **`<meta>`, a description, `<link>`.**
