# ADR-XXXX: what a page may run is generated from its build

Status: draft (the integrator's; ADR-0195's ruling 13, the host's half).
Date: 2026-10-14. Milestone: E14. Builds on ADR-0148 (the classic script
that starts the runtime), ADR-0302 (the page's announcer), ADR-0306 (a
navigation that keeps its layout), ADR-0332 (a payment's page, framed) and
ADR-0258 (a sign-in at a provider).

## Context

- **The owner ruled it** (ADR-0195, ruling 13): a `<link>`'s `href` is a
  literal or a build's asset; a cross-origin one's origin is declared by the
  program; "the host's Content-Security-Policy is generated from that same
  list". None of it was built (KNOWN_LIMITATIONS, "A `<link>` in markup may
  name any origin").
- **The payments made it urgent.** The order page frames the payment
  provider's page (ADR-0332), and since 2025 SAQ A asks that the page
  framing a provider's fields is not open to attacks by script (PCI SSC FAQ
  1588, as ADR-0332 read it). ADR-0332 recorded "the store's pages send no
  Content-Security-Policy" as not claimed.
- **What a document this host serves runs and loads**, read from the host
  and the runtime (2026-10-14):
  - one inline classic script, `import("/pw-runtime.mjs")` (ADR-0148);
  - a `<script type="application/json">` holding the page's parts, a data
    block, which HTML never runs;
  - the runtime's modules, its handlers' (`/handler/…`), its speculations'
    and its computed values' (`/speculation/…`, `/computed/…`), and two
    WebAssembly modules (`/pw-resume.wasm`, `/pw-render.wasm`), all from
    this origin, and every request it makes is to this origin (no
    WebSocket, no `EventSource`, no `eval`);
  - the data layer's stylesheet, in a `<style>` (ADR-0220): the store's;
    none for the feed or kiokun;
  - the announcer (ADR-0302), served with a `style` attribute that hid it;
  - in the store, a frame of the payment provider's page (ADR-0332); in a
    deployment with accounts, a form's `GET /sign-in` answered with a
    redirect to the identity provider (ADR-0258), which a deployment may
    keep at an origin of its own;
  - in the programs' markup, today: images from this origin's uploads, no
    `style` attribute, no `<style>` element, no load from another origin.
    The compiler refuses `<script>` (PW5023) and `on…` attributes, and a
    `<style>` holding a value.
- **Practice** (each read 2026-10-14):
  - **CSP Level 3** (W3C, TR): a hash source is the hash of an inline
    element's text, "base64 and base64url encodings are treated as
    equivalent" (§2.3.1); a source list holding a hash or a nonce "Does Not
    Allow" all inline behaviour, so `'unsafe-inline'` beside one allows
    nothing (§6.7.3.2); a hash matches a style attribute only beside
    `'unsafe-hashes'` (§6.7.3.3); compiling or instantiating WebAssembly is
    gated on `'wasm-unsafe-eval'` or `'unsafe-eval'` (§6.1.10); the
    `form-action` check (§6.4.1.1) names no redirect.
  - **Browsers**, by MDN's compatibility data: `'wasm-unsafe-eval'` since
    Chrome 97, Firefox 102 and Safari 16; `style-src-attr` since 75, 108 and
    15.4; `'unsafe-hashes'` since 69, 109 and 15.4; `style-src-elem` parsed
    with no effect in WebKit from 15.4 until 26.2 (WebKit bug 276931). MDN's
    `form-action`: whether it holds a form's redirect "is debated and
    browser implementations of this aspect are inconsistent (e.g., Firefox
    57 doesn't block the redirects whereas Chrome 63 does)".
  - **A strict policy** (web.dev, "Mitigate cross-site scripting (XSS) with
    a strict Content Security Policy", updated 2024-09-13): for a page
    "served statically, or pages that need to be cached", a script allowed
    by its hash, with `'strict-dynamic'`, `object-src 'none'` and
    `base-uri 'none'`; host allowlists "can be bypassed by attackers".
  - **`nosniff`** (Fetch, "Should response to request be blocked due to
    nosniff?"): a script runs, and a stylesheet applies, only where its
    response's type says it is one. The uploads already sent it (ADR-0260).

## Decision

1. **Every HTML answer the host writes carries a Content-Security-Policy**
   (`csp.rs`), and none is written by hand.
   - **A document's**:
     `default-src 'none'`;
     `script-src 'self' 'wasm-unsafe-eval' '<the bootstrap's hash>'`;
     `style-src 'self' '<the layer's style's hash>' '<each <style>'s>'`;
     `style-src-attr 'unsafe-inline'` only where the build writes a `style`
     attribute (no program does);
     `img-src 'self'`; `connect-src 'self'`;
     `frame-src <the payment provider's origin>` only where the deployment
     takes payments and the build frames a page;
     `form-action 'self'`, and the identity provider's origin where a
     sign-in leaves this one;
     `frame-ancestors 'none'`; `base-uri 'none'`.
   - **Every other HTML answer runs nothing**: a page not found, a page
     moved, a page the build rendered with no runtime, the identity's own
     pages: `default-src 'none'; img-src 'self'; style-src 'self';
     form-action 'self'; frame-ancestors 'none'; base-uri 'none'`.
2. **Generated from the build and the deployment.** The hashes from the
   texts as served (`RUNTIME`'s is `RUNTIME_SOURCE`'s, held by a test; a
   served document's every inline script and style is held to its header by
   another). What the markup asks is read from the build's templates: a
   `style` attribute, written or a value's, or markup a capability lets
   through whole; a `<style>`'s text; an `<iframe>`. The providers from the
   deployment's configuration. **One policy for a build's every page**: a
   navigation that keeps its layout (ADR-0306) keeps its first document,
   and with it that document's policy.
3. **`'self'` and a hash, not `'strict-dynamic'`.** The runtime's modules
   are this origin's, so `'self'` is an allowlist of the one origin the
   page's code comes from. web.dev warns that an allowlist "can be bypassed
   by attackers": with one origin on it, the way past is a response of this
   origin that a browser runs as a script, which the next ruling closes.
   `'strict-dynamic'` would ignore `'self'` and trust whatever the runtime
   inserts; nothing here needs that.
4. **Every answer's type is its type**: `X-Content-Type-Options: nosniff`
   on every answer the host writes, so `'self'` allows this origin's
   JavaScript, served as JavaScript, and nothing else of it; and a `.css`
   the build ships is served as `text/css`, the one type a browser applies
   as a stylesheet.
5. **The host writes no inline style.** The announcer is served empty and
   unstyled, which no one sees; the runtime hides it through the CSSOM,
   which a policy allows, before it says anything. A `style` attribute
   would be allowed only by `'unsafe-hashes'` (§6.7.3.3).
6. **Tests that ran what a policy refuses** run it otherwise: the
   performance suite's control read layout through `eval`; it reads it
   through a module of this origin now.

## Not claimed

- **Ruling 13's other two clauses**: a `<link>`'s `href` a literal or a
  build's asset, and a cross-origin origin declared by the program. A
  program that writes a load from another origin builds, and its page's
  policy refuses the load in the browser, which says so in its console and
  the suite's check; the build does not say so yet. Step two (NEXT): the
  build refuses it, and the program's declaration adds its origin.
- **An address a value chooses** (`<img src={…}>`) loads from this origin
  alone, and nothing at build says so.
- **Reports**: no `report-to`. A refusal is said in the browser's console
  and seen by the suite; a deployment hears of none.
- **Trusted Types** (`require-trusted-types-for 'script'`): the runtime
  writes markup its renderer made through `innerHTML`.
- **One policy for the build**, not one per page: a page that frames
  nothing, in a build that frames the provider, may frame it too.
- **A blob store at another origin** (none exists) would need its origin
  among the images'.

## Alternatives

- **A nonce per response.** It needs a value made for each document and a
  document that is never kept; a hash needs neither, and the one inline
  script is the same bytes on every page.
- **`'strict-dynamic'`**, web.dev's strict policy: see ruling 3.
- **A policy per page**: a navigation that keeps its layout keeps the
  first page's document, so a page reached that way would run under
  another page's policy.
- **`<meta http-equiv>`**: a policy delivered so ignores `frame-ancestors`
  and applies only to what is parsed after it, and `http-equiv` is the
  host's anyway (ADR-0186).
- **`'unsafe-hashes'` for the announcer's style**: one "unsafe" source for
  a style no one sees, where the CSSOM needs none.
- **Report-Only first**: a policy that refuses nothing is one whose every
  refusal the suite would have to find in a report; the suite runs every
  served program under the enforced one instead, in three engines.

## Acceptance

Recorded by `just e14-page-policy` in `docs/evidence/E14/page-policy.txt`:

- **The server's tests** (`csp.rs`; `tests/page_policy.rs`; the moved
  page's in `tests/redirects.rs`; the identity provider's page's in
  `tests/sign_in.rs`): the hash; what markup asks, wherever it is; a
  document's policy, written whole and streamed, each inline script and
  style it holds allowed by its hash; the payment provider framed; a
  sign-in at another origin a form's to reach; every other HTML answer
  running nothing, and no other answer carrying a policy; every answer's
  type its type; the host writing no style attribute.
- **The browser suite** (`e2e/page-policy.spec.mjs`, three engines): each
  page the suite serves, the store's, the feed's, the identity provider's
  and kiokun's, runs under its policy with nothing refused; a press reaches
  its handler; the order page frames the provider's page; the announcer is
  seen by no one when it speaks. The controls: an inline script and a
  `style` attribute refused and said; without `'wasm-unsafe-eval'` the
  runtime does not start; a policy naming no provider frames nothing.
- **Mutation controls** (`scripts/page_policy_mutations.py`).
