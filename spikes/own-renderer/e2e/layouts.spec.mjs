// ADR-0303: a page is shown in its layout, which the pages that name it
// share. The store's four pages are shown in `StoreLayout`: a header with the
// way home and the session's cart, counted, its markup the same on every
// page, the page's own in its slot, and its count told as the cart changes,
// as any part of the page is (ADR-0219).
import { expect, test } from "@playwright/test";

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** The document's markup around its slot: the layout's before and after
 * it, as the browser parsed them, and whether the page's `<main>` is inside. */
function around(page) {
  return page.evaluate(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
    let start = null;
    let end = null;
    while (walker.nextNode()) {
      const c = walker.currentNode;
      if (c.data === "pw-slot") start = c;
      if (c.data === "/pw-slot") end = c;
    }
    if (!start || !end) return null;
    const main = document.querySelector("main");
    const inside =
      !!main &&
      !!(start.compareDocumentPosition(main) & Node.DOCUMENT_POSITION_FOLLOWING) &&
      !!(end.compareDocumentPosition(main) & Node.DOCUMENT_POSITION_PRECEDING);
    const header = document.querySelector("header");
    const before =
      !!header && !!(start.compareDocumentPosition(header) & Node.DOCUMENT_POSITION_PRECEDING);
    return { inside, before, header: header ? header.outerHTML : null };
  });
}

const PAGES = ["/", "/stores/47", "/cart", "/order"];

test("every page of the store is shown in its layout, the page in its slot", async ({ page }) => {
  let first = null;
  for (const path of PAGES) {
    await ready(page, path);
    const banner = page.getByRole("banner");
    const nav = banner.getByRole("navigation", { name: "Store" });
    await expect(nav).toBeVisible();
    await expect(nav.getByRole("link", { name: "Stores", exact: true })).toHaveAttribute(
      "href",
      "/",
    );
    await expect(nav.locator("#header-cart")).toHaveAttribute("href", "/cart");
    await expect(page.locator("#header-cart-count")).toHaveText("0");
    // The header is the layout's, before the slot; the page's `<main>` is
    // in it.
    const where = await around(page);
    expect(where, path).not.toBeNull();
    expect(where.before, path).toBe(true);
    expect(where.inside, path).toBe(true);
    // And its markup is the same on every page: its parts are numbered
    // before each page's, so its `data-pw` and markers are too.
    if (first === null) first = where.header;
    expect(where.header, path).toBe(first);
  }
});

test("the layout's count is told as the cart changes, beside the page's", async ({ context }) => {
  // Two tabs of one session: the store's page, and the order's, which reads
  // no cart of its own, only its layout's.
  const store = await context.newPage();
  const order = await context.newPage();
  await ready(store, "/stores/47");
  await ready(order, "/order");

  await store.getByRole("button", { name: "Add Espresso" }).click();
  // The page's own count, and the layout's, on the page that changed it.
  await expect(store.locator("#cart-count")).toHaveText("1");
  await expect(store.locator("#header-cart-count")).toHaveText("1");
  // And the layout's on a page that reads the cart only through it.
  await expect(order.locator("#header-cart-count")).toHaveText("1");

  await store.getByRole("button", { name: "Remove Espresso" }).click();
  await expect(store.locator("#header-cart-count")).toHaveText("0");
  await expect(order.locator("#header-cart-count")).toHaveText("0");
});

test("the layout's count moves with the page's before the server answers", async ({ page }) => {
  // ADR-0122: an optimistic transition is shown before the round trip, on
  // every part that reads the entry, the layout's as the page's, so the two
  // never show two counts while the command is out. The command is held at
  // the network, so "before" is observed rather than inferred from timing.
  await ready(page, "/stores/47");
  const held = [];
  await page.route("**/command/store.page.add_to_cart", async (route) => {
    await new Promise((resolve) => held.push(resolve));
    await route.continue();
  });
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect.poll(() => held.length).toBe(1);
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(page.locator("#header-cart-count")).toHaveText("1");
  const answered = page.waitForResponse("**/command/store.page.add_to_cart");
  held.splice(0).forEach((release) => release());
  await answered;
  await expect(page.locator("#header-cart-count")).toHaveText("1");
});

test.describe("without a script", () => {
  test.use({ javaScriptEnabled: false });

  test("the layout is served with the page, its count as read", async ({ page }) => {
    await page.goto("/cart");
    await expect(page.getByRole("banner").getByRole("navigation", { name: "Store" })).toBeVisible();
    await expect(page.locator("#header-cart-count")).toHaveText("0");
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Your cart");
  });
});
