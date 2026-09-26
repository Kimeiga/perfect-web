# ADR-0096: a template moves no URL

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (the architect's
escaping ruling of 2026-08-06; ADR-0094).

## Context

ADR-0094 refuses code a template writes. Two elements move a URL instead,
past every check a URL attribute gets. On 2026-09-26, at e24b572, both
checked and built:
- **`<base href={msg}>`.** A `<base>` element anywhere in a document moves
  every relative URL resolved after it. The page a view renders into writes
  the view's markup first, then `<script type="module"
  src="/pw-runtime.mjs">` (the development server's shell). So `msg` chose
  the origin the platform's own runtime loaded from, and every relative
  link and form after it.
- **`<animate attributeName="href" values={msg}>` and `<set ..
  to={msg}>`.** An SVG animation sets the attribute it names, and `values`
  and `to` are plain attributes. So a link's `href` became any URL,
  `javascript:` included, and `<set attributeName="onclick">` an inline
  handler, which ADR-0094 refuses when it is written.

## Decision

**A template moves no URL** (PW5024):
- a `<base>` element is refused. The document's base is the platform's,
  and a path is written whole;
- an `animate` or `set` element whose `attributeName` names a URL attribute
  (read as ADR-0095 reads it) or an inline handler is refused. So is one
  whose target a value names. Animating anything else is left alone.

Nothing in the corpus writes a `<base>` or an animation.

**(ruling needed)** Which other head elements a view may write in a body.
`<meta http-equiv="refresh">` redirects, and `<link rel="stylesheet">`
loads a stylesheet; neither runs a script, and neither is refused.

## Acceptance

- **`compiler/pw-core/tests/platform_markup.rs`**, 2 tests, each with a
  control. Both fail at e24b572, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/moved_url_mutations.py`,
  `just e10-moved-urls`, 4 mutants.
