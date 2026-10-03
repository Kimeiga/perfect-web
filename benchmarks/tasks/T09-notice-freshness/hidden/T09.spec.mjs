// T09's hidden tests: the store's notice is fresh within ten seconds, and the
// notice board is still asked at most once in ten seconds. Read by role and
// text, never by a stack's markup.

import { test, expect } from "./fixtures.mjs";

// The board is one per server: these run one at a time, and the count
// first, so it is measured whatever the other test finds.
test.describe.configure({ mode: "serial" });

const notice = (page) => page.locator("#notice");

test("the board is asked at most once in ten seconds", async ({ page, store }) => {
  await store.open();
  const before = await store.noticeCalls();
  for (let i = 0; i < 6; i++) await store.open();
  const after = await store.noticeCalls();
  // Six pages in a few seconds: at most one expiry falls among them.
  expect(after - before).toBeLessThanOrEqual(2);
});

test("a new notice shows within ten seconds", async ({ page, store }) => {
  await store.open();
  const text = `Closing early, ${Date.now()}`;
  await store.notice(text);
  const posted = Date.now();
  await expect
    .poll(
      async () => {
        await store.open();
        return notice(page).textContent();
      },
      { timeout: 15_000, intervals: [1_000] },
    )
    .toBe(text);
  expect(Date.now() - posted).toBeLessThan(13_000);
});
