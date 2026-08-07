// E7 gate items 7–10 — what the own renderer costs, measured on the real page.
//
// Charter §14 E7 gate:
//
// > Browser runtime size and activation CPU are measured.
// > Standard store-page interactions produce no script-attributed forced
// > synchronous layout in the supported Chromium trace harness.
// > Layout measurements and mutations appear as separate runtime phases in
// > debug traces.
// > The large-menu benchmark uses bounded DOM size through virtualization or
// > `content-visibility` where semantically correct.
//
// # The instrument, and what it can and cannot see
//
// `spikes/layout-phase-scheduler` established this project's forced-layout
// detector in E0, and also established its limit — recorded in
// `docs/evidence/E0/spike-layout-phase-scheduler.txt`:
//
//     forcedStyleAndLayoutDuration exposed by this browser: false
//
// So the attribute that would name forced layout directly is unavailable, and
// the detector E0 VALIDATED is the long-animation-frame count: a thrash
// produced 7 long frames at n=400, and the phase-scheduled version produced 0.
// That is what is reused here. Writing a fresh detector around the unexposed
// attribute is how this file's first draft reported a reassuring zero from an
// instrument that could only ever report zero.
//
// The decisive evidence is gate 9's, not gate 8's: a runtime that performs no
// layout reads at all cannot force a synchronous layout. Gate 8 corroborates
// that at the browser's own level; gate 9 proves it.
//
// Chromium only. LoAF is not implemented in Firefox or WebKit, and a detector
// that silently reports zero where it is unsupported is `RISK_QUEUE` 22 — a
// measurement that cannot fail. So this file SKIPS rather than passes there,
// and the skip is visible in the run.
//
// # Every number here has a negative control
//
// A zero from a detector that measures nothing looks exactly like a zero from a
// runtime that does no forced layout. Each measurement below is followed by a
// deliberate violation on the same page, through the same instrument, and the
// instrument must go red.

import { test, expect } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

