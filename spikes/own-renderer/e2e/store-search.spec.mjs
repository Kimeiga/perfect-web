// Track `store-search` (ADR-0308; its address ADR-0309's): the store's
// search page at `/search{?q}`, its search box a plain GET form. The stores
// that match and the dishes that match, each dish under its store and linked
// to it; the form with scripts on and off; no `q`, an empty one, `+` and a
// repeated `q`; case ignored and accents kept; no text that fails the page;
// a menu changed while the page is open reaching it; and the page keeping
// the audit's rules.
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
  expect((await ready(page, "/search?q=coffee")).status()).toBe(200);
  await expect(page).toHaveTitle("Search for “coffee”");
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
  await ready(page, "/search?q=CAF%C3%89");
  await expect(said(page)).toHaveText("1 store matches “CAFÉ”.");
  await expect(stores(page)).toHaveText(["Harbor Coffee"]);
  await ready(page, "/search?q=Blueberry+SCONE");
  await expect(dishes(page, "Harbor Coffee")).toHaveText(["Blueberry Scone"]);
  await ready(page, "/search?q=blueberry%20latte");
  await expect(said(page)).toHaveText("No stores or dishes match “blueberry latte”.");
  await expect(stores(page)).toHaveCount(0);
});

test("no text fails the page, and each is told", async ({ page }) => {
  for (const [path, text] of [
    ["/search?q=+", "Type a store, a dish or a cuisine to search for."],
    ["/search?q=%25", "No stores or dishes match “%”."],
    ["/search?q=_", "No stores or dishes match “_”."],
    ["/search?q=%5C", "No stores or dishes match “\\”."],
    ["/search?q='%22", "No stores or dishes match “'\"”."],
    ["/search?q=%3Cb%3Ex%3C%2Fb%3E", "No stores or dishes match “<b>x</b>”."],
    [`/search?q=${"x".repeat(101)}`, "That search is too long: 100 characters at most."],
  ]) {
    expect((await ready(page, path)).status(), path).toBe(200);
    await expect(said(page), path).toHaveText(text);
    await expect(stores(page), path).toHaveCount(0);
    await expect(page.locator("main b"), path).toHaveCount(0);
  }
});

test("a dish added while the page is open reaches it", async ({ page, request }) => {
  await ready(page, "/search?q=flat%20white");
  await expect(said(page)).toHaveText("No stores or dishes match “flat white”.");
  // The sentence is inside the `Some` arm of `{#match q}`, and is told
  // where it is (ADR-XXXX): the status region is the element it was, so a
  // screen reader hears its new text. Marked BEFORE the change.
  await page.evaluate(() => {
    document.querySelector("#search-said").__pwIdentity = "said";
  });
  const kept = () => page.evaluate(() => document.querySelector("#search-said")?.__pwIdentity);
  const added = await request.post("/command/menu?op=insert_after&id=flat-white&name=Flat%20White");
  expect(added.status()).toBe(202);
  try {
    await expect(said(page)).toHaveText("1 store matches “flat white”.");
    await expect(dishes(page, "Blue Bottle")).toHaveText(["Flat White"]);
    expect(await kept(), "the status region is the one it was").toBe("said");
  } finally {
    expect((await request.post("/command/menu?op=remove&id=flat-white")).status()).toBe(202);
  }
  await expect(said(page)).toHaveText("No stores or dishes match “flat white”.");
  expect(await kept(), "and again").toBe("said");
});

test("the search page keeps every rule, found, empty and blank", async ({ page }) => {
  for (const path of ["/search?q=coffee", "/search?q=tofu", "/search?q=+"]) {
    await ready(page, path);
    expect(await page.evaluate(violations), path).toEqual([]);
  }
});

test("the search box is a GET form that makes the address", async ({ page }) => {
  await ready(page, "/search");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Search");
  await expect(page.locator("#search-intro")).toHaveText(
    "Search the stores by name or cuisine, and their dishes by name or what they are.",
  );
  // No search asked: no status; the intro, in the match's other arm.
  await expect(said(page)).toHaveCount(0);
  await expect(stores(page)).toHaveCount(0);
  await page.getByLabel("Stores and dishes").fill("blueberry scone");
  await page.getByRole("button", { name: "Search" }).click();
  await page.waitForURL(/\/search\?q=blueberry\+scone$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Search for “blueberry scone”");
  await expect(dishes(page, "Harbor Coffee")).toHaveText(["Blueberry Scone"]);
  await expect(page.getByLabel("Stores and dishes")).toHaveValue("blueberry scone");
});

test.describe("with scripts off", () => {
  test.use({ javaScriptEnabled: false });

  test("the search box works with the browser alone", async ({ page }) => {
    await page.goto("/search");
    await page.getByLabel("Stores and dishes").fill("café & co");
    await page.getByRole("button", { name: "Search" }).click();
    await page.waitForURL(/\/search\?q=caf%C3%A9\+%26\+co$/);
    await expect(said(page)).toHaveText("No stores or dishes match “café & co”.");
    await page.getByLabel("Stores and dishes").fill("CAFÉ");
    await page.getByRole("button", { name: "Search" }).click();
    await expect(stores(page)).toHaveText(["Harbor Coffee"]);
  });
});

test("an empty q is a blank search, and the first q is taken", async ({ page }) => {
  await ready(page, "/search?q=");
  await expect(said(page)).toHaveText("Type a store, a dish or a cuisine to search for.");
  await ready(page, "/search?utm_source=mail&q=scone&q=espresso");
  await expect(said(page)).toHaveText("1 store matches “scone”.");
  await expect(stores(page)).toHaveText(["Harbor Coffee"]);
});

test("the search page without a q keeps every rule", async ({ page }) => {
  await ready(page, "/search");
  expect(await page.evaluate(violations)).toEqual([]);
});
