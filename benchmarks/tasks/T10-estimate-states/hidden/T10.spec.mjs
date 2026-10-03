// T10's hidden tests: a slow, failing delivery estimate neither holds up nor
// takes down the store page. Read by role and text, never by a stack's
// markup. The estimator is the session's, so each test sets its own.

import { test, expect } from "./fixtures.mjs";

const delivery = (page) => page.getByRole("region", { name: "Delivery" });

test("the estimate is shown when it comes", async ({ page, store }) => {
  await store.open();
  await store.estimate("minutes=35");
  await store.open();
  await expect(delivery(page)).toHaveText("Delivery in 35 min");
});

test("a failing estimate leaves the rest of the page working", async ({ page, store }) => {
  await store.open();
  await store.estimate("fail=down");
  await store.open();
  await expect(delivery(page)).toHaveText("Delivery estimate unavailable");
  await expect(page.locator("#menu li")).toHaveCount(3);
  await store.add();
  await expect(store.count()).toHaveText("1");
});

test("a slow estimate does not hold up the page", async ({ page, store, storePath }) => {
  await store.open();
  await store.estimate("delay=2000");
  await page.goto(storePath, { waitUntil: "commit" });
  await expect(delivery(page)).toHaveText("Estimating delivery");
  await expect(page.locator("#menu li")).toHaveCount(3);
  await store.ready();
  await store.add();
  await expect(store.count()).toHaveText("1");
  await expect(delivery(page)).toHaveText("Delivery in 25 min");
});
