// Track `scheduled-orders`: an order placed for a time its reader chooses.
// The store's build served with the development provider (`PW_PAYMENTS=dev`)
// on hosts of its own, one per engine: a test here moves its host's clock
// forward, which would sweep another suite's order awaiting its payment
// (ADR-0334). The reader chooses a time on the page of times, pays inside the
// provider's frame, is told when the order is for, and, the clock moved past
// when its store is told, that it is placed.
//
// Each test's steps are named, and a press's navigation that does not come
// fails at once, in the page's own words where it says why, and otherwise
// with where the page is and what its runtime last did (`navigated`): train-10's
// verify 39131257549 saw one of these waits time out in Chromium, and its
// record said neither which nor why. A failure also attaches the runtime's
// log and the host's records of the page's document (`record`).
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

/** When the running test began: a wait below is bounded by what is left of
 * its time, so that it fails with its own words before the test's timeout. */
let started = 0;
test.beforeEach(() => {
  started = Date.now();
});

/** **What the page and its host knew**, where a test failed: the runtime's
 * log, and the host's trail of the document shown, its slow holds and its
 * tellings (`/bench/records`, as `feed.spec.mjs` records them), attached,
 * and their latest lines printed (`pw-record`). */
test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status === testInfo.expectedStatus) return;
  const runtime = await page
    .evaluate(() => ({
      url: location.href,
      log: window.__pw?.log ?? null,
      doc: JSON.parse(document.getElementById("pw-parts")?.textContent ?? "{}").cursor ?? null,
    }))
    .catch((e) => ({ unavailable: String(e) }));
  await testInfo.attach("runtime", {
    body: JSON.stringify(runtime, null, 2),
    contentType: "application/json",
  });
  const host = await page.request
    .get(`/bench/records?doc=${runtime.doc ?? 0}`)
    .then((r) => r.json())
    .catch((e) => ({ unavailable: String(e) }));
  await testInfo.attach("host", { body: JSON.stringify(host, null, 2), contentType: "application/json" });
  const latest = (list, n) => (Array.isArray(list) ? list.slice(-n) : list);
  console.log(
    `pw-record ${JSON.stringify({
      url: runtime.url,
      log: latest(runtime.log, 40),
      trail: latest(host.trail, 60),
      slow: latest(host.slow, 20),
      tellings: latest(host.tellings, 40),
    })}`,
  );
});

/**
 * **A navigation, or why there was none** (ADR-0280): the page at the
 * address `to`; or, where the page says first why it stayed (its notice
 * `said`, the platform's refusal by the press, ADR-0302, or a handler's
 * error), a failure at once in those words, named by `step`. Bounded by what
 * is left of the test's time, so a page that neither goes nor says fails
 * here, with where it is and its runtime's last lines.
 */
async function navigated(page, step, to, said = null) {
  const deadline = started + test.info().timeout - 2_000;
  let outcome = null;
  while (outcome === null) {
    try {
      const shown = await page.waitForFunction(
        ({ to, said }) => {
          if (new RegExp(to).test(location.pathname)) return { went: true };
          const words = (selector) =>
            selector ? (document.querySelector(selector)?.textContent ?? "").trim() : "";
          const notice = words(said);
          const refusal = words(".pw-refusal");
          const failed =
            document.querySelector("[data-pw-handler-error]")?.getAttribute("data-pw-handler-error") ??
            "";
          return notice || refusal || failed ? { notice, refusal, failed } : false;
        },
        { to: to.source, said },
        { polling: 50, timeout: Math.max(1_000, deadline - Date.now()) },
      );
      outcome = await shown.jsonValue();
    } catch (e) {
      // A whole load replaced the document the check ran in: the next one is
      // checked, while there is time.
      const replaced = /context was destroyed|navigat/i.test(String(e.message));
      if (!(replaced && Date.now() < deadline - 500)) {
        outcome = { waited: String(e.message).split("\n")[0] };
      }
    }
  }
  if (outcome.went) {
    await page.waitForURL(to);
    return;
  }
  const where = page.url();
  const log = await page
    .evaluate(() => (window.__pw?.log ?? []).slice(-12).join(" | "))
    .catch((e) => `unavailable: ${e}`);
  throw new Error(`${step}: did not go to ${to}, at ${where}: ${JSON.stringify(outcome)}; runtime: ${log}`);
}

/** A page, ready to be pressed. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** "Mon 6:30 pm" and "6:30 pm": an instant as the store's zone says it, a time
 * as the store's hours say one (ADR-0328). */
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
  const time = `${parts.hour}:${parts.minute} ${parts.dayPeriod.toLowerCase()}`;
  return { whole: `${parts.weekday} ${time}`, time };
}

