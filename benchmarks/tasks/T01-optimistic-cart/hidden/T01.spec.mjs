// T01's hidden tests: the count moves before the server answers, and returns
// when the server refuses or the request fails. The request is held, refused
// or failed at the network; the page is not edited and the stack is not named.

import { test, expect } from "./fixtures.mjs";

/** Every mutation request, held until released. */
async function hold(page) {
  const held = [];
  await page.route("**/*", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await new Promise((resolve) => held.push(resolve));
    await route.continue();
  });
  return { count: () => held.length, release: () => held.splice(0).forEach((r) => r()) };
}

test("the count moves while the request is still held", async ({ page, store }) => {
  await store.open();
  const h = await hold(page);
  await page.locator("#menu button").first().click();
  await expect.poll(h.count).toBe(1);
  await expect(store.count()).toHaveText("1");
  h.release();
  await page.waitForTimeout(500);
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await store.open();
  await expect(store.count(), "and the server has it").toHaveText("1");
});

/** Every mutation request, held, then answered by `finish(route)`. */
async function holdThen(page, finish) {
  const held = [];
  await page.route("**/*", async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await new Promise((resolve) => held.push(resolve));
    await finish(route);
  });
  return { count: () => held.length, release: () => held.splice(0).forEach((r) => r()) };
}

for (const [what, finish] of [
  ["refused", (route) => route.fulfill({ status: 500, body: "refused" })],
  ["failed", (route) => route.abort()],
]) {
  test(`a ${what} add shows, then puts the count back`, async ({ page, store }) => {
    await store.open();
    const h = await holdThen(page, finish);
    await page.locator("#menu button").first().click();
    await expect.poll(h.count).toBe(1);
    await expect(store.count(), "shown before the answer").toHaveText("1");
    h.release();
    await expect(store.count(), "and put back after it").toHaveText("0");
    await page.unrouteAll({ behavior: "ignoreErrors" });
    await store.open();
    await expect(store.count(), "the server has nothing").toHaveText("0");
  });
}
