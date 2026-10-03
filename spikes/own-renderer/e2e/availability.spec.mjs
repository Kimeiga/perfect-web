// Charter §15.4 and §15.6 test 10: an item that sells out while its page is
// open is refused when added, by name, and the page says so (ADR-0157). The
// count it moved before the round trip goes back.
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.availability[testInfo.project.name]}`);
  },
});

// An item sold out is one per server: one test at a time.
test.describe.configure({ mode: "serial" });

async function ready(page) {
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

async function stock(request, item, available) {
  const r = await request.post(`/bench/stock?item=${item}&available=${available}`);
  expect(r.ok()).toBe(true);
}

test("an item sold out since the page was rendered is refused by name (test 10)", async ({
  page,
  request,
}) => {
  await ready(page);
  await stock(request, "cortado", false);
  try {
    // Cortado, the menu's second item, sold out after the page was rendered.
    await page.locator("#menu button").nth(1).click();
    await expect(page.locator("#cart-notice")).toHaveText("That item just sold out.");
    await expect(page.locator("#cart-count")).toHaveText("0");
    // Another item still adds, and the notice goes.
    await page.locator("#menu button").nth(0).click();
    await expect(page.locator("#cart-count")).toHaveText("1");
    await expect(page.locator("#cart-notice")).toHaveText("");
  } finally {
    await stock(request, "cortado", true);
  }
});

test("back in stock, the item adds", async ({ page, request }) => {
  await stock(request, "cortado", true);
  await ready(page);
  await page.locator("#menu button").nth(1).click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(page.locator("#cart-notice")).toHaveText("");
});
