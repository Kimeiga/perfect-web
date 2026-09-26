# ADR-0094: a template writes no code

Status: accepted under the owner's instruction of 2026-09-26 ("keep working
until its perfect"). Date: 2026-09-26. Milestone: E10 (charter §7.8, §8.1;
the architect's escaping ruling of 2026-08-06).

## Context

The architect ruled on 2026-08-06 that "the renderer must know whether a
value occupies Text, Attribute, URL, Style or RawHtml". The template IR
chooses the context from where a value sits, and `pw-render` escapes it:
- text escapes `&`, `<` and `>`;
- an attribute escapes its quotes;
- a URL refuses a scheme that executes;
- a style refuses `expression(` and a scheme in `url(`.

Four places a value can sit are none of those. On 2026-09-26, at 7fc50db,
each checked and built:
- **`<script>{msg}</script>`**: the IR wrote a text part. Text escaping means
  nothing inside a script element, so `msg = "alert(document.cookie)"` runs
  as the page loads.
- **`<button onclick={msg}>`**: an attribute part. An inline handler's value
  is JavaScript, and `alert(document.cookie)` has nothing to escape.
- **`<iframe srcdoc={msg}>`**: an attribute part. The browser decodes the
  escaped entities, so `<script>` in `msg` is a script element in the frame's
  document. That document runs on this page's origin.
- **`<style>{msg}</style>`**: a text part, although the IR's own
  documentation gives a `style` element the Style context. A stylesheet a
  value writes into can style and read out the page.

A static `<script>` or `onclick="go()"` checked too. That is code the
language never sees: no effect row, no placement, no resumability. Handlers
are bound with `on:`. So did `<a href="javascript:go({id})">`. The renderer
refuses a script scheme only where a value is the whole URL, and a browser
percent-decodes a `javascript:` URL before it runs. So an `id` of
`1);alert(document.cookie);(` runs, however it was percent-encoded.

## Decision

**A template writes no code** (PW5023). Each of these is refused:
- a `<script>` element;
- an inline handler attribute: `on` and a name, without the `on:`
  directive's colon;
- a URL attribute whose written text begins with `javascript:` or
  `vbscript:`, read as a browser reads it (whitespace and controls
  dropped, case ignored), or whose scheme a value completes:
  `jav{x}ascript:`. A static `data:` is left to the author: an image can be
  one, and a document in one runs on an opaque origin;
- a `srcdoc` attribute;
- a `<style>` element that holds anything but text. Text braces open a
  hole, so a static stylesheet cannot be written in one either. A value is
  styled through a `style` attribute, which is escaped as a style.

Nothing in the corpus writes any of them. `pw build` refuses them too
(ADR-0090).

## Acceptance

- **`compiler/pw-core/tests/code_in_markup.rs`**, 5 tests, each with
  controls, including an `on:` directive, an `https:` URL with a value and a
  `data:` image. All 5 fail at 7fc50db, the commit before.
- **No rejected, rule or generality fixture's diagnostics change.** The
  store, kiokun and the accepted corpus check clean.
- **Mutation controls:** `scripts/code_in_markup_mutations.py`,
  `just e10-code-in-markup`, 10 mutants.
