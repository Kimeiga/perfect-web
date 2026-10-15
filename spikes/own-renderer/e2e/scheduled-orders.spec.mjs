// Track `scheduled-orders`: an order placed for a time its reader chooses.
// The store's build served with the development provider (`PW_PAYMENTS=dev`)
// on hosts of its own, one per engine: a test here moves its host's clock
// forward, which would sweep another suite's order awaiting its payment
// (ADR-0334). The reader chooses a time on the page of times, pays inside the
// provider's frame, is told when the order is for, and, the clock moved past
// when its store is told, that it is placed.
import { expect, test } from "@playwright/test";
import { SCHEDULED_ORDERS_PORTS } from "../playwright.config.mjs";
import { violations } from "./accessibility-rules.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${SCHEDULED_ORDERS_PORTS[testInfo.project.name]}`);
  },
});

// The clock is one for every page on a host: one test at a time.
test.describe.configure({ mode: "serial" });

const MINUTE = 60 * 1000;
const QUARTER = 15 * MINUTE;

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** "Mon 6:30 PM" and "6:30 PM": an instant as the store's zone says it. */
function said(at) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", {
      timeZone: "America/Los_Angeles",
      weekday: "short",
      hour: "numeric",
      minute: "2-digit",
    })
      .formatToParts(new Date(at))
      .map(({ type, value }) => [type, value]),
  );
  const time = `${parts.hour}:${parts.minute} ${parts.dayPeriod}`;
  return { whole: `${parts.weekday} ${time}`, time };
}

/** **The host's clock moved to a minute past a quarter hour**, so the first
 * window offered is known: the first quarter hour more than an hour on. */
async function aMinutePastAQuarter(request) {
  const { now } = await (await request.get("/bench/clock")).json();
  const at = (Math.floor(now / QUARTER) + 1) * QUARTER + MINUTE;
  expect((await request.post(`/bench/clock?at=${at}`)).status()).toBe(200);
  return { first: at - MINUTE + 75 * MINUTE };
}

/** An espresso in the reader's cart, and the page of times open from it. */
async function anEspressoAndItsTimes(page) {
  await ready(page, "/stores/47");
  const added = page.waitForResponse("**/command/store.page.add_to_cart");
  await page.getByRole("button", { name: "Add Espresso" }).click();
  await added;
  await ready(page, "/cart");
  await expect(page.locator("#cart-when")).toContainText("As soon as possible");
  await page.getByRole("link", { name: "Choose a time" }).click();
  await page.waitForURL(/\/cart\/when$/);
  await expect(page.locator("#when-said")).toHaveText("As soon as possible");
}

const AWAITING = "Awaiting your payment: pay below, on the provider's page.";

test("an order for a time is paid now, says when, and is placed when its store is told", async ({
  page,
  request,
}) => {
  const { first } = await aMinutePastAQuarter(request);
  const when = said(first);
  // Sent to its store the window less the estimate's most, 35 minutes with
  // no address chosen, and five more.
  const sends = first - 40 * MINUTE;
  await anEspressoAndItsTimes(page);
  expect(await page.evaluate(violations)).toEqual([]);
  const window = page.locator("#when-windows button").first();
  await expect(window).toHaveText(when.time);
  const chosen = page.waitForResponse("**/command/store.page.choose_time");
  await window.click();
  await chosen;
  await page.waitForURL(/\/cart$/);
  await expect(page.locator("#cart-when")).toContainText(`Scheduled for ${when.whole}`);
  await page.getByRole("button", { name: "Place order" }).click();
  await page.waitForURL(/\/order$/);
  await expect(page.locator("#order-status")).toHaveText(AWAITING);
  const frame = page.frameLocator("#payment");
  await frame.getByLabel("Card number").fill("4242424242424242");
  await frame.getByRole("button", { name: /^Pay / }).click();
  // Paid now, and its store told nothing yet: the page says when.
  await expect(page.locator("#order-status")).toHaveText(`Scheduled for ${when.whole}.`);
  await expect(page.locator("#order-total")).toHaveText(/^Total charged: \$\d/);
  // A minute before its store is told, it is scheduled still.
  expect((await request.post(`/bench/clock?at=${sends - MINUTE}`)).status()).toBe(200);
  await page.waitForTimeout(1500);
  await expect(page.locator("#order-status")).toHaveText(`Scheduled for ${when.whole}.`);
  // Past it, its store has it, and the page open on it is told.
  expect((await request.post(`/bench/clock?at=${sends + 1000}`)).status()).toBe(200);
  await expect(page.locator("#order-status")).toHaveText("Placed: the store has your order.");
});

test("a time no longer offered is refused when the order is placed, and the times now are offered", async ({
  page,
  request,
}) => {
  const { first } = await aMinutePastAQuarter(request);
  await anEspressoAndItsTimes(page);
  const chosen = page.waitForResponse("**/command/store.page.choose_time");
  await page.locator("#when-windows button").first().click();
  await chosen;
  await page.waitForURL(/\/cart$/);
  await expect(page.locator("#cart-when")).toContainText(`Scheduled for ${said(first).whole}`);
  // Its hour comes: the cart is told, and the order is refused by name.
  expect((await request.post(`/bench/clock?at=${first - 60 * MINUTE + 1000}`)).status()).toBe(200);
  await expect(page.locator("#cart-when")).toContainText(
    `${said(first).whole} is no longer offered: choose another time.`,
  );
  const answered = page.waitForResponse("**/command/store.page.place_order");
  await page.getByRole("button", { name: "Place order" }).click();
  await answered;
  await expect(page.locator("#cart-notice")).toHaveText(
    "The time you chose is no longer offered: choose another.",
  );
  await expect(page).toHaveURL(/\/cart$/);
  // The times offered now begin a quarter hour later.
  await page.getByRole("link", { name: "Choose a time" }).click();
  await page.waitForURL(/\/cart\/when$/);
  await expect(page.locator("#when-windows button").first()).toHaveText(
    said(first + QUARTER).time,
  );
});
