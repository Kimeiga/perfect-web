// ADR-0121: a command declared `idempotent_by InteractionId` runs once per
// interaction, however many times its request is sent.
//
// `add_to_cart` declared it from E4 on, and until 2026-10-02 nothing read it:
// the runtime sent no interaction, the server ran every request, and a
// retried request added twice. The retry here is made at the NETWORK, by
// sending the browser's own request twice, so the runtime is not edited and
// has no test-only branch.

import { test, expect } from "@playwright/test";

async function ready(page) {
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

test("a request sent twice is one add", async ({ page }) => {
  await ready(page);
  let sent = 0;
  await page.route("**/command/store.page.add_to_cart", async (route) => {
    // The same request, twice, as a retrying proxy or a flaky network would.
    await route.fetch();
    const second = await route.fetch();
    sent += 2;
    await route.fulfill({ response: second });
  });

  await page.locator("#menu button").first().click();
  // Both sends complete before the count is read: the count moves before the
  // round trip (ADR-0122), so it alone says nothing about the server.
  await expect.poll(() => sent).toBe(2);
  await expect(page.locator("#cart-count")).toHaveText("1");

  // The second send's patch, if it had committed, would land after the
  // first's. Reloading reads the server's state rather than waiting on a frame.
  await page.reload();
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("two presses are two interactions", async ({ page }) => {
  // The control: the server is not simply refusing a second add.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("2");
});

test("a command request without an interaction is refused", async ({ page, request }) => {
  await ready(page);
  // Well-formed arguments, the item as its button shows it (ADR-0172), so
  // what is refused is the missing interaction.
  const item = JSON.parse(
    await page.locator("#menu button").first().getAttribute("data-pw-captures"),
  ).item;
  const response = await request.post("/command/store.page.add_to_cart", {
    data: [item, 1],
  });
  expect(response.status()).toBe(400);
  const body = await response.json();
  expect(body.committed).toBe(false);
  expect(body.error).toMatch(/carries no interaction id/);
});
