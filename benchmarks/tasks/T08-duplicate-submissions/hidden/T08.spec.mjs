// T08's hidden test: one press, its request delivered twice, is one add.
//
// The request is the stack's own (a Server Action, a form action, a Pleris
// command), sent twice at the network as a retrying proxy would. The page is
// not edited and the stack is not named.

import { test, expect } from "./fixtures.mjs";

test("a press whose request is delivered twice adds once", async ({ page, store }) => {
  await store.open();
  let delivered = 0;
  await page.route("**/*", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fetch();
    const again = await route.fetch();
    delivered += 2;
    await route.fulfill({ response: again });
  });
  await store.add();
  expect(delivered, "the press's request was delivered twice").toBe(2);
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await store.open();
  await expect(store.count()).toHaveText("1");
});

test("two presses, each delivered twice, add two", async ({ page, store }) => {
  await store.open();
  await page.route("**/*", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fetch();
    await route.fulfill({ response: await route.fetch() });
  });
  await store.add();
  await store.add(undefined, 1);
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await store.open();
  await expect(store.count()).toHaveText("2");
});
