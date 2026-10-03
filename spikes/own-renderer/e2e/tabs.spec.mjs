// ADR-0161: each document is its own subscriber. Two tabs of one session
// each hear every change to the session's cart, whichever tab made it.
//
// Until 2026-10-03 a session had one subscriber. Serving the second tab
// cleared what the first was waiting for, and the two tabs shared one queue,
// whose frames either tab's request acknowledged.
import { expect, test } from "@playwright/test";

// Each adapter names the document it is for, so the test runs on both.
for (const transport of ["stream", "poll"]) {
  test(`two tabs of one session each hear the other's add (${transport})`, async ({ context }) => {
    // How many times each tab navigated to the store: a tab told to reload
    // would show the right count from a fresh document, and hide the frames
    // it never heard. Navigations, not `load` events: Firefox reports the new
    // tab's `about:blank` as a second load.
    const loads = new Map();
    const opened = async () => {
      const page = await context.newPage();
      loads.set(page, 0);
      page.on("framenavigated", (frame) => {
        if (frame === page.mainFrame() && frame.url().includes("/StorePage.html")) {
          loads.set(page, loads.get(page) + 1);
        }
      });
      await page.goto(`/StorePage.html?transport=${transport}`);
      await page.waitForFunction(() => document.documentElement.dataset.pwReady);
      return page;
    };
    const first = await opened();
    // The second tab: the same session, by its cookie, another document.
    const second = await opened();
    const count = (page) => page.locator("#cart-count");
    await expect(count(first)).toHaveText("0");
    await expect(count(second)).toHaveText("0");

    // An add in the second tab reaches the first ...
    await second.locator("#menu button").first().click();
    await expect(count(second)).toHaveText("1");
    await expect(count(first)).toHaveText("1");

    // ... and one in the first reaches the second, each tab a page of its own.
    await first.locator("#menu button").first().click();
    await expect(count(first)).toHaveText("2");
    await expect(count(second)).toHaveText("2");
    // Heard, not reloaded: each tab loaded once.
    expect([loads.get(first), loads.get(second)]).toEqual([1, 1]);
  });
}
