// Track `reviews` (ADR-XXXX): a store rated through a delivered order, in
// three engines, on hosts of its own (REVIEWS_PORTS), one per engine, since a
// store's ratings are every reader's on a host; its readers signed in
// through the development identity provider (`PW_IDENTITY=dev-accounts`).
//   - the order page links each store of a delivered order to rate, told
//     when the kitchen delivers it, unreloaded;
//   - the rate page: 1 to 5 chosen, a comment, Send; then what was given,
//     the form gone; the store's page shows the review, without its
//     reviewer;
//   - another reader's open store page is told of a rating, unreloaded.
// Each test rates a store no other test of this file rates, so tests that
// run at once on one host do not change what another reads.
import { expect, test } from "@playwright/test";
import { REVIEWS_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${REVIEWS_PORTS[testInfo.project.name]}`);
  },
});

/** `path`, its handlers attached. */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** The page's handlers attached after a navigation the page made. */
async function settled(page) {
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** A handle no other test or engine has: at most 15 of a-z, 0-9 and _. */
const handle = (testInfo, who) =>
  `${who}${testInfo.project.name.slice(0, 2)}${(Date.now() % 1e7).toString(36)}`.slice(0, 15);

const PASSWORD = "correct horse battery";

/** Signed up at the development provider, and back at the store's home. */
async function signUp(page, who) {
  await page.goto("/sign-up");
  await page.getByLabel("Handle").fill(who);
  await page.getByLabel("Name").fill(who);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();
  await expect(page).toHaveURL(/\/$/);
}

/** `item` ordered from store `store`, the order page open after. */
async function ordered(page, store, item) {
  await ready(page, `/stores/${store}`);
  const added = page.waitForResponse("**/command/store.page.add_to_cart");
  await page.getByRole("button", { name: `Add ${item}` }).click();
  await added;
  await ready(page, "/cart");
  await page.getByRole("button", { name: "Place order" }).click();
  await expect(page).toHaveURL(/\/order$/);
  await settled(page);
  await expect(page.locator("#order-status")).toHaveText("Placed: the store has your order.");
}

test("a delivered order's store is rated once, and its page shows the review", async ({
  page,
}, testInfo) => {
  await signUp(page, handle(testInfo, "rater"));
  await ordered(page, "47", "Espresso");
  // Placed, nothing to rate yet.
  await expect(page.locator("#rate-links")).toHaveCount(0);
  // The kitchen delivers it, and the open page is told, unreloaded.
  expect((await page.request.post("/bench/order?status=delivered")).ok()).toBe(true);
  await expect(page.locator("#order-status")).toHaveText("Delivered.");
  const rate = page.getByRole("link", { name: "Rate Blue Bottle" });
  await expect(rate).toBeVisible();
  await rate.click();
  await expect(page).toHaveURL(/\/order\/rate\/47$/);
  await settled(page);
  await expect(page.locator("#rate-store")).toHaveText("How was your order from Blue Bottle?");
  await expect(page.locator("#stars-chosen")).toHaveText("Choose from 1 to 5.");
  // Sent with none chosen: the page says so, and nothing is sent.
  await page.getByRole("button", { name: "Send rating" }).click();
  await expect(page.locator("#rate-said")).toHaveText("Choose from 1 to 5 first.");
  const four = page.getByRole("button", { name: "4 out of 5" });
  await four.click();
  await expect(four).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("button", { name: "5 out of 5" })).toHaveAttribute(
    "aria-pressed",
    "false",
  );
  await expect(page.locator("#stars-chosen")).toHaveText("Your rating: 4 out of 5");
  const words = `Good espresso ${testInfo.project.name} ${Date.now()}`;
  await page.locator("#comment").fill(words);
  const sent = page.waitForResponse("**/command/store.rate.rate_order");
  await page.getByRole("button", { name: "Send rating" }).click();
  expect((await sent).ok()).toBe(true);
  await expect(page.locator("#rate-said")).toHaveText("Thanks: your rating is sent.");
  // What was given, read again; the form gone.
  await expect(page.locator("#rate-state")).toHaveText(
    "You rated Blue Bottle 4 out of 5. Thanks!",
  );
  await expect(page.locator("#rate-comment")).toHaveText(words);
  await expect(page.locator("#rate-form")).toBeHidden();
  // The order page says what was given, and links nothing more to rate.
  await ready(page, "/order");
  await expect(page.locator("#rate-links")).toHaveText("You rated Blue Bottle 4 out of 5.");
  await expect(page.getByRole("link", { name: "Rate Blue Bottle" })).toHaveCount(0);
  // The store's page shows the review, its stars and words, and no reader.
  await ready(page, "/stores/47");
  const review = page.locator("#reviews li", { hasText: words });
  await expect(review).toHaveCount(1);
  await expect(review.locator(".stars")).toHaveText("4 out of 5");
  await expect(page.locator("#reviews")).not.toContainText("rater");
  await expect(page.locator("#store-rating")).toContainText("rating");
});

test("another reader's open store page is told of a rating, unreloaded", async ({
  browser,
}, testInfo) => {
  const [mine, theirs] = await Promise.all([browser.newContext(), browser.newContext()]);
  const [rater, reader] = await Promise.all([mine.newPage(), theirs.newPage()]);
  await signUp(rater, handle(testInfo, "harbor"));
  await ordered(rater, "48", "Drip Coffee");
  expect((await rater.request.post("/bench/order?status=delivered")).ok()).toBe(true);
  await ready(reader, "/stores/48");
  const words = `Warm and quick ${testInfo.project.name} ${Date.now()}`;
  await expect(reader.locator("#reviews li", { hasText: words })).toHaveCount(0);
  await ready(rater, "/order/rate/48");
  await rater.getByRole("button", { name: "5 out of 5" }).click();
  await rater.locator("#comment").fill(words);
  const sent = rater.waitForResponse("**/command/store.rate.rate_order");
  await rater.getByRole("button", { name: "Send rating" }).click();
  expect((await sent).ok()).toBe(true);
  // The other reader's open page shows it, unreloaded.
  await expect(reader.locator("#reviews li", { hasText: words })).toHaveCount(1);
  await expect(reader.locator("#store-rating")).toContainText("rating");
  await Promise.all([mine.close(), theirs.close()]);
});
