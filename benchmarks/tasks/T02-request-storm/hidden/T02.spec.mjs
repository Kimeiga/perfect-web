// T02's hidden tests: customers who open the store page at once share one
// ask of the kitchen, and a prep time is never kept past its ask. Read by
// the page's text, never by a stack's markup.

import { test, expect } from "./fixtures.mjs";

// The kitchen is one per server: these run one at a time.
test.describe.configure({ mode: "serial" });

// The prep time a served page shows, from its text.
function prepTime(html) {
  const text = html.replace(/<!--[\s\S]*?-->/g, "").replace(/<[^>]+>/g, "");
  return text.match(/Ready in (\d+) min/)?.[1];
}

test("customers who open the page at once share an ask", async ({ page, store, storePath }) => {
  await store.open();
  const before = await store.prepCalls();
  const pages = await Promise.all(
    Array.from({ length: 12 }, () => page.request.get(storePath).then((r) => r.text())),
  );
  const after = await store.prepCalls();
  for (const html of pages) expect(prepTime(html)).toMatch(/^\d+$/);
  // Twelve pages at once share one ask. A page another test opens meanwhile
  // may ask too.
  expect(after - before).toBeLessThanOrEqual(4);
});

test("a page shows the prep time as the kitchen says it now", async ({ page, store, storePath }) => {
  await store.open();
  const minutes = 20 + (Date.now() % 30);
  await store.prep(minutes);
  // An ask already under way when the time changed answers what it was
  // told; it is over well within this.
  await page.waitForTimeout(1_000);
  const html = await (await page.request.get(storePath)).text();
  expect(prepTime(html)).toBe(String(minutes));
});
