// ADR-0172, charter §15.3: the cart lists its lines, and a line can be
// increased, decreased and removed. A change shows before the server answers,
// in every part that reads the cart at once, and is the server's after: a
// speculation reaches every part that reads it.
import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.cart[testInfo.project.name]}`);
  },
});

// An item sold out is one per server: one test at a time.
test.describe.configure({ mode: "serial" });

// What the runtime did, beside a failure: which frames it applied, which
// speculations it made and dropped.
test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status === testInfo.expectedStatus) return;
  const log = await page.evaluate(() => (window.__pw?.log ?? []).join("\n")).catch(() => "");
  const path = testInfo.outputPath("runtime-log.txt");
  writeFileSync(path, log);
  await testInfo.attach("runtime log", { path, contentType: "text/plain" });
});

/** The store, ready, counting each time the page is read again: a patch
 * the document refused would read it again. */
async function ready(page) {
  const loads = { count: 0 };
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) loads.count += 1;
  });
  await page.goto("/stores/47");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  return loads;
}

const lines = (page) => page.locator("#cart-lines li");

/** A command's request held until the returned function lets it go, so its
 * speculation shows alone. */
async function holding(page, command) {
  let release;
  const held = new Promise((r) => (release = r));
  const route = `**/command/store.page.${command}`;
  await page.route(route, async (r) => {
    await held;
    await r.continue();
  });
  return async () => {
    release();
    // Each held request goes on before the route is taken away.
    await page.unrouteAll({ behavior: "wait" });
  };
}

/** A control pressed from the keyboard, where focus is a keyboard user's:
 * Safari does not focus a button it is clicked on. */
async function press(locator) {
  await locator.focus();
  await locator.press("Enter");
}

/** What the document refused: none, when every patch found what it was
 * derived from. */
const refused = (page) => page.evaluate(() => window.__pw.refused ?? 0);

test("a line shows before the server answers, with every part that reads the cart, and is the server's after", async ({
  page,
}) => {
  const loads = await ready(page);
  await expect(page.locator("#cart-empty")).toBeVisible();
  await expect(lines(page)).toHaveCount(0);
  await expect(page.locator("#cart-fees")).toHaveCount(0);

  const answer = await holding(page, "add_to_cart");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  // Before the server answers: the line, its name and its price as the menu
  // showed them; the count, the subtotal, the empty message, the fees.
  await expect(lines(page)).toHaveCount(1);
  await expect(lines(page).first()).toContainText("Espresso");
  await expect(lines(page).first()).toContainText("$3.50");
  await expect(page.locator("#cart-count")).toHaveText("1");
  await expect(page.locator("#cart-subtotal")).toHaveText("$3.50");
  await expect(page.locator("#cart-empty")).toBeHidden();
  await expect(page.locator("#cart-fees")).toBeVisible();

  await answer();
  // After: one line, the server's, which its next change reaches.
  await expect.poll(() => page.evaluate(() => window.__pw.log.some((l) => l.startsWith("reconciled cart")))).toBe(true);
  await expect(lines(page)).toHaveCount(1);
  await page.getByRole("button", { name: "Increase quantity of Espresso" }).click();
  await expect(lines(page).first()).toContainText("$7.00");
  await expect(page.locator("#cart-subtotal")).toHaveText("$7.00");
  expect(await refused(page)).toBe(0);
  expect(loads.count).toBe(1);
});

test("two presses before the first answers are two steps, and focus stays on the control", async ({
  page,
}) => {
  await ready(page);
  await page.getByRole("button", { name: "Add Cortado" }).click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const answer = await holding(page, "increase_in_cart");
  const more = page.getByRole("button", { name: "Increase quantity of Cortado" });
  await press(more);
  await press(more);
  await expect(lines(page).first()).toContainText("$12.75");
  await expect(page.locator("#cart-count")).toHaveText("3");
  await expect(more).toBeFocused();
  await answer();
  await expect(page.locator("#cart-count")).toHaveText("3");
  await expect(page.locator("#cart-subtotal")).toHaveText("$12.75");
  await expect(more).toBeFocused();
  expect(await refused(page)).toBe(0);
});

test("one fewer at one takes the line away, and focus goes to the next line's control", async ({
  page,
}) => {
  await ready(page);
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await page.getByRole("button", { name: "Add Cortado" }).click();
  await expect(lines(page)).toHaveCount(2);

  const answer = await holding(page, "decrease_in_cart");
  await press(page.getByRole("button", { name: "Decrease quantity of Espresso" }));
  // Gone before the server answers, and focus on the line that is next.
  await expect(lines(page)).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Decrease quantity of Cortado" })).toBeFocused();
  await expect(page.locator("#cart-subtotal")).toHaveText("$4.25");
  await answer();
  await expect(lines(page)).toHaveCount(1);
  await expect(lines(page).first()).toContainText("Cortado");
  expect(await refused(page)).toBe(0);
});

test("a refused add takes its line away again, and says why", async ({ page, request }) => {
  await ready(page);
  const sold = await request.post("/bench/stock?item=cold-brew&available=false");
  expect(sold.ok()).toBe(true);
  try {
    const answer = await holding(page, "add_to_cart");
    await page.getByRole("button", { name: "Add Cold Brew" }).click();
    await expect(lines(page)).toHaveCount(1);
    await answer();
    await expect(page.locator("#cart-notice")).toHaveText("That item just sold out.");
    await expect(lines(page)).toHaveCount(0);
    await expect(page.locator("#cart-count")).toHaveText("0");
    await expect(page.locator("#cart-empty")).toBeVisible();
    expect(await refused(page)).toBe(0);
  } finally {
    await request.post("/bench/stock?item=cold-brew&available=true");
  }
});

test("remove takes the line away, and focus goes to the cart's heading", async ({ page }) => {
  await ready(page);
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await expect(lines(page)).toHaveCount(1);
  const answer = await holding(page, "remove_from_cart");
  await press(page.getByRole("button", { name: "Remove Espresso" }));
  await expect(lines(page)).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Cart" })).toBeFocused();
  await expect(page.locator("#cart-empty")).toBeVisible();
  await answer();
  await expect(page.locator("#cart-count")).toHaveText("0");
  await expect(page.locator("#cart-fees")).toHaveCount(0);
  expect(await refused(page)).toBe(0);
});

test("a value that includes a press is not shown with the press again, before the press's answer comes", async ({
  page,
}) => {
  await ready(page);
  await page.getByRole("button", { name: "Add Cortado" }).click();
  await expect.poll(() => page.evaluate(() => window.__pw.log.some((l) => l.startsWith("reconciled cart")))).toBe(true);
  await expect(page.locator("#cart-count")).toHaveText("1");

  // The server commits the press and its change comes, while its answer is
  // held: the value the page is sent includes the press, and names it.
  const frames = () => page.evaluate(() => window.__pw.log.filter((l) => l.startsWith("resource ")).length);
  const before = await frames();
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/store.page.increase_in_cart", async (route) => {
    const response = await route.fetch();
    await held;
    await route.fulfill({ response });
  });
  await press(page.getByRole("button", { name: "Increase quantity of Cortado" }));
  await expect.poll(frames).toBeGreaterThan(before);
  // Two, the server's, and not three, the press shown over the value that
  // already includes it, for as long as the answer does not come.
  for (let i = 0; i < 10; i += 1) {
    await expect(page.locator("#cart-count")).toHaveText("2");
    await page.waitForTimeout(50);
  }
  release();
  await page.unrouteAll({ behavior: "wait" });
  await expect(page.locator("#cart-count")).toHaveText("2");
  await expect(page.locator("#cart-subtotal")).toHaveText("$8.50");
  expect(await refused(page)).toBe(0);
});

test("focus on a line made before the server answered stays on its control after", async ({ page }) => {
  await ready(page);
  const answer = await holding(page, "add_to_cart");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  const more = page.getByRole("button", { name: "Increase quantity of Espresso" });
  await more.focus();
  await expect(more).toBeFocused();
  // The browser's line goes, and the server's takes its place.
  await answer();
  await expect.poll(() => page.evaluate(() => window.__pw.log.some((l) => l.startsWith("reconciled cart")))).toBe(true);
  await expect(lines(page)).toHaveCount(1);
  await expect(more).toBeFocused();
  expect(await refused(page)).toBe(0);
});
