// ADR-0184, charter §15.6 tests 2, 12 and 13: what a cache may keep holds
// nothing of a session's. The store's page is a session's, so no cache keeps
// it, and the browser asks for it again rather than show a copy; a file of
// the build is every reader's, and names no session. What the server's own
// shared caches keep is the development server's tests'.
import { expect, test } from "@playwright/test";

async function ready(page, path = "/stores/47") {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

test("the store's page is kept by no cache, and a build's file names no session", async ({
  playwright,
  baseURL,
}) => {
  // A fresh visitor each time: no cookie, so the server names a session.
  const fresh = await playwright.request.newContext({ baseURL });
  try {
    const page = await fresh.get("/stores/47");
    expect(page.status()).toBe(200);
    expect(page.headers()["cache-control"]).toBe("private, no-store");
    expect(page.headers()["set-cookie"]).toMatch(/^pw-session=/);
  } finally {
    await fresh.dispose();
  }
  const first = await playwright.request.newContext({ baseURL });
  try {
    // The runtime, asked for before any page: every reader's, so no session.
    const runtime = await first.get("/pw-runtime.mjs");
    expect(runtime.status()).toBe(200);
    expect(runtime.headers()["set-cookie"]).toBeUndefined();
    expect(runtime.headers()["cache-control"] ?? "").not.toContain("private");
  } finally {
    await first.dispose();
  }
});

// Read-your-writes across the history. These engines ask for the page again
// here whatever its response says; `no-store` is what makes a browser that
// keeps pages for going back ask again too, and the test above holds it.
test("back to the store, its cart is as it is now, not as it was left", async ({ page, context }) => {
  await ready(page);
  await page.getByRole("button", { name: "Add Espresso", exact: true }).click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  await ready(page, "/stores/48");
  // The same session adds again, in another tab, while the first is away.
  const other = await context.newPage();
  await ready(other);
  await other.getByRole("button", { name: "Add Cortado", exact: true }).click();
  await expect(other.locator("#cart-count")).toHaveText("2");
  await other.close();
  // A copy of the page as it was left would say 1.
  await page.goBack();
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
  await expect(page.locator("#cart-count")).toHaveText("2");
  await page.getByRole("button", { name: "Add Cold Brew", exact: true }).click();
  await expect(page.locator("#cart-count")).toHaveText("3");
});
