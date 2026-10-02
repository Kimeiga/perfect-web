// What a contract test may know about the stack it runs against: where the
// store page is, and when the page is ready for interaction.
import { test as base, expect } from "@playwright/test";

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
      add: (p = page, n = 0) => p.locator("#menu button").nth(n).click(),
      clear: (p = page) => p.locator("#clear-cart").click(),
      count: (p = page) => p.locator("#cart-count"),
    });
  },
});

export { expect };
