# spike: marko-stream-resume

**Charter reference:** §14 Milestone 0 task 9.

## Questions

1. Does a static Marko page emit no application JavaScript? (Or what does it
   actually emit?)
2. Does a delayed subtree stream independently?
3. Does one interactive button resume without eagerly hydrating the whole page?
4. What are the emitted asset and runtime bytes?

## Run it

```bash
just spike-marko
```

Evidence: `docs/evidence/M0/spike-marko-stream-resume.txt`.
Pinned: `marko` 6.3.32, `@marko/run` 0.11.8, `@marko/run-adapter-node` 2.0.6,
`vite` 8.2.0, Node v22.21.1 (all verified on the npm registry 2026-08-05).

## Routes

| route | contents | tests |
|---|---|---|
| `/static` | no state, no handlers, no `await` | the 0-JS budget |
| `/stream` | two delayed subtrees (400 ms, 1200 ms per charter §15.5) | streaming |
| `/counter` | one button in a small document | resumption |
| `/counter-large` | the same button, 200 extra inert table rows | payload scaling |

## Measured results

```text
route            html    html.gz  downloaded JS  inline JS  script tags
/static           588        357          0 B        0 B    0
/stream          1669        986          0 B      849 B    3 inline
/counter         1721        930       3921 B      350 B    1 ext + 1 inline
/counter-large  16537       2468       3927 B      356 B    1 ext + 1 inline
```

Streaming timeline for `/stream` (chunked transfer, 3 chunks):

```text
time to first byte    3.1 ms
SHELL_READY           3.1 ms     <- shell not blocked by either subtree
ESTIMATE_READY      408.2 ms     <- 400 ms delay
RECS_READY         1206.0 ms     <- 1200 ms delay
response complete  1206.7 ms
```

Payload scaling:

```text
                  html     total JS   route-specific JS
/counter          1721      3921 B          176 B
/counter-large   16537      3927 B          182 B
HTML grew 9.6x            JS grew 1.002x
```

**All six checks pass:** `static-truly-zero-js`, `stream-zero-downloaded-js`,
`shell-not-blocked`, `estimate-streamed`, `recs-streamed`, `out-of-order-ok`,
`payload-tracks-island`.

## Findings

**F-1 — the static route is genuinely zero-JS. Not "small". Zero.**
`/static` serves 588 bytes of HTML with **no `<script>` tag of any kind** and no
downloaded JavaScript. Charter §18.4's budget — *"static page: 0 application JS
and 0 runtime JS"* — is met exactly, and Marko's own build report agrees
(`/static … 0.0 kB`). Assumption A-004 is **validated**.

**F-2 — the streamed route downloads zero JS but needs ~849 bytes of INLINE
script.** This is the honest qualification of "zero JS". `/stream` references no
external asset, but carries three inline `<script>` blocks: a ~700-byte
tree-walking shim plus two 12-byte `M._.w()` calls that fire as each delayed
subtree lands. The shim locates comment markers and replaces placeholder content
in place.
That is **exactly the mechanism charter §8.6 describes** — *"support out-of-order
server completion using document-range patches… implement this with a tiny,
well-tested browser shim and `<template>`-like payloads"*. Marko has already
built the thing the charter proposes building. The cost is measured: 849 bytes,
inline, no extra request, no framework download.

**F-3 — streaming is real and out-of-order, verified by wall clock.** The shell
arrived at 3.1 ms while two subtrees were still pending; the 400 ms subtree
arrived before the 1200 ms one; the response stayed open until 1206.7 ms across
3 chunks with `transfer-encoding: chunked`. Nothing about the shell waited on
either slow region.

**F-4 — resumption is real: the client payload does NOT scale with the
document.** `/counter-large` has 9.6× the HTML of `/counter` and the same single
button. Route-specific client code: **176 B vs 182 B** (1.03×, and that
difference is the longer chunk filename in the import specifier). A hydrating
framework would need code proportional to the rendered tree. Charter §8.5's
"no whole-tree hydration" holds.

**F-5 — the emitted HTML *is* the charter's document-parts model, visibly.**
The rendered button carries no inline handler:

```html
<p>In cart: <output id=count>0<!--M_*2 a--></output></p>
<button id=add>Add to cart</button><!--M_*2 b-->
```

Those `<!--M_*2 a-->` comments are the dynamic-part anchors, and the inline
`M._.r=[…]` block is the serialized resume manifest. This maps almost one-to-one
onto charter §8.4's `TextPart` / `EventPart` and §8.5's resumption metadata.
**Consequence:** Milestone 7 ("own document-parts compiler") has a concrete,
working oracle to diff against, exactly as the charter intends.

**F-6 — the client runtime splits into a shared chunk plus a tiny per-route
entry.** 3,745 B shared (2,024 B gzip) + 176 B per interactive route. The shared
chunk is amortized across every interactive route; only the 176 B is per-route.
Reporting a single "client JS" number for a route is therefore misleading in
both directions, and Milestone 3's benchmark harness must report the split.

**F-7 — measuring only the entry chunk under-reports client JS by 20×.**
The first version of `measure.mjs` counted `<script src>` and stopped, reporting
`/counter` as 176 bytes. The entry `import`s the 3,745-byte shared runtime.
`clientJs()` now walks the static ESM import graph transitively. Recorded because
the same trap will appear in every Milestone 3 baseline comparison against
Next/SvelteKit, where under-reporting our own numbers would be an unfair
benchmark.

**F-8 — Marko's syntax has sharp edges that cost real time.**
- `--` at line start is a **text line**, not a comment. Comments are `/* */`.
  Compilation failed with *"A concise mode closing block delimiter can only be
  followed by whitespace"*.
- `@placeholder` / `@catch` attach to `<try>`, **never** to `<await>`.
- A top-level `>` in an attribute value silently ends the tag — so arrow
  functions in attributes must be parenthesized. The `delay` helper was moved
  into `src/delay.js` to sidestep this entirely.
- `<for>` over primitives needs `by=(x) => x`, not `by=x`.

These are all documented in `marko/cheatsheet.md`, which the compiler error
helpfully points at. Relevant to charter §19 (AI reliability): these are exactly
the "silent miscompile" failure modes the new language is meant to eliminate —
Marko's own diagnostics catch some (`--`) but not others (the `>` truncation
"usually still compiles clean").

## Limitations

- **No browser was used.** Interactivity is inferred from the emitted resume
  manifest and payload sizes, *not* observed. Nobody has clicked the button.
  Charter §14 M3 task 11 (Playwright across Chromium/WebKit/Firefox) is where
  that gets verified; until then, claim 3 is supported by byte-level evidence
  only.
- No JS-disabled test, no accessibility audit, no Lighthouse/INP measurement.
- Single run, single machine, localhost, no network shaping. Charter §18.5
  requires sample counts and distributions for anything published as a
  benchmark; these numbers are spike evidence, not benchmark results.
- No comparison against Next/React or SvelteKit baselines (Milestone 3, §18.1).
- Nothing here is generated from `pw` — these are hand-written Marko templates.
  The adapter that lowers `pw` templates to Marko is Milestone 3.
