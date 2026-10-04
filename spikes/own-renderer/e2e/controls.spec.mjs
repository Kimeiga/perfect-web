// ADR-0174, charter §15.5: deterministic test controls. A store delay and a
// cart delay, each configurable, and a one-shot database error that the
// session's next cart write, or its next cart read, meets.
//
// The store's delay is one per server, as the store is one value for every
// reader, so this suite has a server per engine.
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.controls[testInfo.project.name]}`);
  },
});
test.describe.configure({ mode: "serial" });

/** The store, ready. */
async function ready(page) {
  await page.goto("/stores/47");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** How long the store's document takes to be served to `page`'s session. */
async function served(page) {
  const started = Date.now();
  const response = await page.request.get("/stores/47");
  expect(response.ok()).toBe(true);
  return Date.now() - started;
}

test("a write the database fails: the press fails, its line goes, and the next press is one line", async ({
  page,
}) => {
  await ready(page);
  expect((await page.request.post("/bench/fail?next=write")).ok()).toBe(true);
  const add = page.getByRole("button", { name: "Add Espresso" });
  await add.click();
  await expect(page.locator("[data-pw-handler-error]")).toHaveCount(1);
  await expect(page.locator("#cart-lines li")).toHaveCount(0);
  await expect(page.locator("#cart-count")).toHaveText("0");
  // Once: the next press is written.
  await add.click();
  await expect.poll(() => page.evaluate(() => window.__pw.log.some((l) => l.startsWith("reconciled cart")))).toBe(true);
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("a read the database fails: the page cannot be shown, once", async ({ page }) => {
  await ready(page);
  expect((await page.request.post("/bench/fail?next=read")).ok()).toBe(true);
  const failed = await page.goto("/stores/47");
  expect(failed.status()).toBe(503);
  expect(await page.textContent("body")).toContain("the database is unavailable");
  const again = await page.goto("/stores/47");
  expect(again.status()).toBe(200);
});

test("a cart delay slows its own session's page, and no other's", async ({ page, browser }) => {
  await ready(page);
  const other = await browser.newContext();
  const theirs = await other.newPage();
  await theirs.goto("/stores/47");
  try {
    expect((await page.request.post("/bench/cart?delay=1500")).ok()).toBe(true);
    expect(await served(page)).toBeGreaterThanOrEqual(1500);
    expect(await served(theirs)).toBeLessThan(1500);
    expect((await page.request.post("/bench/cart?delay=0")).ok()).toBe(true);
    expect(await served(page)).toBeLessThan(1500);
  } finally {
    await other.close();
  }
});

test("the store's delay slows every page that reads the store, until it is cleared", async ({
  page,
  browser,
}) => {
  const other = await browser.newContext();
  const theirs = await other.newPage();
  try {
    expect((await page.request.post("/bench/store?delay=1500")).ok()).toBe(true);
    expect(await served(page)).toBeGreaterThanOrEqual(1500);
    // The store is kept for its freshness, for every reader. Set again, the
    // control drops what was kept, and another session's read waits too:
    // the delay is the store's, not a session's.
    expect((await page.request.post("/bench/store?delay=1500")).ok()).toBe(true);
    expect(await served(theirs)).toBeGreaterThanOrEqual(1500);
  } finally {
    expect((await page.request.post("/bench/store?delay=0")).ok()).toBe(true);
    await other.close();
  }
  expect(await served(page)).toBeLessThan(1500);
});
