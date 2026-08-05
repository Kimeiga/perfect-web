# spike: layout-phase-scheduler

**Charter reference:** §14 Milestone 0 task 13, §7.5A, and the §20 risk
*"Fine-grained updates are mistaken for layout safety"* — which says explicitly:
**"build forced-layout instrumentation in Milestone 0"**.

## Question

Charter §7.5A opens with the claim that motivates the whole layout-phase model:

> *"Fine-grained DOM updates do not by themselves prevent forced synchronous
> layout."*

Can this project **detect** forced synchronous layout before it tries to prevent
it, and does phase scheduling measurably fix it?

## Run it

```bash
just spike-layout
# or directly:
node spikes/layout-phase-scheduler/measure.mjs
```

Evidence: `docs/evidence/M0/spike-layout-phase-scheduler.txt`.

The harness drives **headless Chrome over the DevTools Protocol** using Node 22's
built-in `WebSocket` and `fetch` — no Playwright, no Puppeteer, no dependency at
all. Set `CHROME_PATH` to use a different Chromium build.

## What is under test

```text
public/thrash.html     interleaves geometry reads with layout-invalidating writes
public/phased.html     batches all reads, plans, then commits all writes
public/observers.html  ResizeObserver, a deliberate feedback loop, and containment
```

`thrash.html` and `phased.html` do **identical work** — same element count, same
arithmetic, same written values — and the harness asserts they produce the same
checksum. The only difference is the phase discipline.

## Results

### 1. Forced synchronous layout is enormous, and phase scheduling removes it

```text
elements   mode     median ms   checksum
400        thrash   79.1        59600
400        phased    0.3        59600     -> 264x faster
1200       thrash  678.6       178800
1200       phased    0.8       178800     -> 848x faster
```

Identical checksums, so this is not a case of doing less work. It is the same
work, ordered correctly.

Note the **superlinear** growth in the thrashing case: 3× the elements costs
8.6× the time, because each of N reads forces a layout over a document that is
itself N elements large. Phase scheduling is flat.

### 2. Long Animation Frame attribution — with an important caveat

```text
n=1200  thrash  longFrames=7  duration=687.4ms  blockingDuration=637.3ms
n=1200  phased  longFrames=0  (no frame exceeded the 50ms LoAF threshold)

forcedStyleAndLayoutDuration exposed by this browser: FALSE
```

**The charter's preferred signal is not available.** Chrome 150 reports
`long-animation-frame` entries with `duration` and `blockingDuration`, but
`forcedStyleAndLayoutDuration` is `undefined`. The charter anticipated this —
task 13 says *"and, where available"* — so the fallback is recorded rather than
the gap being hidden.

**The usable proxy is the long-frame count plus `blockingDuration`.** It is a
clean signal here: 7 long frames versus 0.

### 3. ResizeObserver feedback loops are detectable

```text
resizeObserver deliveries      : 1
feedback-loop iterations       : 30
browser reported the RO loop   : true
check:feedback-loop-detectable=pass
```

An observer callback that resizes the element it observes causes the browser to
emit `ResizeObserver loop completed with undelivered notifications` as a window
`error` event. Charter §7.5A wants the runtime to *"warn on feedback loops where
a resize observation immediately changes the observed size"* — this proves the
signal exists to warn from.

### 4. Containment helps — but not where you would first guess

```text
build + first layout of 3000 rows
  plain     median 23.8 ms
  contained median  5.7 ms      -> 4.18x faster
```

with `contain: layout style paint` + `content-visibility: auto` +
`contain-intrinsic-size`.

## Findings

**F-1 — the charter's §7.5A premise is correct and the effect size is large.**
264×–848× on identical work. This is not a micro-optimization; a page that
thrashes is qualitatively broken, and the difference between the two versions is
purely *ordering*. That justifies making the phase distinction a language-level
effect rather than a style guideline.

**F-2 — a naive layout benchmark measures nothing after the first run.**
The first version of `thrash.html` wrote `paddingLeft` derived only from the
measured width. On run 2 the computed value was identical, so **the write did not
invalidate layout**, and the measurement collapsed from 79.7 ms to 0.3 ms — which
looks exactly like a successful optimization. Fixed by salting the written value
per run. Any future layout regression gate must ensure its writes actually dirty
layout, or it will silently pass.

**F-3 — `forcedStyleAndLayoutDuration` is not exposed in Chrome 150.**
The `long-animation-frame` entry type is supported and `buffered: true` works,
but the forced-layout-specific field is `undefined`. Milestone 0's instrumentation
therefore relies on long-frame count and `blockingDuration`. If a later Chrome
exposes the field, the harness will pick it up automatically — it already probes
for it and prints `forcedStyleAndLayoutDuration exposed by this browser`.

**F-4 — containment does NOT help the case people reach for it first.**
Measured both ways:

| case | plain | contained | verdict |
|---|---|---|---|
| forced layout after an unrelated mutation on the host | 0.6 ms | 1.0 ms | contained is **slower** |
| build + first layout of a 3,000-row subtree | 23.8 ms | 5.7 ms | contained is **4.18× faster** |

Containment constrains where layout work *propagates*; it does not make an
already-forced document-wide flush cheap, and it adds per-element bookkeeping.
This matters for charter §7.5A's instruction to *"generate CSS `contain` or
`content-visibility` only when subtree independence is semantically valid"* —
applying it indiscriminately can lose performance as well as correctness.

**F-5 — a runtime-enforced phase API is enough to get the benefit; compile-time
proof is a separate, later win.** `FrameScheduler` in `phased.html` separates
`measure()` from `mutate()` by API shape alone and captures the entire 848×. This
supports the charter's own fallback position: *"keep the phase scheduler as a
runtime-enforced API even if compile-time proof is initially incomplete."*
The compiler's job is then to make the unsafe path *unrepresentable*, not to make
the safe path fast — it is already fast.

**F-6 — Node 22 can drive Chrome over CDP with zero dependencies.** Built-in
`WebSocket` plus `Target.createTarget` / `Runtime.evaluate` was sufficient for
everything here. Useful for Milestone 3: Playwright is needed for cross-engine
testing (WebKit, Firefox), but Chromium-only performance instrumentation does not
require it.

## Limitations

- **Chromium only.** No WebKit or Firefox. Safari's layout behavior and its lack
  of LoAF are untested, and Safari is a first-class target per charter §11.1.
- **Headless, no CPU throttling, no network shaping, single machine.** Charter
  §18.5 requires sample counts and distributions before publishing benchmarks;
  these are detection baselines, not benchmark results.
- **No Chrome performance *trace* is parsed.** The harness uses in-page
  `performance.now()` and the LoAF observer. A `Tracing.start`/`Tracing.end` CDP
  capture with `disabled-by-default-devtools.timeline` categories would give
  per-`Layout` event attribution and is the natural next step, but the long-frame
  signal was sufficient to answer Milestone 0's question.
- **The phase scheduler is a toy.** No cancellation on unmount, no coalescing by
  document part, no integration with any renderer. Charter §7.5A lists eleven
  further obligations; this implements two (read/write ordering, measurement
  deduplication).
- **Nothing here is connected to `pw`.** These are hand-written HTML pages. The
  language-level effects `layout.measure` / `style.mutate<LayoutAffect>` do not
  exist yet.
