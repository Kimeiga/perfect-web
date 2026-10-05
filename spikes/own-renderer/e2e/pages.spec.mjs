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

/** The cart page with one Espresso in its session's cart. */
async function oneEspresso(page) {
  await ready(page, "/stores/47");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  await ready(page, "/cart");
}

test("a press on the cart's page is shown before the server answers", async ({ page }) => {
  // ADR-0191: the cart's page speculates from its own module, as the
  // store's does. The command's request is held at the network, so "before"
  // is observed rather than inferred from timing.
  await oneEspresso(page);
  const held = [];
  await page.route("**/command/store.page.increase_in_cart", async (route) => {
    await new Promise((resolve) => held.push(resolve));
    await route.continue();
  });
  await page.getByRole("button", { name: "Increase quantity of Espresso" }).click();
  await expect.poll(() => held.length).toBe(1);
  await expect(page.locator("#cart-count")).toHaveText("2");
  held.splice(0).forEach((resolve) => resolve());
  await expect(page.locator("#cart-count")).toHaveText("2");
  await page.unroute("**/command/store.page.increase_in_cart");
  // And the server agrees, read from a fresh document.
  await ready(page, "/cart");
  await expect(page.locator("#cart-count")).toHaveText("2");
});

test("a press the server refuses is restored on the cart's page", async ({ page }) => {
  await oneEspresso(page);
  await page.route("**/command/store.page.increase_in_cart", (route) =>
    route.fulfill({ status: 202, contentType: "application/json", body: '{"committed":false}' }),
  );
  await page.getByRole("button", { name: "Increase quantity of Espresso" }).click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect
    .poll(() => page.evaluate(() => window.__pw.log.join("\n")))
    .toMatch(/restored cart/);
});

test("the stores are the home page, each a link to its own", async ({ page }) => {
  // ADR-0192: a delivery site's first page.
  await ready(page, "/");
  await expect(page).toHaveTitle("Stores");
  const stores = page.locator("#stores li");
  await expect(stores.getByRole("heading", { level: 2 })).toHaveText(["Blue Bottle", "Harbor Coffee"]);
  await page.getByRole("link", { name: "Harbor Coffee" }).click();
  await expect(page).toHaveURL(/\/stores\/48$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
  await expect(page.locator("#store-name")).toHaveText("Harbor Coffee");
});

test("the home page counts the session's cart as it changes", async ({ context }) => {
  const home = await context.newPage();
  const store = await context.newPage();
  await ready(home, "/");
  await expect(home.locator("#cart-count")).toHaveText("0");
  await ready(store, "/stores/47");
  await store.getByRole("button", { name: "Add Espresso" }).click();
  await expect(home.locator("#cart-count")).toHaveText("1");
});

test("an order is placed from the cart, and its page follows it as the store moves it along", async ({
  context,
}) => {
  // ADR-0193: the cart, placed as an order; the order's page, kept current
  // as the store says where it is.
  const page = await context.newPage();
  await oneEspresso(page);
  await page.getByRole("button", { name: "Place order" }).click();
  await expect(page.locator("#cart-notice")).toHaveText("Your order is placed.");
  await expect(page.locator("#cart-count")).toHaveText("0");
  await page.getByRole("link", { name: "Follow your order" }).click();
  await expect(page).toHaveURL(/\/order$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
  await expect(page).toHaveTitle("Your order");
  const status = page.locator("#order-status");
  await expect(status).toHaveText("Placed: the store has your order.");
  // The store moves it along, and the page open on it says so, unreloaded.
  for (const [step, said] of [
    ["preparing", "Preparing: the store is making it."],
    ["on-the-way", "On its way to you."],
    ["delivered", "Delivered."],
  ]) {
    const answer = await page.request.post(`/bench/order?status=${step}`);
    expect(answer.ok()).toBe(true);
    await expect(status).toHaveText(said);
  }
});

test("an empty cart places no order, and says so", async ({ page }) => {
  await ready(page, "/cart");
  await page.getByRole("button", { name: "Place order" }).click();
  await expect(page.locator("#cart-notice")).toHaveText(
    "Your cart is empty: there is nothing to order.",
  );
  await expect(page.locator("#order-link")).toHaveCount(0);
});
