# spike: browser-resumption (risk-retirement experiment RQ-1)

**Question:** is Marko 6's resumption *real*, or is it inferred from payload
sizes? Until this ran, the repository's strongest claim about resumption rested
on "9.6× more HTML produced 1.03× the client JS" — and **nobody had clicked the
button**.

This is a **risk-retirement experiment**, not an engineering milestone. Per the
architect ruling:

> Engineering milestones describe what we have implemented. The risk-retirement
> queue describes experiments that may use temporary dependencies to test
> assumptions early. Passing an early experiment can retire a risk but cannot
> close the corresponding implementation milestone.

Passing here does **not** close E7 (our own document-parts renderer). It
establishes whether Marko is a valid behavioural oracle to build E7 against.

## Run it

```bash
just rq-resumption
```

Evidence: `docs/evidence/E0/spike-browser-resumption.txt`.
Drives **Chrome via CDP** and **Safari via WebDriver or a self-running fallback**,
using only Node 22 built-ins — no Playwright, no Puppeteer, no WebDriver client.

## Method

`proxy.mjs` starts the real built Marko app and injects `public/harness.js` as
the first script in `<head>`. Instrumentation is injected rather than authored
into the templates, because Marko treats `<script>` in a template as a reactive
effect — and because editing the app would mean measuring something other than
the app.

The harness only *observes*: a `MutationObserver` installed before `<body>`
exists, a `PerformanceObserver` for resource timing, capture-phase error
listeners, and DOM node identity captured before and after interaction. It never
patches framework internals.

## Pre-registered checks and decision rule

Both were fixed **before** the experiment ran.

| # | check |
|---|---|
| 1 | page is usable before interaction code loads |
| 2 | no component/template replay during startup |
| 3 | the first click invokes its handler |
| 4 | the handler artifact is absent until needed |
| 5 | only the expected artifact loads |
| 6 | existing DOM nodes are retained, not replaced |
| 7 | focus survives resumption |
| 8 | a second interaction does not reinitialize |
| 9 | out-of-order streamed completion lands in document order |
| 10 | a handler-load failure is observable, not a silently dead control |

Decision rule:

- **all core properties pass in both engines** → Marko is the behavioural oracle for E7
- **passes Chrome, fails Safari** → architectural donor, not a cross-browser oracle
- **replay / eager handler / DOM replacement / lost state** → stop calling Marko resumable for our purposes; keep only what passed
- **only payload scaling passes** → report payload scaling and nothing else

Core properties = checks 2, 3, 6, 7, 8.

## Result

**11 of 12 pass in both Chrome 150 and Safari 26.5.2.**

```text
check                                        chrome   safari
page usable before interaction code loads    pass     pass
no component/template replay during startup  pass     pass
first click invokes its handler              pass     pass
handler artifact absent until needed         FAIL     FAIL
only the expected artifact loads             pass     pass
existing DOM nodes retained, not replaced    pass     pass
focus survives resumption                    pass     pass
second interaction does not reinitialize     pass     pass
both streamed regions arrive                 pass     pass
out-of-order completion lands in doc order   pass     pass
placeholders replaced, not appended          pass     pass
handler failure observable                   pass     pass
```

**Ruling under the pre-registered rule**, as corrected by the project architect:

> Marko is the behavioural oracle for **resumption, DOM preservation and
> patch-placement semantics**. It is **not** the oracle for interaction-lazy
> code delivery.

Check 4 was a pre-registered property, not a footnote. Its failure in both
engines falsifies the interaction-lazy claim, so E7 is subdivided into **E7-R**
(resumption/DOM — Marko accepted), **E7-P** (patch semantics — accepted in
Chrome, Safari *timing* unmeasured) and **E7-L** (lazy loading — **no oracle**).

## Findings

