/* spike: browser-resumption — in-page instrumentation.
 *
 * Injected by proxy.mjs as the FIRST script in <head>, classic (non-deferred),
 * so it runs before any framework code and before <body> is parsed.
 *
 * This file only *observes*. It never patches framework internals, so the thing
 * under test is the real application, not a rewritten one.
 *
 * The pre-registered checks come from the architect ruling and are numbered to
 * match it. `window.__pw.run()` executes them and returns a plain object the
 * driver reads over CDP (Chrome) or WebDriver (Safari).
 */
(function () {
  "use strict";

  const pw = (window.__pw = {
    installedAt: performance.now(),
    readyStateAtInstall: document.readyState,
    // childList mutations, split by whether they touched the inert region
    mutations: { total: 0, inertRegion: 0, cartRegion: 0, records: [] },
    resources: [],
    errors: [],
    resourceErrors: [],
    unhandledRejections: [],
  });

  window.addEventListener("error", (e) =>
    pw.errors.push(String(e.message || e.error || "error")),
  );
  // Resource-load failures (a <script> that 404s/500s) do NOT bubble and do not
  // reach the bubble-phase handler above. They are only observable in the
  // CAPTURE phase. Check 10 depends on this: without it, a dead interaction
  // artifact looks identical to a page with no errors at all.
  window.addEventListener(
    "error",
    (e) => {
      const t = e.target;
      if (t && t !== window && (t.tagName === "SCRIPT" || t.tagName === "LINK" || t.tagName === "IMG")) {
        pw.resourceErrors.push({ tag: t.tagName, src: String(t.src || t.href || "").replace(location.origin, "") });
      }
    },
    true,
  );
  window.addEventListener("unhandledrejection", (e) =>
    pw.unhandledRejections.push(String(e.reason)),
  );

  // --- resource timing: WHEN does the interaction artifact actually load? ----
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        if (e.initiatorType === "script" || /\.js(\?|$)/.test(e.name)) {
          pw.resources.push({
            name: e.name.replace(location.origin, ""),
            startTime: Math.round(e.startTime),
            responseEnd: Math.round(e.responseEnd),
            transferSize: e.transferSize,
            initiatorType: e.initiatorType,
          });
        }
      }
    }).observe({ type: "resource", buffered: true });
  } catch (err) {
    pw.resourceObserverError = String(err);
  }

  // --- mutation observation: does anything get REPLACED during startup? -----
  // Charter §8.5: resumption must not re-execute the component tree. If it did,
  // the inert markup would be rebuilt and we would see childList mutations here.
  const startObserving = () => {
    const mo = new MutationObserver((records) => {
      for (const r of records) {
        if (r.type !== "childList") continue;
        const added = r.addedNodes.length;
        const removed = r.removedNodes.length;
        if (added === 0 && removed === 0) continue;
        pw.mutations.total += added + removed;
        const target = r.target;
        const inInert = !!(target.closest && target.closest('[aria-label="Store information"]'));
        const inCart = !!(target.closest && target.closest('[aria-label="Cart"]'));
        if (inInert) pw.mutations.inertRegion += added + removed;
        if (inCart) pw.mutations.cartRegion += added + removed;
        if (pw.mutations.records.length < 40) {
          pw.mutations.records.push({
            target: target.nodeName + (target.id ? "#" + target.id : ""),
            added,
            removed,
            at: Math.round(performance.now()),
          });
        }
      }
    });
    mo.observe(document.documentElement, { childList: true, subtree: true });
    pw.observer = mo;
  };
  startObserving();

  const $ = (sel) => document.querySelector(sel);
  const text = (sel) => { const el = $(sel); return el ? el.textContent.trim() : null; };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  /** Wait until `fn()` is truthy, or time out. Returns ms waited, or -1. */
  async function waitFor(fn, timeoutMs = 4000, step = 25) {
    const t0 = performance.now();
    while (performance.now() - t0 < timeoutMs) {
      let v = false;
      try { v = fn(); } catch { v = false; }
      if (v) return Math.round(performance.now() - t0);
      await sleep(step);
    }
    return -1;
  }

  // -------------------------------------------------------------------------
  // The counter route test — checks 1-8, 10
  // -------------------------------------------------------------------------
  pw.runCounter = async function runCounter(opts) {
    opts = opts || {};
    const out = { route: "counter", checks: {}, observed: {} };

    // --- 1. the page is usable before interaction code has loaded -----------
    // Recorded at install time, before <body> existed, plus the state now.
    out.observed.readyStateAtHarnessInstall = pw.readyStateAtInstall;
    out.observed.contentPresentAtDomReady = pw.contentAtDomReady;
    out.checks["1-html-before-js"] =
      pw.contentAtDomReady && pw.contentAtDomReady.hasButton && pw.contentAtDomReady.hasMenu;

    // Node identity BEFORE any interaction (check 6).
    const button = $("#add");
    const output = $("#count");
    const inertRows = Array.from(document.querySelectorAll('[aria-label="Menu"] li'));
    const identityBefore = { button, output, firstRow: inertRows[0], rowCount: inertRows.length };

    // --- 2. no component/template replay during startup --------------------
    //
    // MEASUREMENT NOTE (finding F-1): the observer is installed in <head>, so it
    // necessarily sees the browser's own initial parse — every element the HTML
    // parser appends is a childList mutation. Counting from install would report
    // ~72 "replacements" in the inert region on a page that was never replayed.
    // That is parsing, not replay.
    //
    // Replay is what happens AFTER the document has been parsed: the framework
    // re-executing the component tree and rebuilding markup the server already
    // sent. So the measurement starts at DOMContentLoaded.
    out.observed.mutationsDuringInitialParse = pw.mutations.inertRegion;
    const postParse = {
      total: pw.mutations.total - (pw.mutationsAtDomReady?.total ?? 0),
      inertRegion: pw.mutations.inertRegion - (pw.mutationsAtDomReady?.inertRegion ?? 0),
    };
    out.observed.mutationsAfterParse = postParse;
    out.checks["2-no-component-replay"] = postParse.inertRegion === 0;

    // --- 4/5. which scripts loaded, and when -------------------------------
    // Give async module scripts a chance to arrive.
    await sleep(300);
    out.observed.scripts = pw.resources.slice();
    const appScripts = pw.resources.filter((r) => /\/assets\//.test(r.name));
    out.observed.appScriptCount = appScripts.length;
    out.checks["5-only-expected-artifacts"] =
      appScripts.length > 0 && appScripts.every((r) => /counter|_[A-Za-z0-9]+\.js/.test(r.name));

    // Check 4 asks whether the handler artifact is ABSENT until needed.
    // Recorded as an observation with the load time, not assumed either way.
    out.observed.handlerLoadedBeforeInteraction = appScripts.length > 0;
    out.observed.earliestAppScriptStart = appScripts.length
      ? Math.min(...appScripts.map((r) => r.startTime))
      : null;
    out.checks["4-handler-lazy-until-needed"] = appScripts.length === 0;

    if (opts.breakHandler) {
      // --- 10. handler load failure is observable, not silent -------------
      const before = text("#count");
      button && button.click();
      await sleep(600);
      const after = text("#count");
      const failureSignals = pw.errors.length + pw.resourceErrors.length;
      out.observed.brokenHandler = {
        countBefore: before,
        countAfter: after,
        controlStillWorks: before !== after,
        jsErrors: pw.errors.slice(),
        resourceErrors: pw.resourceErrors.slice(),
        failureSignals,
      };
      // Two separable properties, reported independently so neither can hide
      // the other:
      //   (a) did the control silently appear to work? (it must not)
      //   (b) was the failure observable to the page at all?
      out.checks["10a-broken-handler-does-not-fake-success"] = before === after;
      out.checks["10-handler-failure-observable"] = before === after && failureSignals > 0;
      out.errors = pw.errors.slice();
      return out;
    }

    // --- 7. focus survives resumption --------------------------------------
    button && button.focus();
    const focusedBefore = document.activeElement === button;

    // --- 3. the first click actually invokes the handler -------------------
    const countBefore = text("#count");
    button && button.click();
    const waited1 = await waitFor(() => text("#count") !== countBefore, 4000);
    const countAfter1 = text("#count");
    out.observed.firstClick = { countBefore, countAfter1, waitedMs: waited1 };
    out.checks["3-first-click-works"] = waited1 >= 0 && countAfter1 !== countBefore;

    out.observed.focus = {
      focusedBefore,
      focusedAfter: document.activeElement === button,
      activeElement: document.activeElement ? document.activeElement.id || document.activeElement.nodeName : null,
    };
    out.checks["7-focus-survives"] = focusedBefore && document.activeElement === button;

    // --- 6. DOM nodes were retained, not replaced --------------------------
    const inertRowsAfter = Array.from(document.querySelectorAll('[aria-label="Menu"] li'));
    out.observed.nodeIdentity = {
      sameButton: $("#add") === identityBefore.button,
      sameOutput: $("#count") === identityBefore.output,
      sameFirstRow: inertRowsAfter[0] === identityBefore.firstRow,
      rowCountStable: inertRowsAfter.length === identityBefore.rowCount,
    };
    out.checks["6-dom-nodes-retained"] =
      out.observed.nodeIdentity.sameButton &&
      out.observed.nodeIdentity.sameFirstRow &&
      out.observed.nodeIdentity.rowCountStable;

    // --- 8. a second interaction does not reinitialize ---------------------
    const mutationsBeforeSecond = pw.mutations.inertRegion;
    button && button.click();
    const waited2 = await waitFor(() => text("#count") !== countAfter1, 4000);
    const countAfter2 = text("#count");
    out.observed.secondClick = {
      countAfter2,
      waitedMs: waited2,
      inertMutationsDuring: pw.mutations.inertRegion - mutationsBeforeSecond,
    };
    out.checks["8-second-interaction-no-reinit"] =
      waited2 >= 0 &&
      countAfter2 !== countAfter1 &&
      pw.mutations.inertRegion === mutationsBeforeSecond;

    out.observed.totalMutations = { ...pw.mutations, records: pw.mutations.records.slice(0, 10) };
    out.errors = pw.errors.slice();
    out.unhandledRejections = pw.unhandledRejections.slice();
    return out;
  };

  // -------------------------------------------------------------------------
  // The streamed route test — check 9
  // -------------------------------------------------------------------------
  pw.runStream = async function runStream() {
    const out = { route: "stream", checks: {}, observed: {} };

    // The shell must be usable while both subtrees are still pending.
    out.observed.shellAtInstall = pw.contentAtDomReady || null;

    const estimateAt = await waitFor(() => /ESTIMATE_READY/.test(document.body.textContent), 6000);
    const recsAt = await waitFor(() => /RECS_READY/.test(document.body.textContent), 8000);

    out.observed.arrivalMs = { estimate: estimateAt, recommendations: recsAt };

    // Document ORDER must be correct even though the 400ms region completed
    // before the 1200ms one — that is what out-of-order patching means.
    const bodyText = document.body.textContent;
    const iShell = bodyText.indexOf("SHELL_READY");
    const iEstimate = bodyText.indexOf("ESTIMATE_READY");
    const iRecs = bodyText.indexOf("RECS_READY");
    out.observed.documentOrder = { iShell, iEstimate, iRecs };

    out.checks["9-out-of-order-patch-lands-in-order"] =
      iShell >= 0 && iEstimate > iShell && iRecs > iEstimate;
    out.checks["9-both-regions-arrived"] = estimateAt >= 0 && recsAt >= 0;
    // Placeholders must be gone, not merely appended after.
    out.observed.placeholdersRemaining =
      (bodyText.match(/Estimating delivery|Loading recommendations/g) || []).length;
    out.checks["9-placeholders-replaced"] = out.observed.placeholdersRemaining === 0;

    out.errors = pw.errors.slice();
    return out;
  };

  // Capture what the document looked like the moment parsing finished, i.e.
  // before any async module script could have run.
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      pw.contentAtDomReady = {
        at: Math.round(performance.now()),
        hasButton: !!document.querySelector("#add"),
        hasMenu: !!document.querySelector('[aria-label="Menu"] li'),
        hasHeading: !!document.querySelector("h1"),
        countText: (document.querySelector("#count") || {}).textContent || null,
        scriptsSoFar: pw.resources.length,
      };
      // Baseline for check 2: everything before this point is the browser
      // parsing the server's HTML, not the framework replaying anything.
      pw.mutationsAtDomReady = {
        total: pw.mutations.total,
        inertRegion: pw.mutations.inertRegion,
        cartRegion: pw.mutations.cartRegion,
      };
    },
    { once: true },
  );

  // -------------------------------------------------------------------------
  // Self-running mode, for browsers we cannot drive over a protocol.
  //
  // Safari's WebDriver endpoint requires "Allow Remote Automation" to be enabled
  // by hand in Safari Settings > Developer, which cannot be scripted. Rather than
  // skip Safari — a first-class target per charter §11.1 — the page runs the same
  // pre-registered checks itself and POSTs the result back to the proxy.
  //
  // The checks are identical. Only the transport differs.
  // -------------------------------------------------------------------------
  const params = new URLSearchParams(location.search);
  if (params.has("autorun")) {
    const which = location.pathname.includes("stream") ? "stream" : "counter";
    const broken = params.has("breakHandler");
    const go = async () => {
      let payload;
      try {
        const res =
          which === "stream"
            ? await pw.runStream()
            : await pw.runCounter({ breakHandler: broken });
        payload = { ok: true, engine: params.get("engine") || "unknown", ...res };
      } catch (e) {
        payload = { ok: false, engine: params.get("engine") || "unknown", route: which, error: String(e) };
      }
      payload.userAgent = navigator.userAgent;
      try {
        await fetch("/__result", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(payload),
        });
      } catch (e) {
        /* nothing more we can do from here */
      }
      document.title = "pw-done:" + which;
    };
    if (document.readyState === "loading") {
      document.addEventListener("DOMContentLoaded", () => setTimeout(go, 400), { once: true });
    } else {
      setTimeout(go, 400);
    }
  }
})();
