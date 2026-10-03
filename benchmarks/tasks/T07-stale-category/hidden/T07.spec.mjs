// T07's hidden tests: the menu by category, Hot made slow. The chosen
// category's items show as soon as it answers, whatever answers last, and a
// category the customer moved past is not read any longer. Read by role and
// text, never by a stack's markup.

import { test, expect } from "./fixtures.mjs";

// The slow category is one per server: these run one at a time.
test.describe.configure({ mode: "serial" });

const browsed = (page) => page.locator("#browse li");
const tab = (page, name) =>
  page.getByRole("navigation", { name: "Categories" }).getByRole("button", { name });

test("the chosen category shows as soon as it answers", async ({ page, store }) => {
  await store.category("hot", 1500);
  await store.open();
  await tab(page, "Hot").click();
  await tab(page, "Cold").click();
  // Cold's items, though Hot is still being read.
  await expect(browsed(page)).toHaveText(["Cold Brew"], { timeout: 1_000 });
  // And still, once Hot would have answered.
  await page.waitForTimeout(2_000);
  await expect(browsed(page)).toHaveText(["Cold Brew"]);
});

test("a category the customer moved past is not read any longer", async ({ page, store }) => {
  await store.category("hot", 1500);
  await store.open();
  const stopped = [];
  page.on("requestfailed", (r) => stopped.push(decodeURIComponent(r.url())));
  await tab(page, "Hot").click();
  // Hot's request is under way.
  await page.waitForTimeout(300);
  await tab(page, "Cold").click();
  await expect
    .poll(() => stopped.some((url) => url.includes("hot")), { timeout: 3_000 })
    .toBe(true);
});
