// ADR-0175, charter §15.5: a one-shot network error the server makes, and a
// forced reconnect. The server closes the session's next command connection
// with no answer, before the command runs or after it has committed; and it
// ends the session's subscriptions, refusing new ones for a while. A press
// survives either drop as one mutation (ADR-0173), and a page cut off hears,
// once it is back, what changed meanwhile.
//
// Each fault is the session's own, so the suite shares its host.
import { expect, test } from "@playwright/test";

/** The store, ready. */
async function ready(page) {
  await page.goto("/stores/47");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

const logged = (page, prefix) =>
  page.evaluate((p) => (window.__pw?.log ?? []).filter((l) => l.startsWith(p)).length, prefix);

for (const at of ["before", "after"]) {
  test(`a command whose connection is dropped ${at} it runs is sent again, and is one line`, async ({
    page,
  }) => {
    await ready(page);
    expect((await page.request.post(`/bench/drop?next=command&at=${at}`)).ok()).toBe(true);
    await page.getByRole("button", { name: "Add Espresso" }).click();
    await expect.poll(() => logged(page, "resent store.page.add_to_cart")).toBe(1);
    await expect.poll(() => logged(page, "reconciled cart")).toBeGreaterThan(0);
    await expect(page.locator("#cart-count")).toHaveText("1");
    await expect(page.locator("[data-pw-handler-error]")).toHaveCount(0);
    // The server's: one line, though the command reached it twice.
    await ready(page);
    await expect(page.locator("#cart-count")).toHaveText("1");
  });
}

test("a page cut off from its subscription hears, once it is back, what changed meanwhile", async ({
  page,
}) => {
  await ready(page);
  const reconnects = () => page.evaluate(() => window.__pw.reconnects ?? 0);
  expect((await page.request.post("/bench/reconnect?for=1500")).ok()).toBe(true);
  // Cut off: its subscription ends, and asking again is refused.
  await expect.poll(reconnects).toBeGreaterThan(0);
  // A change while it is cut off: the press is answered, and its change is
  // queued for the page.
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(page.locator("#cart-count")).toHaveText("1");
  // Back: the change reaches it, and the press is reconciled by it.
  await expect
    .poll(() => logged(page, "reconciled cart"), { timeout: 15_000 })
    .toBeGreaterThan(0);
  expect(await logged(page, "subscription failed")).toBeGreaterThan(0);
  await expect(page.locator("#cart-count")).toHaveText("1");
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("1");
});
