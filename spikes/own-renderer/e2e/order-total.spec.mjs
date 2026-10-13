// Track `order-total` (W9): the cart page itemizes the order's charges
// under a disclosure, priced by the program from the store's data, San
// Francisco's as the stores are first; a tip is none until the reader
// chooses one; and the order is charged the total the page showed.
import { expect, test } from "@playwright/test";

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

test("the cart itemizes each charge, and the order is charged the total it showed", async ({
  page,
}) => {
  await ready(page, "/stores/47");
  for (let i = 0; i < 2; i += 1) {
    const answered = page.waitForResponse("**/command/store.page.add_to_cart");
    await page.getByRole("button", { name: "Add Espresso" }).click();
    await answered;
  }
  await ready(page, "/cart");
  // $7.00 of food: delivery $2.99; service 15% is $1.05, at least $3.00;
  // $2.50 under $10.00; 8.625% of $7.00 is $0.60375, $0.60; no tip.
  await expect(page.locator("#checkout-total")).toHaveText("$16.09");
  await page.locator("#cart-fees summary").click();
  await expect(page.locator("#charge-subtotal")).toHaveText("$7.00");
  await expect(page.locator("#charge-delivery")).toHaveText("$2.99");
  await expect(page.locator("#charge-service")).toHaveText("$3.00");
  await expect(page.locator("#charge-small")).toHaveText("$2.50");
  await expect(page.locator("#charge-tax")).toHaveText("$0.60");
  await expect(page.locator("#charge-tip")).toHaveText("$0.00");
  // A tip of 20%, $1.40, untaxed.
  const tipped = page.waitForResponse("**/command/store.page.choose_tip");
  await page.locator("#tip-20").click();
  await tipped;
  await expect(page.locator("#charge-tip")).toHaveText("$1.40");
  await expect(page.locator("#charge-tax")).toHaveText("$0.60");
  await expect(page.locator("#checkout-total")).toHaveText("$17.49");
  await page.getByRole("button", { name: "Place order" }).click();
  await page.waitForURL(/\/order$/);
  await expect(page.locator("#order-total")).toHaveText("Total charged: $17.49");
});

test("a code that is not one is refused by name", async ({ page }) => {
  await ready(page, "/stores/47");
  const answered = page.waitForResponse("**/command/store.page.add_to_cart");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await answered;
  await ready(page, "/cart");
  await page.locator("#promo-code").fill("NOPE");
  await page.locator("#apply-promo").click();
  await expect(page.locator("#cart-notice")).toHaveText("That code isn't one we know.");
});
