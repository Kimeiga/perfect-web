// ADR-0148: a region filled in the same response as the page it is sent
// without.
//
// `examples/demo/streamed.pw` streams its recommendations. What this shows,
// in each engine:
// - the page is usable at once: Press counts while the region shows its
//   placeholder;
// - the region fills in place when the recommender answers, and the Pick
//   buttons it holds work;
// - a failure the recommender declares, and one of the host's, show apart,
//   and a query past its budget ends the response when its budget is spent;
// - with JavaScript off, a browser without the platform's out-of-order
//   streaming keeps the placeholder. Chrome 150 and later fills it, which the
//   last test shows where the host has Chrome installed.
//
// The recommender is one per server, so this suite has a server per engine.
import { chromium, expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

const PAGE = "/page/demo.streamed.StreamedPage?id=47";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.stream[testInfo.project.name]}`);
  },
});
test.describe.configure({ mode: "serial" });

/** Set the recommender: how long it takes, and how it fails. */
async function recommender(request, query = "") {
  const r = await request.post(`/bench/recommendations?${query}`);
  expect(r.ok()).toBe(true);
}

/** The page, as soon as its runtime has started: before its region settles. */
async function opened(page) {
  await page.goto(PAGE, { waitUntil: "commit" });
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

test.afterAll(async ({ request }) => {
  await recommender(request);
});

test("the page is usable while its region is pending", async ({ page, request }) => {
  await recommender(request, "delay=2500");
  await opened(page);
  await expect(page.locator("#pending")).toHaveText("Finding recommendations");
  await page.locator("#press").click();
  await page.locator("#press").click();
  await expect(page.locator("#presses")).toHaveText("2");
  // Still pending: the press did not wait for the recommender.
  await expect(page.locator("#pending")).toBeVisible();
  await expect(page.locator("#picks li")).toHaveCount(0);
});

test("the region fills in place, and what it holds works", async ({ page, request }) => {
  await recommender(request, "delay=800");
  await opened(page);
  await expect(page.locator("#picks li")).toHaveText(["Cortado Pick", "Cold Brew Pick"], {
    useInnerText: true,
  });
  await expect(page.locator("#pending")).toHaveCount(0);
  // The patch is gone once applied, by the browser or by the runtime, and
  // the region keeps its anchors, so it is still addressable.
  await expect(page.locator("template")).toHaveCount(0);
  const region = await page.evaluate(() => {
    const comments = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) comments.push(n.data);
    return {
      anchors: comments.filter((c) => /^pw:[se]3$/.test(c)),
      markers: comments.filter((c) => c.startsWith("?")),
      settled: window.__pw.settled,
    };
  });
  expect(region.anchors).toEqual(["pw:s3", "pw:e3"]);
  expect(region.markers).toEqual([]);
  // Playwright's engines have no out-of-order streaming: the runtime filled
  // the region in each.
  expect(region.settled).toEqual([{ part: 3, by: "runtime" }]);
  // A handler in the region is bound once it has settled.
  await page.locator("#picks button").nth(1).click();
  await expect(page.locator("#picked")).toHaveText("Cold Brew");
});

test("a declared failure and the host's show apart", async ({ page, request }) => {
  await recommender(request, "delay=0&fail=declared");
  await opened(page);
  await expect(page.locator("#declared")).toHaveText("No recommendations right now");
  await recommender(request, "delay=0&fail=host");
  await opened(page);
  await expect(page.locator("#unavailable")).toHaveText("Recommendations are unavailable");
  // And the page works either way.
  await page.locator("#press").click();
  await expect(page.locator("#presses")).toHaveText("1");
});

test("a query past its budget ends the response when its budget is spent", async ({ page, request }) => {
  // Six seconds of a three-second budget.
  await recommender(request, "delay=6000");
  const started = Date.now();
  await page.goto(PAGE);
  const loaded = Date.now() - started;
  await expect(page.locator("#unavailable")).toHaveText("Recommendations are unavailable");
  expect(loaded).toBeGreaterThan(2500);
  expect(loaded).toBeLessThan(5000);
});

test("with JavaScript off, the placeholder stays where the platform cannot fill it", async ({
  browser,
  request,
  baseURL,
}) => {
  await recommender(request, "delay=300");
  const context = await browser.newContext({ javaScriptEnabled: false, baseURL });
  const page = await context.newPage();
  await page.goto(PAGE);
  await expect(page.locator("h1")).toHaveText("Blue Bottle");
  await expect(page.locator("#pending")).toHaveText("Finding recommendations");
  // The arm arrived, and waits inert in its template.
  expect(await page.locator("template").evaluate((t) => t.content.textContent)).toContain(
    "Cold Brew",
  );
  await context.close();
});

test("Chrome 150 and later fills a region itself, with JavaScript off", async ({
  request,
  baseURL,
}, testInfo) => {
  test.skip(testInfo.project.name !== "chromium", "one run: it launches the host's Chrome");
  let chrome;
  try {
    chrome = await chromium.launch({ channel: "chrome" });
  } catch {
    test.skip(true, "no Chrome installed on this host");
  }
  const major = Number(chrome.version().split(".")[0]);
  test.skip(major < 150, `Chrome ${chrome.version()} predates out-of-order streaming`);
  await recommender(request, "delay=300");
  const context = await chrome.newContext({ javaScriptEnabled: false, baseURL });
  const page = await context.newPage();
  await page.goto(PAGE);
  await expect(page.locator("#picks li")).toHaveCount(2);
  await expect(page.locator("#pending")).toHaveCount(0);
  await expect(page.locator("template")).toHaveCount(0);
  await context.close();
  // And with it on, the browser filled it, not the runtime. The runtime
  // records a region that settles after it starts, and one the browser
  // filled first is in no record: which comes first is the runner's speed
  // against the recommender's 300 ms. Until 2026-10-07 this waited for a
  // record, and timed out on a runner slow to start the runtime (run
  // 37651362024). A region still pending once the runtime has started is one
  // it saw pending, so the browser's fill is recorded; one already filled is
  // in no record, and nothing may say the runtime filled it.
  const live = await chrome.newContext({ baseURL });
  const scripted = await live.newPage();
  await scripted.goto(PAGE, { waitUntil: "commit" });
  await scripted.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  const pendingOnceStarted = await scripted.evaluate(() => !!document.querySelector("#pending"));
  await scripted.waitForLoadState("load");
  await expect(scripted.locator("#picks li")).toHaveCount(2);
  await expect(scripted.locator("template")).toHaveCount(0);
  const settled = await scripted.evaluate(() => window.__pw.settled);
  if (pendingOnceStarted) {
    expect(settled).toEqual([{ part: 3, by: "browser" }]);
  } else {
    expect(settled.filter((s) => s.by !== "browser")).toEqual([]);
  }
  // Either way, what the region holds works.
  await scripted.locator("#picks button").nth(1).click();
  await expect(scripted.locator("#picked")).toHaveText("Cold Brew");
  await chrome.close();
});
