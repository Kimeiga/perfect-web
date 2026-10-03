// T04's hidden tests: a new state of an order, shown where every other one
// is. Read by role and text, never by a stack's markup.

import { test, expect } from "./fixtures.mjs";

const order = (page) => page.getByRole("region", { name: "Your order" });

test("a ready order says so", async ({ page, store }) => {
  await store.open();
  await expect(order(page)).toHaveText("No order yet.");
  await store.order("ready");
  await store.open();
  await expect(order(page)).toHaveText("Your order is ready for pickup.");
});

test("every other state still shows as it did", async ({ page, store }) => {
  await store.open();
  for (const [status, said] of [
    ["placed", "Your order is placed."],
    ["preparing", "Your order is being prepared."],
    ["delivered", "Your order was delivered."],
  ]) {
    await store.order(status);
    await store.open();
    await expect(order(page)).toHaveText(said);
  }
});

test("one customer's order is not another's", async ({ browser, page, store }) => {
  await store.open();
  await store.order("ready");
  const context = await browser.newContext();
  const other = await context.newPage();
  await store.open(other);
  await expect(order(other)).toHaveText("No order yet.");
  await store.open(page);
  await expect(order(page)).toHaveText("Your order is ready for pickup.");
  await context.close();
});
