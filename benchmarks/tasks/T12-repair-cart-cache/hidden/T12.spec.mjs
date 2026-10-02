// T12's hidden tests: whoever read the page before, a reader sees their own
// cart, at once and after their own changes.

import { test, expect } from "./fixtures.mjs";

async function reader(browser, baseURL) {
  const context = await browser.newContext({ baseURL });
  return { context, page: await context.newPage() };
}

test("a new reader's cart is empty, right after another reader's add", async ({ browser, baseURL, store }) => {
  const a = await reader(browser, baseURL);
  const b = await reader(browser, baseURL);
  await store.open(a.page);
  await store.add(a.page);
  await store.add(a.page);
  await store.open(a.page);
  await expect(store.count(a.page)).toHaveText("2");
  await store.open(b.page);
  await expect(store.count(b.page), "B has added nothing").toHaveText("0");
  await a.context.close();
  await b.context.close();
});

test("each reader sees their own count, read in either order", async ({ browser, baseURL, store }) => {
  const a = await reader(browser, baseURL);
  const b = await reader(browser, baseURL);
  await store.open(a.page);
  await store.add(a.page);
  await store.open(b.page);
  await store.add(b.page, 1);
  await store.add(b.page, 2);
  for (const [r, n] of [[b, "2"], [a, "1"], [b, "2"], [a, "1"]]) {
    await store.open(r.page);
    await expect(store.count(r.page)).toHaveText(n);
  }
  await a.context.close();
  await b.context.close();
});
