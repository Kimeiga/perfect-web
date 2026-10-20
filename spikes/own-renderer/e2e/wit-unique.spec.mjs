// ADR-XXXX: a query and UI declaration with the same source name stay distinct.
import { expect, test } from "@playwright/test";

const PAGE = "/namespace-identities";

test("same-named query, view and page render their own values before JavaScript", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(PAGE);
  await expect(page.locator("#namespace-view")).toHaveText("Card view");
  await expect(page.locator("#namespace-query")).toHaveText("7");
  await expect(page.locator("#namespace-page-query")).toHaveText("11");
  await expect(page.locator("#namespace-computed")).toHaveText("8");
  await expect(page.locator("#namespace-count")).toHaveText("0");
  await expect(page.locator("#namespace-next")).toHaveText("1");
  await context.close();
});

test("the namespaced UI resumes and updates its computed signal", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await page.getByRole("button", { name: "Count", exact: true }).click();
  await expect(page.locator("#namespace-count")).toHaveText("1");
  await expect(page.locator("#namespace-next")).toHaveText("2");
  await expect(page.locator("#namespace-query")).toHaveText("7");
  await expect(page.locator("#namespace-page-query")).toHaveText("11");
  await expect(page.locator("#namespace-view")).toHaveText("Card view");
  expect(errors).toEqual([]);
});