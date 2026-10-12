# ADR-0311: a page states its canonical address

Status: accepted under the owner's delegation of 2026-10-02 (the
integrator's). Date: 2026-10-12. Milestone: E14. Builds on ADR-0189 (a
`<link>` where HTML allows it), ADR-0186 (a page states its metadata),
ADR-0280 (a page's address, as `navigate` writes it), ADR-0306 (soft
navigation) and the ruling of 2026-10-09 (PARALLEL.md, "1e's questions
answered", Q2). Asked for by W6: kiokun's word page answers an inflected
word with its dictionary form's entry (PARALLEL.md, 2026-10-12).

## Context

- **A page could not say which address is its own.**
  - ADR-0189 refuses a `<link rel="canonical">` in markup (PW5035). Markup
    is the body, where HTML does not allow one and nothing reads it.
  - ADR-0189's Alternatives call hoisting a page's canonical link into its
    head "the next step if a page needs one".
  - ADR-0293's Differences 10 and 12, kiokun.com's canonical link and its
    `og:url`, wait on it.
- **Now a page needs one.** kiokun's word page answers /word/食べた with
  食べる's entry and a note, 200: many addresses, one entry (the ruling of
  2026-10-12). Without a canonical link, a search engine indexes each
  inflected address as a page of its own.
- **The ruling of 2026-10-09, Q2:**
  - a canonical link is a page address, written as HTML writes it at the
    top of a page's view, and checked as `navigate`'s address is;
  - the host writes it absolute with the deployment's configured public
    origin, never the request's `Host`;
  - a host whose pages state one and that has no origin refuses to start;
  - `og:url` is the host's, written from it, and a page's own is refused;
  - there is no default self-canonical.
- **Found on the way: a soft navigation kept the last page's metadata.**
  ADR-0306's swap took the next document's title, `description` and style,
  and no other metadata, though ADR-0186 lets a page state any name or
  property. After a navigation, the head kept the last page's Open Graph
  properties and `theme-color`, which browsers read live, and lacked the
  next page's.
- **Found on the way: an address read as a call.** `href={StorePage(id)}`
  is a call in the language's syntax, and the analyses that read a call
  looked it up by its path.
  - Effect inference charged the store's page, stating its own address,
    with its own reads, `database.read` and `session.read`, as a value its
    template computes (PW0401, PW0334). The page did not build.
  - The privacy label joined the label of the page an address names. A
    shared page naming a session page's address was refused as the
    session's (PW5001). This ADR's mutation controls found it: exempting
    the address in effect inference and in the callee the other analyses
    read was not enough, and a mutant of the second survived.
  An address names a page and runs nothing.

## Research (primary sources, read 2026-10-12)

- **WHATWG HTML, "Link type canonical"** (§4.6.8.4): the keyword "indicates
  that URL given by the href attribute is the preferred URL for the current
  document". It creates a hyperlink, and is not body-ok.
- **RFC 6596.** §3: the canonical IRI "MUST identify content that is either
  duplicative or a superset of the content at the context (referring)
  IRI". §5: before adding one, verify that "the content of the context IRI
  is duplicated within the content of the target".
- **Google Search Central, "How to specify a canonical URL"** (updated
  2026-07-10):
  - the link "is only accepted if it appears in the <head>";
  - "Use absolute paths rather than relative paths";
  - a self-referential canonical on the canonical page itself is
    recommended;
  - canonicals are for "duplicate or very similar pages".
- **The Open Graph protocol**: `og:url` is "The canonical URL of your
  object that will be used as its permanent ID in the graph."
- **PortSwigger, "HTTP Host header attacks"**: the header "is in fact user
  controllable". Where an absolute URL is needed, "require the current
  domain to be manually specified". Building it from the header leads to
  web cache poisoning and password reset poisoning.
- **The URL Standard's origin** is a tuple of scheme, host and port, with
  nothing after them.

## Decision

1. **A page states its canonical address with `<link rel="canonical"
   href={Page(args)} />`**, once, at the top of its view, beside its
   `<title>` and its `<meta>`.
   - Its `href` is a page's address, checked as `navigate`'s is
     (ADR-0280): the page by its declaration, among the views, served at a
     route.
   - Its arguments are the page's parameters, related as a call's are
     (PW0604, PW0605), by place or by name.
   - Each argument is a value the page reads, by its path, or text.
   - A page may name another page's address as its own.
2. **PW5051, `canonical_address`**, refuses an address that is not a page's:
   - one that states no address;
   - one written as text, which could name no page, or one served
     elsewhere;
   - one computed in place;
   - one naming a function, a view or nothing;
   - one naming a page served at no route, or at a route with a query part
     (not claimed);
   - an argument computed in place.
3. **PW5035 still refuses a canonical link anywhere else**, saying where it
   goes:
   - in a view, a component or a layout, which every page that composes it
     would share;
   - inside an element or a block;
   - a second one;
   - one whose `rel` names more than `canonical`;
   - one with any other attribute, `hreflang` or `media`, which the host
     does not write.

   Its invariant now names the exception. A call that names a page outside
   a canonical link's address is still PW0027.
4. **An address is its own node in the HIR, `Expr::Address`, never a
   call.** A canonical link's `href` that calls a path is lowered as one.
   - Its page stays the path as written, and is not an expression a name
     check reads as a value.
   - Its arguments are its children, and are read as values are.
   - Nothing that reads a call reads it: what a body performs, what it
     reads, and the label it carries.
   - The value relations type it as text, and relate its arguments to the
     page's parameters.
   - A call naming a page anywhere else is still a call, refused (PW0027).
