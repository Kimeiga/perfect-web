// E7 task 1 — the golden suite, run against one renderer.
//
// The cases live in `golden.mjs` as data, so this file is the only thing that
// knows which renderer is under test. Running the suite against the own
// renderer means running this file with a different server, not editing it.
//
// `RENDERER` names the renderer for the report. It is not used to branch: a
// suite that behaved differently per renderer would be two suites.
import { test, expect } from "@playwright/test";
import { ORACLE_CASES, OWN_TARGET_CASES } from "./golden.mjs";

const RENDERER = process.env.PW_RENDERER ?? "marko";

/**
 * Load a route and collect what a case may need beyond the page.
 *
 * `requestedBeforeInteraction` is captured up to the load event and no further,
 * because "before interaction is possible" is the question — a request made
 * after the user's first click is not evidence of eager loading.
 */
async function visit(browser, c) {
  const context = await browser.newContext(
    c.jsDisabled ? { javaScriptEnabled: false } : {},
  );
  const page = await context.newPage();

  // Every request, in order, with a marker so a case can split "before the
  // first interaction" from "because of it".
  const requests = [];
  page.on("request", (r) => requests.push(r.url()));

  // `load`, not `networkidle`: a streaming route deliberately keeps its
  // connection open, so waiting for idle waits for the thing the route exists
  // to demonstrate not happening.
  const response = await page.goto(
    c.route,
    c.waitUntil ? { waitUntil: c.waitUntil } : undefined,
  );
  // The served bytes, fetched separately rather than read off the navigation
  // response. `routes.spec.mjs` records why, and this file reproduced the
  // defect before adopting it: a navigation response object is not equally
  // available in every engine, and the first version failed in Firefox while
  // the served bytes were in fact clean.
  //
  // Only when a case asks, because `response.text()` waits for the WHOLE body
  // — on a streaming route that means waiting for the slow region, and the case
  // asserting the shell arrives first would then run after it had.
  const html = c.needsHtml ? await (await context.request.get(c.route)).text() : "";
  void response;

  const isJs = (u) => u.endsWith(".js") || u.endsWith(".mjs") || u.includes("/@");
  // Settle whatever the page loads eagerly, so "before interaction" means the
  // page as it sits, not the page mid-load. Skipped for a case that asked for
  // an earlier moment — waiting for idle would wait past the thing it is
  // asserting about.
  if (!c.waitUntil) {
    await page.waitForLoadState("networkidle").catch(() => {});
  }
  const jsBefore = requests.filter(isJs);

  /** Click the page's control and return the JS requested up to that point. */
  const interactAndCollect = async () => {
    const control = page.locator("#add").first();
    if (await control.count()) {
      await control.click();
      await page.waitForTimeout(250);
    }
    return requests.filter(isJs);
  };

  return { context, page, html, jsBefore, interactAndCollect };
}

test.describe(`golden suite (${RENDERER}) — Marko is the oracle`, () => {
  for (const c of ORACLE_CASES) {
    test(`${c.id}: ${c.describes}`, async ({ browser }) => {
      const ctx = await visit(browser, c);
      try {
        await c.check(ctx.page, expect, ctx);
      } finally {
        await ctx.context.close();
      }
    });
  }
});

// Reported separately, and deliberately not as failures.
//
// RQ-1 falsified interaction-lazy loading for Marko: its interaction module
// loads during initial page load. A suite that counted this as a failure would
// be saying the oracle is broken; a suite that omitted it would let E7-L ship
// with no target. So it runs, its result is recorded, and the recorded result
// for Marko today is "does not hold".
test.describe(`the project's own targets (${RENDERER}) — no oracle`, () => {
  for (const c of OWN_TARGET_CASES) {
    test(`${c.id}: ${c.describes}`, async ({ browser }) => {
      const ctx = await visit(browser, c);
      let held = true;
      let detail = "";
      try {
        await c.check(ctx.page, expect, ctx);
      } catch (e) {
        held = false;
        detail = String(e.message ?? e).split("\n")[0];
      } finally {
        await ctx.context.close();
      }

      // The expectation is recorded per renderer, so the day the own renderer
      // holds it, this test goes red for the RIGHT reason and is promoted.
      const expected = RENDERER === "marko" ? false : true;
      expect(
        held,
        held
          ? `${c.id} now HOLDS for ${RENDERER}. If that is the own renderer, ` +
            `move this case to oracle:"marko"? No — promote it to a required ` +
            `target and update docs/evidence/E7/golden.txt.`
          : `${c.id} does not hold for ${RENDERER}: ${detail}`,
      ).toBe(expected);
    });
  }
});
