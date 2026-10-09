// Track `kiokun` (W6): kiokun.com's word page, in three engines. kiokun's
// program (`examples/kiokun-site`) is served by the development server on
// hosts of its own (KIOKUN_PORTS), one per engine, its entries read from
// KIOKUN_DATA or the repository's sample (ADR-0037).
//   - an entry's words in each language, as kiokun.com's page shows them;
//   - the same with script off, since the page is rendered by the server
//     (docs/PARALLEL.md, "W6's inventory, answered", Q1);
//   - a stub's redirect followed, and a word kiokun lacks a 404;
//   - what kiokun.com does not show, not shown (Q8).
// The route is `/word/{word}` until the host's own paths move under
// `/_pw/`, then kiokun.com's `/{word}`.
import { expect, test } from "@playwright/test";
import { KIOKUN_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${KIOKUN_PORTS[testInfo.project.name]}`);
  },
});

const at = (word) => `/word/${encodeURIComponent(word)}`;

/** What a page for 人 shows, whichever way it was rendered. */
async function shows人(page) {
  await expect(page.locator("#headword")).toHaveText("人");
  await expect(page.locator("#chinese-heading")).toHaveText("Chinese");
  await expect(page.locator("#chinese .chinese-pronunciation").first()).toHaveText("[rén]");
  await expect(page.locator("#chinese .cantonese-pronunciation").first()).toHaveText("[jan4]");
  await expect(page.locator("#chinese .chinese-definition").first()).toHaveText("man");
  await expect(page.locator("#japanese .kana-pronunciation").first()).not.toBeEmpty();
  await expect(page.locator("#japanese .common").first()).toHaveText("★");
  await expect(page.locator("#korean .korean-word-text").first()).toHaveText("인");
  await expect(page.locator("#korean .korean-hanja").first()).toHaveText("[人]");
  await expect(page.locator("#names-heading")).toHaveText("Japanese Names");
  expect(await page.locator("#names .name-entry").count()).toBeGreaterThan(0);
}

test("an entry shows its words in each language, as kiokun.com's page does", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  const response = await page.goto(at("人"));
  expect(response.status()).toBe(200);
  await shows人(page);
  await expect(page).toHaveTitle("人 | Kiokun");
  expect(errors, "no error in the console").toEqual([]);
});

test("the page is the same with script off: the server renders it", async ({ browser }) => {
  const off = await browser.newContext({ javaScriptEnabled: false });
  const page = await off.newPage();
  const response = await page.goto(at("人"));
  expect(response.status()).toBe(200);
  await shows人(page);
  await off.close();
});

test("a stub is followed to the entry it names", async ({ page }) => {
  // 谚's file holds only a redirect to 諺, followed by the compiled `Word`.
  const response = await page.goto(at("谚"));
  expect(response.status()).toBe(200);
  await expect(page.locator("#headword")).toHaveText("谚");
  await expect(page.locator("#chinese .chinese-definition").first()).toHaveText("proverb");
  await expect(page.locator("#japanese .kanji-text").first()).toHaveText("諺");
});

test("a word kiokun does not have is a 404", async ({ page }) => {
  const response = await page.goto(at("zzzz-no-such-word"));
  expect(response.status()).toBe(404);
  expect(await page.locator("section").count()).toBe(0);
});

test("what kiokun.com does not show is not shown", async ({ page }) => {
  // The slice showed these; kiokun.com's page does not, and the owner rules
  // on them with the word page's first review.
  await page.goto(at("人"));
  const text = await page.locator("main").innerText();
  for (const hidden of ["Strokes", "School grade", "Frequency rank", "Meaning in Korean"]) {
    expect(text).not.toContain(hidden);
  }
});
