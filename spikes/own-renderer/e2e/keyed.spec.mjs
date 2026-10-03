// ADR-0152: a key a page changes, in three engines. The store with T07's
// category tabs, `on_key_change cancel` (`keyed-store.sh`), on a host per
// engine. The charter's store tests (§15.6):
//   6. Same query key under repeated local recomputation makes one request.
//   7. Changing the query key cancels or supersedes stale work.
//   8. Navigating away cancels unneeded requests.
import { expect, test } from "@playwright/test";
import { KEYED_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${KEYED_PORTS[testInfo.project.name]}`);
  },
});

// Which category is slow is one per server: one test at a time.
test.describe.configure({ mode: "serial" });

const PAGE = "/StorePage.html";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

const tab = (page, name) =>
  page.getByRole("navigation", { name: "Categories" }).getByRole("button", { name });
const browsed = (page) => page.locator("#browse li");

/** Make a category slow to read on this engine's host. */
async function slow(request, category, delay) {
  const r = await request.post(`/bench/category?slow=${category}&delay=${delay}`);
  expect(r.ok()).toBe(true);
}

/** How many reads of a category the host stopped part way. */
async function stopped(request) {
  return (await (await request.get("/bench/calls")).json()).category_stopped;
}

test("a changed key's old read is stopped, and its answer never shown (test 7)", async ({
  page,
  request,
}) => {
  await slow(request, "hot", 1500);
  const before = await stopped(request);
  const aborted = [];
  page.on("requestfailed", (r) => aborted.push(decodeURIComponent(r.url())));
  await ready(page);
  await tab(page, "Hot").click();
  // Hot's read is under way.
  await page.waitForTimeout(300);
  await tab(page, "Cold").click();
  await expect(browsed(page)).toHaveText(["Cold Brew"], { timeout: 1_000 });
  // Hot's request aborted in the browser, and its read stopped on the host.
  await expect.poll(() => aborted.some((url) => url.includes('"hot"'))).toBe(true);
  await expect.poll(() => stopped(request)).toBeGreaterThan(before);
  // Still Cold's, once Hot would have answered.
  await page.waitForTimeout(1_500);
  await expect(browsed(page)).toHaveText(["Cold Brew"]);
});

test("a key the page has is not asked for again (test 6)", async ({ page, request }) => {
  await slow(request, "", 0);
  await ready(page);
  const reads = [];
  page.on("request", (r) => {
    if (new URL(r.url()).pathname === "/pw-read") reads.push(r.url());
  });
  await tab(page, "Cold").click();
  await expect(browsed(page)).toHaveText(["Cold Brew"]);
  await tab(page, "Cold").click();
  await tab(page, "Cold").click();
  await page.waitForTimeout(500);
  expect(reads).toHaveLength(1);
});

test("leaving the page stops its read (test 8)", async ({ page, request }) => {
  await slow(request, "hot", 2000);
  const before = await stopped(request);
  await ready(page);
  await tab(page, "Hot").click();
  // Hot's read is under way.
  await page.waitForTimeout(300);
  await page.goto("about:blank");
  // The host found the request gone, and let go of its read.
  await expect.poll(() => stopped(request), { timeout: 3_000 }).toBeGreaterThan(before);
});
