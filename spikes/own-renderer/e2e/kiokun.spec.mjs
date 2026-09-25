// E10 — the kiokun slice, in a browser.
//
// A piece of kiokun.com written in Pleris: entry lookup and search over one
// shard (`han-1char-3`) of the real dictionary. The pages are the templates
// `pw build` compiled from `examples/kiokun/app.pw`; `Lookup` and `Search` are
// compiled components, run through the E8 host with the shard as their data
// layer (`spikes/kiokun/server`).

import { test, expect } from "@playwright/test";
import { KIOKUN_PORT } from "../playwright.config.mjs";

test.use({ baseURL: `http://127.0.0.1:${KIOKUN_PORT}` });

test("an entry renders from the compiled lookup, with no script", async ({ page }) => {
  const response = await page.goto("/人");
  expect(response.status()).toBe(200);
  await expect(page.locator("#headword")).toHaveText("人");
  await expect(page.locator("#chinese .pinyin").first()).toHaveText("rén");
  await expect(page.locator("#japanese .readings").first()).not.toBeEmpty();
  expect(await page.locator("script").count(), "a static page").toBe(0);
});

test("a simplified form is followed to its entry by the compiled lookup", async ({ page }) => {
  // `谚`'s file holds only a redirect to `諺`. Following it is `Lookup`'s
  // compiled match, not the host's.
  const response = await page.goto("/谚");
  expect(response.status()).toBe(200);
  await expect(page.locator("#headword")).toHaveText("諺");
});

test("a word this shard does not have is a 404", async ({ page }) => {
  const response = await page.goto("/無");
  expect(response.status()).toBe(404);
  await expect(page.locator("#headword")).toHaveText("無");
  await expect(page.locator("main p")).toHaveText("This shard has no entry for it.");
});

test("searching, then following a result", async ({ page }) => {
  await page.goto("/");
  await page.locator("#q").fill("person");
  await page.locator("#q").press("Enter");
  await expect(page).toHaveURL(/\/search\?q=person$/);
  await expect(page.locator("#q")).toHaveValue("person");
  const first = page.locator("#hits li a").first();
  await expect(first).toHaveText("人");
  await first.click();
  await expect(page.locator("#headword")).toHaveText("人");
});

test.describe("with JavaScript disabled", () => {
  test.use({ javaScriptEnabled: false });

  test("search and lookup still work: the pages carry no behaviour", async ({ page }) => {
    await page.goto("/search?q=%E3%81%B2%E3%81%A8");
    await expect(page.locator("#hits li a").first()).toHaveText("人");
    await page.locator("#hits li a").first().click();
    await expect(page.locator("#headword")).toHaveText("人");
  });
});
