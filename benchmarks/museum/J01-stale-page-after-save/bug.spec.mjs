// J01: a page shown after a save is the one read before it.
//
// The cart's page is prefetched whole by a link, as a store page links to its
// cart. A press saves (one Espresso added), then pushes the router to the
// cart's page. Passes when the bug happens: the page shown says the cart is
// empty, and a reload of the same address says it holds one.
import { test, expect } from "./fixtures.mjs";

test("the cart's page pushed to after a save is the one read before it", async ({ page, store }) => {
  const prefetched = page.waitForResponse(
    (r) => new URL(r.url()).pathname.endsWith("/cart") && r.request().method() === "GET",
    { timeout: 15_000 },
  );
  await store.open();
  await prefetched;
  await page.locator("#add-and-view").click();
  await page.waitForURL(/\/cart$/);
  await expect(page.locator("#cart-page-count"), "shown after the save: the page read before it").toHaveText("0");
  await page.reload();
  await expect(page.locator("#cart-page-count"), "the same address, read again: the save is there").toHaveText("1");
});
