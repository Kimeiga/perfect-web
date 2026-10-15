// H07: one round trip per row (N+1).
//
// Passes when the bug happens: one view of the store page asks the database
// once for each item on its menu, so the work grows with the menu and with
// every reader, where one round trip would do.
import { test, expect } from "./fixtures.mjs";

async function queries(page) {
  const r = await page.request.get("/bench/queries");
  if (!r.ok()) throw new Error(`/bench/queries: ${r.status()}`);
  return (await r.json()).queries;
}

test("one view of the page reads the database once per menu item", async ({ page, store }) => {
  const before = await queries(page);
  await store.open();
  const after = await queries(page);
  const items = await page.locator("#menu li").count();
  expect(items).toBe(3);
  expect(after - before, "a round trip for each item on the menu").toBe(items);
});
