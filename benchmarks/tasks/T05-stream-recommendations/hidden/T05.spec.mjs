// T05's hidden tests: the store's recommendations, sent after the store's own
// content and never holding up its menu. Read by role and text, never by a
// stack's markup.

import { test, expect } from "./fixtures.mjs";

// The recommender is one per server: these run one at a time, and leave it
// as they found it.
test.describe.configure({ mode: "serial" });
test.afterAll(async ({ request }) => {
  await request.post("/bench/recommendations");
});

const section = (page) => page.getByRole("region", { name: "Recommendations" });

test("the recommendations are shown, by name", async ({ page, store }) => {
  await store.recommend("delay=300&items=mocha:Mocha,latte:Latte");
  await store.open();
  await expect(section(page).getByRole("listitem")).toHaveText(["Mocha", "Latte"]);
});

test("the menu does not wait for them", async ({ page, store, storePath }) => {
  await store.recommend("delay=2500");
  await page.goto(storePath, { waitUntil: "commit" });
  await expect(section(page)).toContainText("Finding recommendations");
  await expect(page.locator("#menu li")).toHaveCount(3);
  await store.ready();
  await store.add();
  await expect(store.count()).toHaveText("1");
  // The press was answered while the recommender was still answering.
  await expect(section(page)).toContainText("Finding recommendations");
  await expect(section(page).getByRole("listitem")).toHaveText(["Cortado", "Cold Brew"]);
});