/** **The host's clock moved to a minute past a quarter hour**, so the first
 * window offered is known: the first quarter hour more than an hour on. */
async function aMinutePastAQuarter(request) {
  return test.step("the host's clock at a minute past a quarter hour", async () => {
    const { now } = await (await request.get("/bench/clock")).json();
    const at = (Math.floor(now / QUARTER) + 1) * QUARTER + MINUTE;
    expect((await request.post(`/bench/clock?at=${at}`)).status()).toBe(200);
    return { first: at - MINUTE + 75 * MINUTE };
  });
}

/** An espresso in the reader's cart, and the page of times open from it. */
async function anEspressoAndItsTimes(page) {
  await test.step("an espresso in the cart", async () => {
    await ready(page, "/stores/47");
    const added = page.waitForResponse("**/command/store.page.add_to_cart");
    await page.getByRole("button", { name: "Add Espresso" }).click();
    await added;
    await ready(page, "/cart");
    await expect(page.locator("#cart-when")).toContainText("As soon as possible");
  });
  await test.step("the page of times, from the cart's link", async () => {
    await page.getByRole("link", { name: "Choose a time" }).click();
    await navigated(page, "the page of times, from the cart's link", /\/cart\/when$/);
    await expect(page.locator("#when-said")).toHaveText("As soon as possible");
  });
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
  const window = page.locator("#when-windows button").first();
  await test.step("the page of times keeps every rule, and offers the first window", async () => {
    expect(await page.evaluate(violations)).toEqual([]);
    await expect(window).toHaveText(when.time);
  });
  await test.step("the first window chosen, and back at the cart", async () => {
    const chosen = page.waitForResponse("**/command/store.page.choose_time");
    await window.click();
    await chosen;
    await navigated(page, "the first window chosen, and back at the cart", /\/cart$/, "#when-notice");
  });
  await test.step("the cart says when, and the order is placed", async () => {
    await expect(page.locator("#cart-when")).toContainText(`Scheduled for ${when.whole}`);
    await page.getByRole("button", { name: "Place order" }).click();
    await navigated(page, "the cart says when, and the order is placed", /\/order$/, "#cart-notice");
  });
  await test.step("the order awaits its payment, paid in the provider's frame", async () => {
    await expect(page.locator("#order-status")).toHaveText(AWAITING);
    const frame = page.frameLocator("#payment");
    await frame.getByLabel("Card number").fill("4242424242424242");
    await frame.getByRole("button", { name: /^Pay / }).click();
  });
  await test.step("paid now, its store told nothing yet: the page says when", async () => {
    await expect(page.locator("#order-status")).toHaveText(`Scheduled for ${when.whole}.`);
    await expect(page.locator("#order-total")).toHaveText(/^Total charged: \$\d/);
  });
  await test.step("a minute before its store is told, scheduled still", async () => {
    expect((await request.post(`/bench/clock?at=${sends - MINUTE}`)).status()).toBe(200);
    await page.waitForTimeout(1500);
    await expect(page.locator("#order-status")).toHaveText(`Scheduled for ${when.whole}.`);
  });
  await test.step("past it, its store has it, and the page open on it is told", async () => {
    expect((await request.post(`/bench/clock?at=${sends + 1000}`)).status()).toBe(200);
    await expect(page.locator("#order-status")).toHaveText("Placed: the store has your order.");
  });
});

test("a time no longer offered is refused when the order is placed, and the times now are offered", async ({
  page,
  request,
}) => {
  const { first } = await aMinutePastAQuarter(request);
  await anEspressoAndItsTimes(page);
  await test.step("the first window chosen, and back at the cart", async () => {
    const chosen = page.waitForResponse("**/command/store.page.choose_time");
    await page.locator("#when-windows button").first().click();
    await chosen;
    await navigated(page, "the first window chosen, and back at the cart", /\/cart$/, "#when-notice");
    await expect(page.locator("#cart-when")).toContainText(`Scheduled for ${said(first).whole}`);
  });
  await test.step("its hour comes: the cart is told, and the order is refused by name", async () => {
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
  });
  await test.step("the times offered now begin a quarter hour later", async () => {
    await page.getByRole("link", { name: "Choose a time" }).click();
    await navigated(page, "the page of times, from the cart's link", /\/cart\/when$/);
    await expect(page.locator("#when-windows button").first()).toHaveText(
      said(first + QUARTER).time,
    );
  });
});
