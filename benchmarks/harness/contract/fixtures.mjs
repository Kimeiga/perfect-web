// What a contract test may know about the stack it runs against: where the
// store page is, and when the page is ready for interaction.
import { test as base, expect } from "@playwright/test";

async function pressed(p, button) {
  const answered = p.waitForResponse((r) => r.request().method() === "POST");
  await button.click();
  await answered;
}

export const test = base.extend({
  storePath: ["/", { option: true }],
  stack: ["", { option: true }],
  store: async ({ page, storePath, stack }, use) => {
    await use({
      async open(p = page) {
        await p.goto(storePath);
        if (stack === "pleris") {
          // The own renderer attaches handlers after its resume decision;
          // a click before that is inert by design (E7V).
          await p.waitForFunction(() => document.documentElement.dataset.pwReady);
        } else {
          // Hydration. Both frameworks' forms also work before it, as plain
          // form posts, but a click racing hydration measures the race.
          await p.waitForLoadState("networkidle");
        }
      },
      // A press, and its request's answer. The Pleris page shows the count
      // before the round trip (ADR-0122), so the count alone no longer says
      // the server has the change; every stack's mutation is a POST.
      add: (p = page, n = 0) => pressed(p, p.locator("#menu button").nth(n)),
      // A press of Clear, and its request's answer. A store that asks first
      // (T11) is answered "Clear cart" in its dialog: the contract is that
      // Clear empties the cart, not how many presses it takes.
      clear: async (p = page) => {
        const answered = p.waitForResponse((r) => r.request().method() === "POST");
        await p.locator("#clear-cart").click();
        const confirm = p.getByRole("dialog").getByRole("button", { name: "Clear cart" });
        const asked = await Promise.race([
          answered.then(() => false),
          confirm.waitFor({ state: "visible", timeout: 5000 }).then(
            () => true,
            () => false,
          ),
        ]);
        if (asked) await confirm.click();
        await answered;
      },
      count: (p = page) => p.locator("#cart-count"),
      // The kitchen sets the session's order (T04): the same hook on every
      // stack, carrying the page's session.
      order: async (status, p = page) => {
        const r = await p.request.post(`/bench/order?status=${status}`);
        if (!r.ok()) throw new Error(`/bench/order?status=${status}: ${r.status()}`);
      },
      // How the recommender behaves (T05): `delay=…&fail=…&items=…`, the
      // same hook on every stack. One per server.
      recommend: async (query = "", p = page) => {
        const r = await p.request.post(`/bench/recommendations?${query}`);
        if (!r.ok()) throw new Error(`/bench/recommendations?${query}: ${r.status()}`);
      },
      // How the estimator behaves for the page's session (T10):
      // `delay=…&fail=…&minutes=…`, the same hook on every stack.
      estimate: async (query = "", p = page) => {
        const r = await p.request.post(`/bench/estimate?${query}`);
        if (!r.ok()) throw new Error(`/bench/estimate?${query}: ${r.status()}`);
      },
      // The store's notice board (T09): post a notice at the source, with
      // no event, and read how often the board was asked.
      notice: async (text, p = page) => {
        const r = await p.request.post(`/bench/notice?text=${encodeURIComponent(text)}`);
        if (!r.ok()) throw new Error(`/bench/notice: ${r.status()}`);
      },
      noticeCalls: async (p = page) => {
        const r = await p.request.get("/bench/calls");
        if (!r.ok()) throw new Error(`/bench/calls: ${r.status()}`);
        return (await r.json()).notice;
      },
      // The page takes a press, without waiting for it to finish loading: a
      // page still streaming has not loaded. The Pleris page attaches its
      // handlers after its resume decision (E7V). Both frameworks' forms
      // work before hydration, as plain form posts.
      ready: async (p = page) => {
        if (stack === "pleris") {
          await p.waitForFunction(() => document.documentElement.dataset.pwReady);
        }
      },
    });
  },
});

export { expect };
