// ADR-0309: a page's query parameters are typed, and a GET form makes them.
//   - a plain GET form gives the page its query, with scripts on and off:
//     the browser writes the address (`+` for a space), the host reads it;
//   - a link gives it, and an address that gives nothing gives `None`;
//   - the first of a name is taken, and a name the route does not give is
//     ignored (an address may carry one from elsewhere);
//   - bytes that are no UTF-8 are refused, not replaced.
import { expect, test } from "@playwright/test";

const PAGE = "/demo/search";

const asked = (page) => page.locator("#asked");

test("a page given no query is given None", async ({ page }) => {
  await page.goto(PAGE);
  await expect(asked(page)).toHaveText("Nothing searched yet.");
});

test("a GET form gives the page its query", async ({ page }) => {
  await page.goto(PAGE);
  await page.getByLabel("Find").fill("flat white");
  await page.getByRole("button", { name: "Search" }).click();
  await page.waitForURL(/\/demo\/search\?q=flat\+white$/);
  await expect(asked(page)).toHaveText("You searched for flat white.");
});

test.describe("with scripts off", () => {
  test.use({ javaScriptEnabled: false });

  test("a GET form gives the page its query, the browser alone", async ({ page }) => {
    await page.goto(PAGE);
    await page.getByLabel("Find").fill("cortado & co");
    await page.getByRole("button", { name: "Search" }).click();
    await page.waitForURL(/\/demo\/search\?q=cortado\+%26\+co$/);
    await expect(asked(page)).toHaveText("You searched for cortado & co.");
  });
});

test("a link gives the page its query", async ({ page }) => {
  await page.goto(PAGE);
  await page.locator("#pie").click();
  await expect(asked(page)).toHaveText("You searched for apple pie.");
});

test("the first of a name is taken, and a name the route does not give is ignored", async ({
  page,
}) => {
  await page.goto(`${PAGE}?utm_source=mail&q=pie&q=cake`);
  await expect(asked(page)).toHaveText("You searched for pie.");
});

test("bytes that are no UTF-8 are refused, not replaced", async ({ request }) => {
  const answer = await request.get(`${PAGE}?q=%FF`);
  expect(answer.status()).toBe(400);
});