// Its own host: `?items=1000` replaces the shared menu, which every other spec
// reasonably assumes is three coffees.
test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.performance[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

test.skip(({ browserName }) => browserName !== "chromium", "LoAF is Chromium-only");

/** Install the forced-layout detector before any of the page's own script. */
async function observe(page) {
  await page.addInitScript(() => {
    window.__loaf = [];
    window.__loafSupported =
      "PerformanceObserver" in window &&
      (PerformanceObserver.supportedEntryTypes ?? []).includes("long-animation-frame");
    if (window.__loafSupported) {
      new PerformanceObserver((list) => {
        for (const e of list.getEntries()) {
          window.__loaf.push({
            duration: e.duration,
            blocking: e.blockingDuration,
            // Recorded even though E0 measured it as unexposed here: if a
            // future browser populates it the number appears in the evidence
            // rather than having to be looked for.
            forced: e.forcedStyleAndLayoutDuration,
            scripts: (e.scripts ?? []).map((s) => s.sourceURL ?? s.name ?? ""),
          });
        }
      }).observe({ type: "long-animation-frame", buffered: true });
    }
  });
}

async function ready(page, url = "/StorePage.html") {
  await page.goto(url);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

/** The long animation frames seen since the window was last opened. */
async function longFrames(page) {
  return page.evaluate(() => ({
    supported: window.__loafSupported,
    frames: window.__loaf.length,
    blocking: window.__loaf.reduce((n, f) => n + (f.blocking ?? 0), 0),
    forcedExposed: window.__loaf.some((f) => f.forced !== undefined),
    worst: window.__loaf.reduce((n, f) => Math.max(n, f.duration), 0),
  }));
}

/** Start a fresh measurement window. */
async function reopen(page) {
  await page.evaluate(() => {
    window.__loaf.length = 0;
  });
}

/** Every byte this route made the browser download, by kind. */
async function transferred(page, run) {
  const seen = new Map();
  const on = (response) => {
    const type = response.headers()["content-type"] ?? "";
    seen.set(response.url(), { type, response });
  };
  page.on("response", on);
  await run();
  page.off("response", on);

  const out = { script: 0, wasm: 0, document: 0, other: 0, urls: [] };
  for (const [url, { type, response }] of seen) {
    let size = 0;
    try {
      size = (await response.body()).length;
    } catch {
      continue;
    }
    const kind = /javascript/.test(type)
      ? "script"
      : /wasm/.test(type)
        ? "wasm"
        : /html/.test(type)
          ? "document"
          : "other";
    out[kind] += size;
    if (kind === "script" || kind === "wasm") out.urls.push(`${kind} ${size} ${url}`);
  }
  return out;
}

test("gate 7a: the static route ships no browser runtime", async ({ page }) => {
  // The strongest form of "size measured": zero. A static page has nothing to
  // activate, so it downloads no runtime at all — and this is what makes the
  // interactive route's figure a cost of INTERACTIVITY rather than of using
  // the framework.
  const bytes = await transferred(page, async () => {
    await page.goto("/HelloStatic.html");
    await page.waitForLoadState("networkidle");
  });
  expect(bytes.script, "no script").toBe(0);
  expect(bytes.wasm, "no wasm").toBe(0);
  expect(bytes.document, "and the document is real").toBeGreaterThan(100);
  console.log(`EVIDENCE static-route-script-bytes=${bytes.script} wasm-bytes=${bytes.wasm}`);
});

test("gate 7b: the interactive route's runtime is measured", async ({ page }) => {
  const bytes = await transferred(page, () => ready(page));
  console.log(`EVIDENCE interactive-script-bytes=${bytes.script}`);
  console.log(`EVIDENCE interactive-wasm-bytes=${bytes.wasm}`);
  for (const u of bytes.urls) console.log(`EVIDENCE  ${u}`);

  // A bound, not a target. It exists so the number cannot quietly become a
  // megabyte; the figure itself is the evidence.
  expect(bytes.script + bytes.wasm).toBeLessThan(128 * 1024);
  expect(bytes.script, "the runtime really was downloaded").toBeGreaterThan(0);

  // Handler code is NOT in that figure, and that is E7-L's claim restated as a
  // size: behaviour is not part of activation.
  expect(bytes.urls.filter((u) => u.includes("/handler/")), "no handler bytes").toEqual([]);
});

test("gate 7c: activation CPU is measured", async ({ page }) => {
  await ready(page);
  const activation = await page.evaluate(() => {
    const m = performance.getEntriesByName("pw:activate")[0];
    return m ? { duration: m.duration, start: m.startTime } : null;
  });
  expect(activation, "the runtime marks its own activation").not.toBeNull();
  console.log(`EVIDENCE activation-ms=${activation.duration.toFixed(2)}`);

  // Attributed to this runtime rather than measured from outside: a stopwatch
  // around navigation would also count the network, the parser and the paint.
  expect(activation.duration).toBeGreaterThan(0);
  expect(activation.duration, "activation is not a page load").toBeLessThan(1000);
});

test("gate 8: standard interactions produce no long animation frame", async ({ page }) => {
  // The measurement and its control are ONE test, on one page, through one
  // observer — because a zero is only evidence if the same instrument, in the
  // same run, can be made to report something else. Split across two tests the
  // clean zero could be recorded while the control silently regressed, and the
  // gate would read as passed.
  await observe(page);
  await ready(page);

  // Let the page go quiet before measuring. Navigation, the wasm compile and
  // the first subscription all land in the frames right after activation, and
  // none of them is an interaction cost.
  //
  // NOT `networkidle`: the page holds a streaming subscription open on purpose,
  // so the network is never idle and waiting for it waits forever.
  await page.waitForTimeout(500);
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );

  // The window opens AFTER activation. `buffered: true` replays load-time
  // frames, and a long frame during navigation is not an interaction cost.
  await reopen(page);

  for (const n of ["1", "2", "3"]) {
    await page.locator("#menu button").first().click();
    await expect(page.locator("#cart-count")).toHaveText(n);
  }
  await page.locator("#clear-cart").click();
  await expect(page.locator("#cart-count")).toHaveText("0");
  await page.waitForTimeout(400);

  const clean = await longFrames(page);
  expect(clean.supported, "the detector is available here").toBe(true);
  console.log(
    `EVIDENCE interaction-long-frames=${clean.frames} worst-ms=${clean.worst.toFixed(1)} ` +
      `forced-attr-exposed=${clean.forcedExposed}`,
  );
  expect(clean.frames, "no interaction took a long animation frame").toBe(0);

  // Now break it deliberately, inside `requestAnimationFrame` — a long
  // ANIMATION frame is what the API reports, and work done outside a rendering
  // frame is not attributed to one.
  await reopen(page);
  await page.evaluate(
    () =>
      new Promise((done) => {
        requestAnimationFrame(() => {
          const items = [...document.querySelectorAll("#menu li")];
          const start = performance.now();
          let sink = 0;
          // Read-write-read-write on the same elements: the canonical thrash,
          // and the shape E0 measured at 7 long frames for n=400.
          while (performance.now() - start < 150) {
            for (const li of items) {
              li.style.paddingLeft = `${sink % 7}px`;
              sink += li.getBoundingClientRect().width | 0;
            }
          }
          window.__sink = sink;
          done();
        });
      }),
  );
  await page.waitForTimeout(400);

  const dirty = await longFrames(page);
  console.log(
    `EVIDENCE control-long-frames=${dirty.frames} worst-ms=${dirty.worst.toFixed(1)}`,
  );
  expect(dirty.frames, "the instrument can go red").toBeGreaterThan(0);
  expect(dirty.worst, "and the long frame really is long").toBeGreaterThan(50);
});

test("gate 9: the runtime never reads layout at all", async ({ page }) => {
  // "Measurements and mutations appear as separate phases" is trivially true
  // of a runtime that takes no measurements — and that is the honest finding
  // here, so it is stated as what it is rather than dressed up as a scheduler.
  //
  // The claim is checked by counting reads, with the counter installed before
  // any of the page's own script runs.
  await page.addInitScript(() => {
    window.__layoutReads = [];
    const record = (what) => {
      // The runtime's own frames only. A read from a test or from the browser's
      // internals is not the runtime reading layout.
      const stack = new Error().stack ?? "";
      if (stack.includes("pw-runtime")) window.__layoutReads.push(`${what}`);
    };
    for (const prop of ["offsetWidth", "offsetHeight", "clientWidth", "clientHeight"]) {
      const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, prop);
      Object.defineProperty(HTMLElement.prototype, prop, {
        get() {
          record(prop);
          return original.get.call(this);
        },
        configurable: true,
      });
    }
    const rect = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function () {
      record("getBoundingClientRect");
      return rect.call(this);
    };
  });

  await ready(page);
  for (const n of ["1", "2"]) {
    await page.locator("#menu button").first().click();
    await expect(page.locator("#cart-count")).toHaveText(n);
  }

  const reads = await page.evaluate(() => window.__layoutReads);
  console.log(`EVIDENCE runtime-layout-reads=${reads.length}`);
  expect(reads, "the runtime measures nothing, so it cannot interleave").toEqual([]);

  // The control: the counter works. Attributed to `pw-runtime` by stack, so
  // this synthesizes a frame that claims to be one.
  const seen = await page.evaluate(() => {
    const before = window.__layoutReads.length;
    const fn = new Function(
      "return document.body.getBoundingClientRect().width",
    );
    Object.defineProperty(fn, "name", { value: "pw-runtime-probe" });
    // Force a matching stack frame by evaluating from a named source.
    // eslint-disable-next-line no-eval
    (0, eval)("//# sourceURL=pw-runtime-probe.js\ndocument.body.getBoundingClientRect()");
    return window.__layoutReads.length - before;
  });
  expect(seen, "the read counter can go up").toBeGreaterThan(0);
});

