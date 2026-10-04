# ADR-0189: a `<link>` is written where HTML allows it

Status: accepted under the owner's delegation of 2026-10-02. Date: 2026-10-04.
Milestone: E14. Found by checking what ADR-0186 left: which elements of a
document's head a page could still write in its markup.

## Context

- **What a page could write.**
  - **Refused already**: a `<script>` (PW5023), a `<base>` (PW5024), a
    `<title>` anywhere but at the top of a page's view (PW5030), and the
    same for a `<meta>` (PW5034).
  - **Checked and built**: `<link rel="canonical" href="/stores/1" />`,
    `<link rel="icon" href="/favicon.ico" />` and
    `<link rel="stylesheet" href="/a.css" />`. Each was written into the
    body, where markup goes.
- **What HTML says** (WHATWG HTML, "Link types"): a `<link>` is allowed in
  the body when its relations are all body-ok, or when it is an item's
  property (`itemprop`).
  - **The body-ok keywords** are `dns-prefetch`, `modulepreload`,
    `pingback`, `preconnect`, `prefetch`, `preload` and `stylesheet`.
  - **Not body-ok**: `canonical`, `icon`, `manifest` and `alternate`, among
    others.
  - **One `<link>` has either a `rel` or an `itemprop`**, not both.
- **What that means for a page.** A canonical link written into the body
  tells no search engine the page's address, and an icon there is shown by
  no browser. The page checked and built, and nothing said that what it
  wrote was read by nothing.

## Decision

1. **PW5035: a `<link>` in markup is one HTML allows in the body**, in a page
   or in a view, since a view's markup is its page's body. Refused:
   - **a relation that is not body-ok**, `canonical` or `icon`, alone or
     among body-ok ones, compared as HTML compares keywords, ignoring ASCII
     case;
   - **a `<link>` with neither `rel` nor `itemprop`**, with both, or with a
     blank `rel`;
   - **a `rel` computed in place**: which relation it names decides whether
     the link is allowed.
2. **Every other `<link>` is written where it is**, as before: a stylesheet,
   a resource to preload or prefetch, an origin to connect to early, and an
   item's property.
3. **The head stays the host's**, except for what a page states of itself:
   its title (ADR-0183) and its metadata (ADR-0186). The repair says so.
4. **Corpus C12.**
   - **New fixtures**: R-053, a canonical link in a page's markup; A-028,
     links the body allows, an item's property among them. Two categories
     are added, one for each.
   - **Generality witnesses**: a GENERAL one, an icon among body-ok relations
     in a view, and a NEIGHBOUR one, body-ok relations in another case and an
     item's property. Generality is 38 / 38.

## Alternatives

- **Hoist a page's `canonical` and `alternate` links into its head**, as
  ADR-0186 does its metadata. That is the next step if a page needs one. A
  store's page is served at one address today, and an alternate language
  needs a page in two. Until then, refusing one is better than serving it
  where nothing reads it.
- **Refuse every `<link>`.** A stylesheet, a preload and a preconnect are
  allowed in the body and do what they say there.
- **Refuse a stylesheet chosen by a value**, `href={theme}`. CSS loaded
  from an address a value picks could be the wrong file. But that is a
  question about addresses, whose escaping ADR-0095 chooses by the attribute
  that holds them, wherever it is written. It is not a question about where
  a link is written.
- **Leave it to an HTML validator.** None runs on a page this host serves.
  The compiler reads every page, and tells the author at the line.

## Acceptance

Recorded by `just e14-links` in `docs/evidence/E14/links.txt`:

- **`pw-core`, `tests/links.rs`**: each refusal; and the controls, every
  body-ok relation and an item's property, written in the body where they
  are.
- **The corpus at C12**: `corpus-check`; every rejected fixture emitting only
  its own defect; generality 38 / 38.
- **Every program the repository checks is clean.**
- **`scripts/links_mutations.py`**: 9 mutants.

## Not claimed

- **A page's canonical or alternate address.** A page cannot state one yet.
- **`<style>` in markup.** It is written where it is, which for a page is
  its body. Browsers apply it, but HTML puts it in the head. Styling is
  charter §8.3's: scoped styles, cascade layers and typed tokens.
- **What a stylesheet or a preload loads.**
