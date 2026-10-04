// ADR-0190: every page that binds a query is served at its route, and kept
// current by its own plan. The store's cart as a page of its own, `/cart`,
// reads the session's cart as the store's page does: a change made on either
// reaches the other while both are open. Until 2026-10-04 only the store's
// page could bind a query.
import { expect, test } from "@playwright/test";

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

test("the cart is a page of its own, at its route", async ({ page }) => {
  await ready(page, "/cart");
  await expect(page).toHaveTitle("Your cart");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Your cart");
  await expect(page.locator("#cart-count")).toHaveText("0");
  await expect(page.locator("#cart-empty")).toBeVisible();
});

test("a change made on either page reaches the other, open beside it", async ({ context }) => {
  // Two tabs of one session.
  const store = await context.newPage();
  const cart = await context.newPage();
  await ready(store, "/stores/47");
  await ready(cart, "/cart");
  const line = cart.locator("#cart-lines li");

  // Added on the store's page: the cart page shows the line, unreloaded.
  await store.getByRole("button", { name: "Add Espresso" }).click();
  await expect(line).toHaveCount(1);
  await expect(line.locator("span").first()).toHaveText("Espresso");
  await expect(cart.locator("#cart-count")).toHaveText("1");
  await expect(cart.locator("#cart-empty")).toBeHidden();
  await expect(cart.locator("#cart-fees")).toBeVisible();

  // Increased on the cart page: the store's page counts it.
  await cart.getByRole("button", { name: "Increase quantity of Espresso" }).click();
  await expect(cart.locator("#cart-count")).toHaveText("2");
  await expect(store.locator("#cart-count")).toHaveText("2");

  // Removed on the cart page: both are empty.
  await cart.getByRole("button", { name: "Remove Espresso" }).click();
  await expect(line).toHaveCount(0);
  await expect(cart.locator("#cart-empty")).toBeVisible();
  await expect(store.locator("#cart-count")).toHaveText("0");
});

test("the store's page links to the cart's", async ({ page }) => {
  await ready(page, "/stores/47");
  await page.getByRole("link", { name: "Go to your cart" }).click();
  await expect(page).toHaveURL(/\/cart$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
  await expect(page).toHaveTitle("Your cart");
});
