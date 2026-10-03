// T03's hidden tests: the cart's lines, its customer's alone. Read by role
// and text, never by a stack's markup.

import { test, expect } from "./fixtures.mjs";

const list = (page) => page.getByRole("list", { name: "In your cart" });
const lines = (page) => list(page).getByRole("listitem");

test("an empty cart lists nothing", async ({ page, store }) => {
  await store.open();
  await expect(list(page)).toBeAttached();
  await expect(lines(page)).toHaveCount(0);
});

test("each line shows its item's name and how many, in the order added", async ({
  page,
  store,
}) => {
  await store.open();
  await store.add(page, 1);
  await store.add(page, 0);
  await store.add(page, 1);
  await expect(lines(page)).toHaveText(["Cortado × 2", "Espresso × 1"]);
  await expect(store.count()).toHaveText("3");
});

test("the list follows the cart with no reload", async ({ page, store }) => {
  await store.open();
  // A reload would lose this.
  await page.evaluate(() => {
    window.__t03 = "still here";
  });
  await store.add(page, 2);
  await expect(lines(page)).toHaveText(["Cold Brew × 1"]);
  await store.add(page, 2);
  await expect(lines(page)).toHaveText(["Cold Brew × 2"]);
  await store.clear(page);
  await expect(lines(page)).toHaveCount(0);
  expect(await page.evaluate(() => window.__t03)).toBe("still here");
});

test("a cart's lines are its customer's alone", async ({ browser, page, store }) => {
  await store.open();
  await store.add(page, 0);
  await expect(lines(page)).toHaveText(["Espresso × 1"]);
  // Another customer, after the first: whatever was kept, none of it is
  // theirs.
  const context = await browser.newContext();
  const other = await context.newPage();
  await store.open(other);
  await expect(lines(other)).toHaveCount(0);
  await expect(store.count(other)).toHaveText("0");
  // And their change is not the first customer's.
  await store.add(other, 2);
  await expect(lines(other)).toHaveText(["Cold Brew × 1"]);
  await store.open(page);
  await expect(lines(page)).toHaveText(["Espresso × 1"]);
  await expect(store.count(page)).toHaveText("1");
  await context.close();
});
