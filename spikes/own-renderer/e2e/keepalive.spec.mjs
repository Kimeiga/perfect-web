// **A command outlives the page that sent it** (ADR-0268). A press's
// speculation is shown before its request leaves (ADR-0122); a link followed
// at once ended the page and the request with it, and WebKit lost a press so
// on CI. Each command's request is marked `keepalive`, which the Fetch
// standard lets outlive its document, within 64 KiB of such bodies in flight.
import { expect, test } from "@playwright/test";

async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** Records each command request the page makes: whether it asked to be kept
 * alive, and its body's bytes. */
async function recorded(page) {
  await page.addInitScript(() => {
    const send = window.fetch;
    window.__asked = [];
    window.fetch = (url, init) => {
      if (String(url).startsWith("/command/")) {
        window.__asked.push({
          keepalive: init?.keepalive === true,
          bytes: new Blob([init?.body ?? ""]).size,
        });
      }
      return send(url, init);
    };
  });
}

const asked = (page) => page.evaluate(() => window.__asked);
const keptAlive = (page) => page.evaluate(() => window.__pw.keepalive().inFlight);
const ADD = "**/command/store.page.add_to_cart";

/** Holds every request to `url` at the network until the returned function
 * is called. */
async function hold(page, url) {
  let release;
  const held = new Promise((resolve) => (release = resolve));
  await page.route(url, async (route) => {
    await held;
    await route.continue();
  });
  return release;
}

test("a command's request is kept alive, and counted until it is answered", async ({ page }) => {
  await recorded(page);
  await ready(page, "/stores/47");
  const release = await hold(page, ADD);
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect.poll(async () => (await asked(page)).length).toBe(1);
  const [request] = await asked(page);
  expect(request.keepalive).toBe(true);
  // Counted while it is in flight: its body's bytes.
  expect(await keptAlive(page)).toBe(request.bytes);
  const answered = page.waitForResponse(ADD);
  release();
  await answered;
  await expect.poll(() => keptAlive(page)).toBe(0);
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("past what remains of 64 KiB, a request is sent as before, and answers", async ({ page }) => {
  // One body's bytes, read from a press.
  await recorded(page);
  await ready(page, "/stores/47");
  const answered = page.waitForResponse(ADD);
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await answered;
  const [{ bytes }] = await asked(page);
  // A budget of one such body, lowered as `?keepalive=` lets a test: the
  // first press fits, and the second, its predecessor in flight, does not.
  await ready(page, `/stores/47?keepalive=${bytes}`);
  const release = await hold(page, ADD);
  const add = page.getByRole("button", { name: "Add Espresso" });
  await add.click();
  await expect.poll(async () => (await asked(page)).length).toBe(1);
  await add.click();
  await expect.poll(async () => (await asked(page)).length).toBe(2);
  expect((await asked(page)).map((r) => r.keepalive)).toEqual([true, false]);
  expect(await keptAlive(page)).toBe(bytes);
  release();
  await expect.poll(() => keptAlive(page)).toBe(0);
  // Both are made: the first press's, and the two here.
  await ready(page, "/stores/47");
  await expect(page.locator("#cart-count")).toHaveText("3");
});

test("a press followed at once by a link is still a press", async ({ page }) => {
  await ready(page, "/stores/47");
  const add = page.getByRole("button", { name: "Add Espresso" });
  // The handler's code loaded, by a press answered: a press whose code is
  // still loading when the page is left was never sent.
  const answered = page.waitForResponse(ADD);
  await add.click();
  await answered;
  await expect(page.locator("#cart-count")).toHaveText("1");
  // The press and the link in one task: the request leaves as the page does.
  await add.evaluate((button) => {
    button.click();
    location.assign("/cart");
  });
  await page.waitForURL("**/cart");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
  // In the cart's document, or after it from its subscription.
  await expect(page.locator("#cart-count")).toHaveText("2");
});

test("`?keepalive=` lowers the budget and never raises it", async ({ page }) => {
  const budget = async (query) => {
    await ready(page, `/stores/47${query}`);
    return page.evaluate(() => window.__pw.keepalive().budget);
  };
  // The Fetch standard's 64 KiB, unless a test asks for less.
  expect(await budget("")).toBe(64 * 1024);
  expect(await budget("?keepalive=50")).toBe(50);
  expect(await budget("?keepalive=1000000")).toBe(64 * 1024);
  expect(await budget("?keepalive=none")).toBe(0);
});

test("a request that fails is no longer counted", async ({ page }) => {
  await ready(page, "/stores/47");
  // Every attempt the store's retry clause allows fails at the network.
  await page.route(ADD, (route) => route.abort());
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(page.locator("[data-pw-handler-error]")).toHaveCount(1);
  expect(await keptAlive(page)).toBe(0);
  await expect(page.locator("#cart-count")).toHaveText("0");
});