**F-1 — my first version of check 2 was wrong, and wrong in the flattering
direction for a sceptic.** The `MutationObserver` is installed in `<head>`, so it
necessarily observes the browser's own parse: every element the HTML parser
appends is a `childList` mutation. Counting from install reported **72
"replacements" in the inert region** on a page that was never replayed, which
looks exactly like catastrophic component replay.

The fix is to baseline at `DOMContentLoaded` and measure the delta. After the
fix: **0 mutations in the inert region after parse**, in both engines. Node
identity independently confirms it — `sameButton`, `sameOutput`, `sameFirstRow`,
`rowCountStable` are all `true` after two interactions.

This is the same class of error as the layout spike's F-2 (a benchmark that
silently measures nothing). Both were caught only by looking at the raw numbers
and asking whether they were plausible.

**F-2 — resumption is real, and it is real in Safari too.** The first click
works in ~27 ms in both engines. The surrounding inert markup is never rebuilt.
Focus set before interaction is still on the button afterwards. A second click
increments again with zero inert-region mutations. This is the strongest evidence
in the repository for charter §8.5, and it is behavioural rather than inferred.

**F-3 — check 4 fails: Marko loads the interaction artifact eagerly.**
The earliest app script starts at **40–55 ms**, during initial page load, not on
first interaction. Marko emits `<script async type="module" src=...>` in `<head>`.

That is *async*, not *lazy*. The charter's §1.11 "interaction-lazy code" and
§8.5's "load on first interaction" are **not** satisfied by this configuration.
The cost is small here (476 B route entry + 4 KB shared runtime), but the
property does not hold, and E7 must not inherit the assumption that it does.
Recorded as a real gap rather than waved away as a configuration detail.

**F-4 — a failed handler artifact produces a silently dead control, but the
failure *is* detectable.** With the asset forced to 500:

```text
countBefore "0" -> countAfter "0"   (the control does not fake success)
jsErrors: []                        (nothing on window.onerror)
resourceErrors: [SCRIPT /assets/counter-*.js, LINK /assets/_*.js]
```

Two things matter here. First, the control does **not** pretend to work — it
simply does nothing, which is the safe failure. Second, `window.onerror` sees
**nothing**; the failure is only observable via a **capture-phase** `error`
listener, because resource-load errors do not bubble. My first version of check
10 used the bubble phase and therefore reported "silent failure, no signal" —
which would have been a materially wrong finding about the platform.

Consequence for E7: the runtime must install a capture-phase listener and surface
handler-load failure as a recoverable state. Charter §8.5's "stale manifests must
be detected and recover safely" needs an explicit, visible recovery path;
"nothing happens" is not one.

**F-5 — Safari cannot be driven by WebDriver without a manual GUI setting, so
the harness runs itself instead.** `safaridriver` is present and reports
`ready: true`, but session creation fails:

> You must enable 'Allow remote automation' in the Developer section of Safari
> Settings to control Safari via WebDriver.

That cannot be scripted. Rather than drop a first-class target (charter §11.1),
the harness gained an autorun mode: the page runs the same pre-registered checks
itself and POSTs results back to the proxy. Identical checks, different
transport.

**Honest limitation of that fallback:** autorun starts 400 ms after
`DOMContentLoaded`, by which time both streamed regions have usually landed. So
Safari's streamed **arrival timing** is not measured — only document order,
placeholder replacement, and final state. Chrome measures the timing.

## Limitations

- **Chrome and Safari only.** Firefox is untested.
- **Marko is the subject, not `pw`.** Nothing here is generated by our compiler.
- **Synthetic clicks.** `element.click()` exercises the resume path but is not a
  trusted user gesture; APIs gated on user activation are untested.
- **No back/forward cache test**, no multi-tab, no session isolation, no
  JS-disabled run, no accessibility audit. The architect's checklist item about
  back/forward navigation restoring expected state is **not** covered here.
- **Safari streaming timing unmeasured** (F-5).
- Single run per engine, one machine, localhost.
