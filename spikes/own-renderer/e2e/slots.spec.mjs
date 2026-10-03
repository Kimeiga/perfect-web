// ADR-0165, charter §15.3 and §15.6 tests 3 and 17: the store's delivery
// estimate and its recommendations come after the store's own content, in
// the same response, and hold up neither the menu nor an Add. Read by role
// and text.
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.slots[testInfo.project.name]}`);
  },
});

// The recommender is one per server: one test at a time, and each leaves it
// as it found it. The estimator is the session's, so each test has its own.
test.describe.configure({ mode: "serial" });
test.afterEach(async ({ request }) => {
  await request.post("/bench/recommendations");
});

// WebKit paints a page only once it holds about 200 characters of text, or
// has loaded (ADR-0148). The store holds fewer until it says more about
// itself and its items, as charter §15.1 asks: the next ruling. Until then,
// in Safari, the store is shown only when its slots are filled.
const WEBKIT_PAINTS_LATE = "WebKit shows the store only when its slots are filled";

const delivery = (page) => page.getByRole("region", { name: "Delivery" });
const recommendations = (page) => page.getByRole("region", { name: "Recommendations" });
// Polled, not on animation frames, Playwright's default: WebKit runs no
// frame before its first paint, and the store is not painted there until its
// slots are filled (the last test).
const ready = (page) =>
  page.waitForFunction(() => document.documentElement.dataset.pwReady, null, { polling: 50 });

test("the slots come after the store's own content (test 3)", async ({ page, request }) => {
  await request.post("/bench/recommendations?delay=1500");
  await page.goto("/stores/47", { waitUntil: "commit" });
  // The store's own content, while the recommendations are still coming.
  await expect(page.locator("#store-name")).toHaveText("Blue Bottle");
  await expect(page.locator("#menu li")).toHaveCount(3);
  await expect(recommendations(page)).toHaveText("Finding recommendations");
  // Then each slot, filled where it is. The estimate is said to a screen
  // reader when it comes.
  await expect(delivery(page)).toHaveText("Delivery in 25 min");
  await expect(delivery(page)).toHaveAttribute("aria-live", "polite");
  await expect(recommendations(page).getByRole("listitem")).toHaveText(["Cortado", "Cold Brew"]);
});

test("slow recommendations do not hold up an Add (test 17)", async ({
  page,
  request,
  browserName,
}) => {
  // A press needs a page that is shown: Playwright waits for the button to
  // be still across two frames, and a person for it to be there at all.
  test.fixme(browserName === "webkit", WEBKIT_PAINTS_LATE);
  await request.post("/bench/recommendations?delay=2500");
  await page.goto("/stores/47", { waitUntil: "commit" });
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  // The press was answered while the recommender was still answering.
  await expect(recommendations(page)).toHaveText("Finding recommendations");
  await expect(recommendations(page).getByRole("listitem")).toHaveText(["Cortado", "Cold Brew"]);
});

test("a store's recommendations are its own", async ({ page }) => {
  await page.goto("/stores/48");
  await expect(recommendations(page).getByRole("listitem")).toHaveText([
    "Matcha Latte",
    "Blueberry Scone",
  ]);
});

test("an estimate that fails says so, and the page still works", async ({ page }) => {
  await page.goto("/stores/47");
  // The page's session's estimator, down.
  const down = await page.request.post("/bench/estimate?fail=down");
  expect(down.ok()).toBe(true);
  await page.goto("/stores/47");
  await expect(delivery(page)).toHaveText("Delivery estimate unavailable");
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("the store is shown before its slots are filled", async ({ page, request, browserName }) => {
  test.fixme(browserName === "webkit", WEBKIT_PAINTS_LATE);
  await request.post("/bench/recommendations?delay=2500");
  await page.goto("/stores/47", { waitUntil: "commit" });
  await page.waitForFunction(
    () => performance.getEntriesByType("paint").some((p) => p.name === "first-contentful-paint"),
    null,
    { polling: 50, timeout: 1500 },
  );
  await expect(recommendations(page)).toHaveText("Finding recommendations");
});
