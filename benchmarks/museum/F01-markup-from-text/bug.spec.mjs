// F01: text becomes executable markup.
//
// The staff's notice (T09's board, posted at its hook) carries markup with an
// event handler, as a stored cross-site script would. Passes when the bug
// happens: a customer who opens the store page runs the notice's script.
import { test, expect } from "./fixtures.mjs";

const NOTICE = `Open until 7 pm <img src="no-such-image" onerror="window.__pwMuseum = 'ran'">`;

test("a notice's markup runs as script in a customer's page", async ({ page, store }) => {
  await store.notice(NOTICE);
  await store.open();
  await expect(page.locator("#notice")).toContainText("Open until 7 pm");
  await expect
    .poll(() => page.evaluate(() => window.__pwMuseum ?? "did not run"), {
      message: "the notice's onerror handler ran in the customer's page",
      timeout: 5000,
    })
    .toBe("ran");
});