5. **The host writes it into the head, absolute**, after the page's
   metadata (a part of the head, numbered after the metadata).
   - It is the deployment's public origin followed by the route's path,
     each `{name}` filled as a link's hole is (`url_component`).
   - It never uses the request's `Host`.
   - Open Graph's `og:url` is written from it, after it.
   - A value no segment carries (empty, `.` or `..`) is refused at render,
     as `navigate`'s is, and so is a value that is HTML.
6. **The origin is `PW_ORIGIN`**, which the identity already reads; a
   development deployment's is this machine's.
   - **A production deployment whose pages state a canonical address, and
     that states no origin, is refused at start.**
   - **An origin is an origin**: `PW_ORIGIN` with a path, a query, a
     fragment or credentials is refused at start. Until now it was taken
     whole.
   - `pw-render` takes `--origin`, and refuses to render a page that states
     one without it.
7. **`og:url` is the host's**: a page's own is refused (PW5034), as its
   charset and viewport are.
8. **No default self-canonical.** A page that wants search engines to take
   its own address states it, as Google recommends. `examples/demo/about.pw`
   does, and A-033 does.
9. **A navigation takes the next page's head whole**, a correction to
   ADR-0306.
   - It takes the next page's title, every `<meta>` the page states (each
     name but the host's viewport, and each property), its canonical
     address and `og:url`, and its style.
   - The host's own charset and viewport stay.
10. **Corpus C19.**
    - A-033 is new: a page's own canonical address.
    - R-063 is new: a canonical address written as text.
    - R-053 changed: its canonical link is now inside `<main>`. At the top
      of the view it would be the page's own address, refused only for its
      text.
    - Generality witnesses for `canonical_address`: a GENERAL one, an
      address calling a view; a NEIGHBOUR one, another page's address with
      its arguments by name in another order, one of them text.
    - Generality is 45 / 45.

## Alternatives

- **An `href` written as text, checked as a link's is (PW5009).** A link
  may name an upload's path or another site. A canonical address names a
  page, and its arguments are the page's parameters, typed.
- **A clause beside the page, `canonical Page(args)`.** HTML's own element,
  where a reader of the markup expects it, beside the title and the
  metadata, as ADR-0186 chose for metadata.
- **A relative address.** RFC 6596 allows one. Google advises an absolute
  one, and a link's preview needs one. The deployment's origin is
  configuration the host has.
- **The request's `Host`.** It is what kiokun.com uses, and it is the
  client's to choose. An address made from it is the attacker's.
- **Exempt the address wherever a call is read.** This was tried first.
  - Effect inference, PW0027 and the callee the other analyses read were
    exempted.
  - The privacy label still read the address as a call of the page it
    names (above).
  - About ninety places match a call, in twenty-five files. A list of
    exemptions is one that a new analysis forgets.
- **Read the address as a keyword**, as `navigate` is. Fifty places match
  a keyword expression, several of them generically, and would each have to
  be read again. Its own node is named where it must be: Rust's exhaustive
  matches named the two places, the backend's construct names and Koka's.
- **`alternate` links**, for a language or a feed: refused as before. Each
  needs its own reading (`hreflang`, `type`, `media`), and no page needs one
  yet.

## Acceptance

Recorded by `just e14-canonical` in `docs/evidence/E14/canonical.txt`:

- **`pw-core`, `tests/canonical.rs` and `tests/links.rs`:**
  - the part, after the metadata, the document's, and nothing in the body;
  - arguments by place, by name and as text, and another page's address;
  - each PW5035 and PW5051 refusal, PW0604 and PW0605;
  - PW0027 elsewhere still;
  - a shared page naming a session page's address checks clean, its
    address lowered as an address and never as a call;
  - `og:url` refused, and Open Graph's title not.
- **`pw-render`, `tests/canonical.rs`:**
  - the head's address, absolute, escaped as a link's hole, and `og:url`;
  - a value with `/`, `?`, `#` and a quote kept in its segment;
  - refusals: no origin, a value no segment carries, a value not given,
    and a value that is HTML.
- **The development server's tests:**
  - the store's page, stating its own address, served with it in its head
    with the deployment's origin and not the request's `Host`;
  - not served without an origin;
  - a production deployment refused at start without one;
  - `PW_ORIGIN` read as an origin.
- **`e2e/canonical.spec.mjs`, in three engines:**
  - the demo's about page's address in its head, absolute;
  - a soft navigation that leaves nothing of the last page's head but the
    host's.
- **The corpus at C19**: `corpus-check`, `checking_source.rs`, and
  generality 45 / 45.
- **Mutation controls**: `scripts/canonical_mutations.py`, 21 mutants. Two
  of `metadata_mutations.py`'s were re-anchored, and its script was run
  whole.

## Not claimed

- **A canonical address with a query part.** It is refused, as a
  navigation to a page whose parameter is a query's is (ADR-0309).
- **`alternate`, `icon` and `manifest` links**: refused as before.
- **JSON-LD** (Q3 of 2026-10-09): the integrator's next.
- **What a canonical address names.** Whether the named page's content
  duplicates this one's, as RFC 6596 asks, is the program's to decide.
