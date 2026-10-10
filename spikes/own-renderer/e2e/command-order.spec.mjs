// ADR-XXXX: a page's commands run in the order it sent them. Each names the
// latest command its page sent and has not been answered (`pw-after`), and
// the server runs it only once that one has run, whichever request the
// network brings first. Found by `navigate.spec.mjs` under load: "Clear",
// pressed after "Place order", arrived first, the cart was emptied, and the
// order was refused as nothing to order.
import { expect, test } from "@playwright/test";

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** The cart page with one Espresso in its session's cart, the add answered
 * before the cart is read (ADR-0268). */
async function oneEspresso(page) {
  await ready(page, "/stores/47");
  const answered = page.waitForResponse("**/command/store.page.add_to_cart");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await answered;
  await ready(page, "/cart");
}

/** Each request the page sends for `command`: its interaction, and the one
 * it names as before it. */
function sent(page, command) {
  const requests = [];
  page.on("request", (r) => {
    if (new URL(r.url()).pathname !== `/command/store.page.${command}`) return;
    const headers = r.headers();
    requests.push({ interaction: headers["pw-interaction"], after: headers["pw-after"] ?? null });
  });
  return requests;
}

/** "Place order", then "Clear", from the keyboard: no pointer to miss a
 * button the order's commit moves (`navigate.spec.mjs`). */
async function placedThenCleared(page) {
  await page.getByRole("button", { name: "Place order" }).press("Enter");
  await page.locator("#clear-cart").press("Enter");
}

const PLACED = "Placed: the store has your order.";

test("a press's command, slow on its way, runs before the next press's", async ({ page }) => {
  await oneEspresso(page);
  // The order's request is slow in transit: the clear's, sent after it,
  // reaches the server first.
  await page.route("**/command/store.page.place_order", async (route) => {
    await new Promise((r) => setTimeout(r, 400));
    await route.continue();
  });
  const orders = sent(page, "place_order");
  const clears = sent(page, "clear_cart");
  await placedThenCleared(page);
  await page.waitForURL(/\/order$/);
  await expect(page.locator("#order-status")).toHaveText(PLACED);
  // The clear named the order, and the server ran it after.
  expect(orders).toEqual([{ interaction: expect.any(String), after: null }]);
  expect(clears).toEqual([{ interaction: expect.any(String), after: orders[0].interaction }]);
});

test("a command answered early is sent again once the one before it is answered", async ({
  page,
}) => {
  await oneEspresso(page);
  // The order's request is held past the server's wait for it (two
  // seconds): the clear, there first, is answered early, running nothing.
  await page.route("**/command/store.page.place_order", async (route) => {
    await new Promise((r) => setTimeout(r, 3000));
    await route.continue();
  });
  const clears = sent(page, "clear_cart");
  const early = page.waitForResponse(
    (r) => new URL(r.url()).pathname === "/command/store.page.clear_cart" && r.status() === 409,
  );
  await placedThenCleared(page);
  expect(await (await early).json()).toMatchObject({ committed: false, early: true });
  await page.waitForURL(/\/order$/);
  await expect(page.locator("#order-status")).toHaveText(PLACED);
  // Sent twice, one interaction: naming the order, then, the order
  // answered, naming none.
  expect(clears.map((c) => c.after)).toEqual([expect.any(String), null]);
  expect(clears[1].interaction).toBe(clears[0].interaction);
});

test("a command after one the network lost is sent again, naming none, once that one has failed", async ({
  page,
}) => {
  await oneEspresso(page);
  // The order never reaches the server: each attempt fails on its way, its
  // retry clause's two resends among them, and the press is told so.
  await page.route("**/command/store.page.place_order", (route) => route.abort("failed"));
  const clears = sent(page, "clear_cart");
  await placedThenCleared(page);
  await expect.poll(() => clears.map((c) => c.after), { timeout: 10_000 }).toEqual([
    expect.any(String),
    null,
  ]);
  await expect(page.locator("#cart-count")).toHaveText("0");
  // Read again: the clear ran, and no order was placed.
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await ready(page, "/cart");
  await expect(page.locator("#cart-count")).toHaveText("0");
  await ready(page, "/order");
  await expect(page.locator("#order-status")).toHaveText("You have no order yet.");
});

test("three presses, each later one on a quicker way, run in the order pressed", async ({
  page,
}) => {
  await ready(page, "/stores/47");
  // Espresso's add slow by 300 ms, the clear by 600, Cortado's add not at
  // all: the server is brought the third, then the first, then the second.
  let adds = 0;
  await page.route("**/command/store.page.add_to_cart", async (route) => {
    adds += 1;
    if (adds === 1) await new Promise((r) => setTimeout(r, 300));
    await route.continue();
  });
  await page.route("**/command/store.page.clear_cart", async (route) => {
    await new Promise((r) => setTimeout(r, 600));
    await route.continue();
  });
  const added = sent(page, "add_to_cart");
  const cleared = sent(page, "clear_cart");
  let answered = 0;
  page.on("response", (r) => {
    const path = new URL(r.url()).pathname;
    if (/^\/command\/store\.page\.(add_to_cart|clear_cart)$/.test(path) && r.status() === 202) {
      answered += 1;
    }
  });
  await page.getByRole("button", { name: "Add Espresso" }).press("Enter");
  await page.locator("#clear-cart").press("Enter");
  await page.getByRole("button", { name: "Add Cortado" }).press("Enter");
  await expect.poll(() => answered).toBe(3);
  // Each named the one before it.
  expect(added[0].after).toBeNull();
  expect(cleared[0].after).toBe(added[0].interaction);
  expect(added[1].after).toBe(cleared[0].interaction);
  // Sent once the ones before it are answered, a press names none.
  await page.getByRole("button", { name: "Add Cold Brew" }).press("Enter");
  await expect.poll(() => answered).toBe(4);
  expect(added[2].after).toBeNull();
  // Read again: Espresso added, then cleared, then Cortado and Cold Brew.
  await page.unrouteAll({ behavior: "ignoreErrors" });
  await ready(page, "/cart");
  await expect(page.locator("#cart-lines li")).toHaveCount(2);
  await expect(page.locator("#cart-lines")).toContainText("Cortado");
  await expect(page.locator("#cart-lines")).toContainText("Cold Brew");
  await expect(page.locator("#cart-lines")).not.toContainText("Espresso");
});
