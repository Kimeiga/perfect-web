// Track `store-search` (docs/PARALLEL.md, W9): the store's search page, at
// `/search/{q}` until a route reads its query part (TEMPORARY, and no
// search box until then). The stores that match and the dishes that match,
// each dish under its store and linked to it; case ignored and accents
// kept; no text that fails the page; a menu changed while the page is open
// reaching it; and the page keeping the audit's rules.
import { expect, test } from "@playwright/test";
import { STORE_SEARCH_PORTS } from "../playwright.config.mjs";
import { violations } from "./accessibility-rules.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${STORE_SEARCH_PORTS[testInfo.project.name]}`);
  },
});

// Store 47's menu, which a test adds to, is one per server.
test.describe.configure({ mode: "serial" });

async function ready(page, path) {
  const response = await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  return response;
}

const said = (page) => page.locator("#search-said");
const stores = (page) => page.locator("#search-stores > li > h2");
const dishes = (page, store) =>
  page.getByRole("list", { name: `Dishes at ${store}` }).locator("li > a");

test("a search lists the stores that match, and their dishes under them", async ({ page }) => {
  expect((await ready(page, "/search/coffee")).status()).toBe(200);
  await expect(page).toHaveTitle("Search: coffee");
  await expect(said(page)).toHaveText("2 stores match “coffee”.");
  // A store whose name holds the term before one whose cuisine does.
  await expect(stores(page)).toHaveText(["Harbor Coffee", "Blue Bottle"]);
  await expect(dishes(page, "Harbor Coffee")).toHaveText(["Drip Coffee"]);
  await expect(dishes(page, "Blue Bottle")).toHaveText(["Cold Brew", "Cortado", "Espresso"]);
  await expect(page.locator("#search-stores .cuisine")).toHaveText(["Café, Bakery", "Coffee"]);
  // In the store's layout: its way home, and the reader's cart.
  await expect(page.locator("#header-cart-count")).toHaveText("0");
  // A dish links to its store's page.
  await dishes(page, "Harbor Coffee").first().click();
  await expect(page).toHaveURL(/\/stores\/48$/);
  await expect(page.locator("#store-name")).toHaveText("Harbor Coffee");
});

test("case is ignored, accents are not folded, and every term must match", async ({ page }) => {
  await ready(page, "/search/CAF%C3%89");
  await expect(said(page)).toHaveText("1 store matches “CAFÉ”.");
  await expect(stores(page)).toHaveText(["Harbor Coffee"]);
  await ready(page, "/search/Blueberry%20SCONE");
  await expect(dishes(page, "Harbor Coffee")).toHaveText(["Blueberry Scone"]);
  await ready(page, "/search/blueberry%20latte");
  await expect(said(page)).toHaveText("No stores or dishes match “blueberry latte”.");
  await expect(stores(page)).toHaveCount(0);
});

test("no text fails the page, and each is told", async ({ page }) => {
  for (const [path, text] of [
    ["/search/%20", "Type a store, a dish or a cuisine to search for."],
    ["/search/%25", "No stores or dishes match “%”."],
    ["/search/_", "No stores or dishes match “_”."],
    ["/search/%5C", "No stores or dishes match “\\”."],
    ["/search/'%22", "No stores or dishes match “'\"”."],
    ["/search/%3Cb%3Ex%3C%2Fb%3E", "No stores or dishes match “<b>x</b>”."],
    [`/search/${"x".repeat(101)}`, "That search is too long: 100 characters at most."],
  ]) {
    expect((await ready(page, path)).status(), path).toBe(200);
    await expect(said(page), path).toHaveText(text);
    await expect(stores(page), path).toHaveCount(0);
    await expect(page.locator("main b"), path).toHaveCount(0);
  }
});

test("a dish added while the page is open reaches it", async ({ page, request }) => {
  await ready(page, "/search/flat%20white");
  await expect(said(page)).toHaveText("No stores or dishes match “flat white”.");
  const added = await request.post("/command/menu?op=insert_after&id=flat-white&name=Flat%20White");
  expect(added.status()).toBe(202);
  try {
    await expect(said(page)).toHaveText("1 store matches “flat white”.");
    await expect(dishes(page, "Blue Bottle")).toHaveText(["Flat White"]);
  } finally {
    expect((await request.post("/command/menu?op=remove&id=flat-white")).status()).toBe(202);
  }
  await expect(said(page)).toHaveText("No stores or dishes match “flat white”.");
});

test("the search page keeps every rule, found, empty and blank", async ({ page }) => {
  for (const path of ["/search/coffee", "/search/tofu", "/search/%20"]) {
    await ready(page, path);
    expect(await page.evaluate(violations), path).toEqual([]);
  }
});
