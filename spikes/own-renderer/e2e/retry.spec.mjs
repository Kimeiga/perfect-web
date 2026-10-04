// ADR-0173, charter §15.4: "bounded retry only for safe transport failures".
// A command whose request gets no answer is sent again, with the same
// interaction, as many times as its `retry` clause says. The server runs a
// command once for its interaction, and answers a request sent again with
// the first's outcome (ADR-0121). An answer, a refusal included, is never
// sent again.
//
// Each fault is the network's, made in the browser, so every test reads only
// its own session's cart: the suite shares its host.
import { expect, test } from "@playwright/test";

const ADD = "**/command/store.page.add_to_cart";

/** The store, ready. */
async function ready(page) {
  await page.goto("/stores/47");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

const lines = (page) => page.locator("#cart-lines li");
const logged = (page, prefix) =>
  page.evaluate((p) => (window.__pw?.log ?? []).filter((l) => l.startsWith(p)).length, prefix);
/** A press that failed, as the runtime marks its control. */
const failed = (page) => page.locator("[data-pw-handler-error]");

test("a request the network drops is sent again, its line shown meanwhile, and is the server's", async ({
  page,
}) => {
  await ready(page);
  let sent = 0;
  let release;
  const second = new Promise((r) => (release = r));
  await page.route(ADD, async (route) => {
    sent += 1;
    if (sent === 1) {
      // It never reaches the server.
      await route.abort("failed");
    } else {
      await second;
      await route.continue();
    }
  });
  await page.getByRole("button", { name: "Add Espresso" }).click();
  // Sent again, and the line the press made stays while it is.
  await expect.poll(() => logged(page, "resent store.page.add_to_cart")).toBe(1);
  await expect.poll(() => sent).toBe(2);
  await expect(lines(page)).toHaveCount(1);
  await expect(page.locator("#cart-count")).toHaveText("1");
  release();
  await expect.poll(() => logged(page, "reconciled cart")).toBeGreaterThan(0);
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(page.locator("#cart-notice")).toHaveText("");
  await expect(failed(page)).toHaveCount(0);
  await page.unrouteAll({ behavior: "wait" });
  // The server's: a page read again shows the one line.
  await ready(page);
  await expect(lines(page)).toHaveCount(1);
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("an answer the network drops after the server committed is sent again, and the line is added once", async ({
  page,
}) => {
  await ready(page);
  let sent = 0;
  await page.route(ADD, async (route) => {
    sent += 1;
    if (sent === 1) {
      // The server commits; its answer never comes.
      await route.fetch();
      await route.abort("failed");
    } else {
      await route.continue();
    }
  });
  // The resend's answer: the first was never the page's. The commit's own
  // change can come before it, and the press is not shown twice meanwhile.
  const answered = page.waitForResponse((r) => r.url().includes("/command/store.page.add_to_cart"));
  await page.getByRole("button", { name: "Add Espresso" }).click();
  expect((await answered).ok()).toBe(true);
  expect(sent).toBe(2);
  expect(await logged(page, "resent store.page.add_to_cart")).toBe(1);
  await expect.poll(() => logged(page, "reconciled cart")).toBeGreaterThan(0);
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(failed(page)).toHaveCount(0);
  await page.unrouteAll({ behavior: "wait" });
  // Once, though it reached the server twice.
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(lines(page).first()).toContainText("$3.50");
});

test("an answer is an outcome, and a refused request is not sent again", async ({ page }) => {
  await ready(page);
  let sent = 0;
  await page.route(ADD, async (route) => {
    sent += 1;
    await route.fulfill({ status: 500, body: "" });
  });
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(failed(page)).toHaveCount(1);
  expect(sent).toBe(1);
  expect(await logged(page, "resent ")).toBe(0);
  // The press failed: its line is taken away again.
  await expect(lines(page)).toHaveCount(0);
  await expect(page.locator("#cart-count")).toHaveText("0");
});

test("after the resends its clause allows, the press fails and its line is taken away", async ({
  page,
}) => {
  await ready(page);
  let sent = 0;
  await page.route(ADD, async (route) => {
    sent += 1;
    await route.abort("failed");
  });
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(failed(page)).toHaveCount(1, { timeout: 15_000 });
  // `retry transport_only(max = 2, jitter = true)`: the request and two more.
  expect(sent).toBe(3);
  expect(await logged(page, "resent store.page.add_to_cart")).toBe(2);
  await expect(lines(page)).toHaveCount(0);
  await expect(page.locator("#cart-count")).toHaveText("0");
});

test("a command that declares no retry is sent once", async ({ page }) => {
  // `demo.pick`'s `pick` is idempotent and declares no `retry`.
  await page.goto("/page/demo.pick.PickPage?first=espresso&second=cortado");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  let sent = 0;
  await page.route("**/command/demo.pick.pick", async (route) => {
    sent += 1;
    await route.abort("failed");
  });
  await page.locator("#first button").click();
  await expect(failed(page)).toHaveCount(1);
  expect(sent).toBe(1);
  expect(await logged(page, "resent ")).toBe(0);
});
