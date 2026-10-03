// ADR-0162, charter §15.3 and §15.6 test 11: the store at its route,
// `/stores/{id}`, and a second store at its own. A change to one store's menu
// reaches that store's pages, and no other store's. And ADR-0163: a store
// that is not there is not found.
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.stores[testInfo.project.name]}`);
  },
});

// Store 47's menu is one per server: one test at a time.
test.describe.configure({ mode: "serial" });

async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

const names = (page) => page.locator("#menu li span");

test("each store is served at its route", async ({ page }) => {
  await ready(page, "/stores/48");
  await expect(page.locator("#store-name")).toHaveText("Harbor Coffee");
  await expect(names(page)).toHaveText(["Drip Coffee", "Matcha Latte", "Blueberry Scone"]);
  await ready(page, "/stores/47");
  await expect(page.locator("#store-name")).toHaveText("Blue Bottle");
  await expect(names(page)).toHaveText(["Espresso", "Cortado", "Cold Brew"]);
});

test("a store that is not there is not found", async ({ page }) => {
  // The page declares `not_found_on StoreError.NotFound`: 404, where a page
  // whose values cannot be read is 503 (ADR-0147).
  const absent = await page.goto("/stores/999");
  expect(absent.status()).toBe(404);
  await expect(page).toHaveTitle("Not found");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Not found");
  // Nothing of the store's page.
  await expect(page.locator("#menu")).toHaveCount(0);
  await expect(page.locator("#cart-count")).toHaveCount(0);
  // Control: a store that is there.
  expect((await page.goto("/stores/48")).status()).toBe(200);
});

test("the second store's Add adds to the session's cart", async ({ page }) => {
  await ready(page, "/stores/48");
  await expect(page.locator("#cart-count")).toHaveText("0");
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("a change to one store's menu reaches its page, and not another store's (test 11)", async ({
  context,
  request,
}) => {
  // How many times each page navigated: a page sent another store's patch
  // could not apply it, and would read itself again.
  const loads = new Map();
  const opened = async (path) => {
    const page = await context.newPage();
    loads.set(page, 0);
    page.on("framenavigated", (frame) => {
      if (frame === page.mainFrame() && frame.url().includes("/stores/")) {
        loads.set(page, loads.get(page) + 1);
      }
    });
    await ready(page, path);
    return page;
  };
  const blue = await opened("/stores/47");
  const harbor = await opened("/stores/48");
  try {
    const renamed = await request.post("/command/menu?op=rename&id=espresso&name=Espresso%20Doppio");
    expect(renamed.ok()).toBe(true);
    // Store 47's page shows it ...
    await expect(names(blue).first()).toHaveText("Espresso Doppio");
    // ... and store 48's is as it was, and was sent nothing to apply.
    await harbor.waitForTimeout(500);
    await expect(names(harbor)).toHaveText(["Drip Coffee", "Matcha Latte", "Blueberry Scone"]);
    expect([loads.get(blue), loads.get(harbor)]).toEqual([1, 1]);
  } finally {
    await request.post("/command/menu?op=rename&id=espresso&name=Espresso");
  }
});
