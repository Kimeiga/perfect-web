// Charter §15.4 and §15.6 test 10: an item that sells out while its page is
// open is refused when added, by name, and the page says so (ADR-0157). The
// count it moved before the round trip goes back.
//
// Charter §15.1 and §15.2: whether an item can be ordered is shown before the
// press, and a change to it, told, reaches every page open (ADR-0178). Untold,
// it is §15.5's forced stale item, which the first test is.
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

// Untold unless `tell`: then `InventoryChanged(47, item)` (ADR-0178).
async function stock(request, item, available, tell = false) {
  const told = tell ? "&tell=true" : "";
  const r = await request.post(`/bench/stock?item=${item}&available=${available}${told}`);
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

test("a sold-out item is shown so, and has no Add to press (§15.1)", async ({ page, request }) => {
  await stock(request, "cortado", false, true);
  try {
    await ready(page);
    const cortado = page.locator("#menu li").nth(1);
    await expect(cortado).toContainText("Sold out");
    await expect(cortado.locator("button")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Add Cortado" })).toHaveCount(0);
    // The others are as they were.
    await expect(page.getByRole("button", { name: "Add Espresso" })).toHaveCount(1);
    await expect(page.getByRole("button", { name: "Add Cold Brew" })).toHaveCount(1);
    await expect(page.locator("#menu li").filter({ hasText: "Sold out" })).toHaveCount(1);
  } finally {
    await stock(request, "cortado", true, true);
  }
});

test("a page open when an item sells out is told, and only its row changes (§15.2)", async ({
  page,
  request,
}) => {
  await ready(page);
  // Each row's node marked, to see which are kept.
  await page.locator("#menu li").evaluateAll((lis) => lis.forEach((li, i) => (li.__mark = i)));
  const espresso = page.getByRole("button", { name: "Add Espresso" });
  await espresso.focus();
  await stock(request, "cortado", false, true);
  try {
    await expect(page.locator("#menu li").nth(1)).toContainText("Sold out");
    await expect(page.getByRole("button", { name: "Add Cortado" })).toHaveCount(0);
    const marks = await page.locator("#menu li").evaluateAll((lis) => lis.map((li) => li.__mark ?? null));
    expect(marks, "only the sold-out item's row is rendered again").toEqual([0, null, 2]);
    // Focus is where it was (test 15), and the others still add.
    await expect(espresso).toBeFocused();
    await espresso.click();
    await expect(page.locator("#cart-count")).toHaveText("1");
  } finally {
    await stock(request, "cortado", true, true);
  }
  // Back in stock, told: its Add comes back, and adds.
  const cortado = page.getByRole("button", { name: "Add Cortado" });
  await expect(cortado).toHaveCount(1);
  await cortado.click();
  await expect(page.locator("#cart-count")).toHaveText("2");
  await expect(page.locator("#cart-notice")).toHaveText("");
});
