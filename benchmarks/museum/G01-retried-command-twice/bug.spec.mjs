// G01: one press, its request delivered twice, is two adds.
//
// The request is the stack's own (a Server Action's POST), sent twice at the
// network as a retrying proxy or a flaky connection can, the way T08's hidden
// test sends it. Passes when the bug happens: the cart holds two.
import { test, expect } from "./fixtures.mjs";

test("a press whose request is delivered twice adds twice", async ({ page, store }) => {
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
  await expect(store.count(), "one press, and the cart holds two").toHaveText("2");
});