test("gate 10: a thousand-item menu keeps a bounded rendering cost", async ({ page }) => {
  // The charter allows virtualization OR `content-visibility` "where
  // semantically correct". A menu is semantically a list of independent items,
  // so `content-visibility: auto` is correct: every item stays in the DOM,
  // findable and in the accessibility tree, and the ones off screen are not
  // laid out until they approach the viewport.
  //
  // Virtualization would be the wrong trade here — it removes items from the
  // document, which is a correctness cost that has to be justified per case
  // rather than adopted as a default.
  //
  // # Which cost is measured, and why that one
  //
  // BUILD AND FIRST LAYOUT of the list, which is what E0 measured
  // (`3.5x faster with contain + content-visibility` on 3,000 rows). E0 also
  // recorded the caveat, and this test's first draft walked straight into it:
  //
  // > containment does NOT help a forced layout that follows an unrelated
  // > mutation on the host element
  //
  // Timing a padding change on the container measured exactly that case, found
  // containment marginally slower, and would have reported the gate as failed
  // for a correct implementation.
  await ready(page, "/StorePage.html?items=1000");
  await expect(page.locator("#menu li")).toHaveCount(1000);

  const measured = await page.evaluate(() => {
    const source = document.querySelector("#menu");
    const declared = getComputedStyle(source.querySelector("li")).contentVisibility;

    // Build and first layout, from the page's own markup. Detached, built,
    // inserted, then read — so the number is the cost of putting a thousand
    // items on the screen rather than of touching a list already there.
    const build = (contentVisibility) => {
      const list = document.createElement("ul");
      list.style.cssText = "position:absolute;left:-99999px;width:400px";
      list.innerHTML = source.innerHTML;
      for (const li of list.querySelectorAll("li")) {
        li.style.contentVisibility = contentVisibility;
        li.style.containIntrinsicSize = "auto 42px";
      }
      const start = performance.now();
      document.body.append(list);
      void list.offsetHeight; // force the layout we are timing
      const took = performance.now() - start;
      list.remove();
      return took;
    };

    // Alternated and repeated: a single pair would be dominated by whichever
    // ran first while the style engine was cold.
    const visible = [];
    const auto = [];
    for (let i = 0; i < 5; i++) {
      visible.push(build("visible"));
      auto.push(build("auto"));
    }
    const median = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
    return {
      declared,
      items: source.querySelectorAll("li").length,
      visible: median(visible),
      auto: median(auto),
      visibleAll: visible.map((v) => Number(v.toFixed(1))),
      autoAll: auto.map((v) => Number(v.toFixed(1))),
    };
  });

  console.log(`EVIDENCE large-menu-items=${measured.items}`);
  console.log(`EVIDENCE large-menu-content-visibility=${measured.declared}`);
  console.log(
    `EVIDENCE large-menu-first-layout-ms-visible=${measured.visible.toFixed(1)} ` +
      `samples=${JSON.stringify(measured.visibleAll)}`,
  );
  console.log(
    `EVIDENCE large-menu-first-layout-ms-contained=${measured.auto.toFixed(1)} ` +
      `samples=${JSON.stringify(measured.autoAll)}`,
  );

  expect(measured.declared, "the page declares the containment it relies on").toBe("auto");
  // The uncontained case must be worth bounding, or the comparison is noise
  // dressed as a result.
  expect(measured.visible, "a thousand unbounded items cost measurable time").toBeGreaterThan(1);
  expect(measured.auto, "containment bounds the cost").toBeLessThan(measured.visible);

  // Bounded COST, not a bounded DOM: every item is still there to be found,
  // which is the correctness half of the trade.
  const findable = await page.locator("#menu li").nth(999).textContent();
  expect(findable, "the thousandth item is in the document").toBeTruthy();
});
