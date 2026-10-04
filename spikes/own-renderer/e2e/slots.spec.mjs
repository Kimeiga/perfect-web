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

const delivery = (page) => page.getByRole("region", { name: "Delivery" });
const recommendations = (page) => page.getByRole("region", { name: "Recommendations" });
// Polled, not on animation frames, Playwright's default: WebKit runs no
// frame before its first paint, which a page that says too little does not
// have until it has loaded (the last test).
const ready = (page) =>
  page.waitForFunction(() => document.documentElement.dataset.pwReady, null, { polling: 50 });

test("the slots come after the store's own content (test 3)", async ({ page, request }) => {
  await request.post("/bench/recommendations?delay=2500");
  await page.goto("/stores/47", { waitUntil: "commit" });
  // The store's own content, while the recommendations are still coming:
  // the placeholder first, so a slow engine under load does not race it.
  await expect(recommendations(page)).toHaveText("Finding recommendations");
  await expect(page.locator("#store-name")).toHaveText("Blue Bottle");
  await expect(page.locator("#menu li")).toHaveCount(3);
  // Then each slot, filled where it is. The estimate is said to a screen
  // reader when it comes: a range, in words (ADR-0180).
  await expect(delivery(page)).toHaveText("Delivery in 25 to 35 min");
  await expect(delivery(page)).toHaveAttribute("aria-live", "polite");
  await expect(recommendations(page).getByRole("listitem")).toHaveText(["Cortado", "Cold Brew"]);
});

test("slow recommendations do not hold up an Add (test 17)", async ({ page, request }) => {
  // A press needs a page that is shown: Playwright waits for the button to
  // be still across two frames, and a person for it to be there at all.
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

test("an estimate is a range, and one of no minutes is refused", async ({ page }) => {
  // ADR-0180: the least and the most minutes, in words. Each is a
  // `PositiveInt`, so an estimator's answer of 0 breaks its invariant, which
  // the host refuses as a failed read (ADR-0179): the slot says so.
  await page.goto("/stores/47");
  const ranged = await page.request.post("/bench/estimate?minutes=15&max=45&delay=0");
  expect(ranged.ok()).toBe(true);
  await page.goto("/stores/47");
  await expect(delivery(page)).toHaveText("Delivery in 15 to 45 min");
  const none = await page.request.post("/bench/estimate?minutes=0&delay=0");
  expect(none.ok()).toBe(true);
  await page.goto("/stores/47");
  await expect(delivery(page)).toHaveText("Delivery estimate unavailable");
});

test("the store is shown before its slots are filled", async ({ page, request }) => {
  // WebKit paints a page while it loads only once it holds more than 200
  // characters of text, or 32 by 32 pixels of an image (WebKit's
  // `LocalFrameView`). The store says enough of itself and its items to
  // (ADR-0166); until it did, Safari showed it only when its slots were
  // filled.
  await request.post("/bench/recommendations?delay=2500");
  await page.goto("/stores/47", { waitUntil: "commit" });
  await page.waitForFunction(
    () => performance.getEntriesByType("paint").some((p) => p.name === "first-contentful-paint"),
    null,
    { polling: 50, timeout: 1500 },
  );
  await expect(recommendations(page)).toHaveText("Finding recommendations");
});
