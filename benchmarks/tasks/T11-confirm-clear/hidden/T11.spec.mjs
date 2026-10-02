// T11's hidden tests: Clear asks first, in a modal dialog named by its title.
// The dialog takes focus and keeps the page behind it out of reach. It closes
// on Escape and on "Keep items" without clearing, and clears on "Clear cart".
// Focus goes back to Clear whenever it closes, and Clear asks again.
//
// Read from roles, names and focus, never from a stack's markup: a native
// `<dialog>` and an ARIA one that does the same pass alike.

import { test, expect } from "./fixtures.mjs";

/** Every mutation request the page sends from now on. */
function mutations(page) {
  const sent = [];
  page.on("request", (r) => {
    if (r.method() === "POST") sent.push(r.url());
  });
  return sent;
}

/** Press Clear, and the dialog it opens. */
async function ask(page) {
  await page.locator("#clear-cart").click();
  const dialog = page.getByRole("dialog", { name: "Clear your cart?" });
  await expect(dialog).toBeVisible();
  return dialog;
}

const focusedInside = (dialog) => dialog.evaluate((d) => d.contains(document.activeElement));
const focusedOnClear = (page) =>
  page.evaluate(() => document.activeElement?.id === "clear-cart");

test("Clear asks before it clears", async ({ page, store }) => {
  await store.open();
  await store.add();
  await expect(store.count()).toHaveText("1");
  const sent = mutations(page);
  const dialog = await ask(page);
  await expect(dialog.getByRole("button", { name: "Keep items" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Clear cart" })).toBeVisible();
  await page.waitForTimeout(300);
  expect(sent, "asking clears nothing").toEqual([]);
  await expect(store.count()).toHaveText("1");
});

test("the dialog is modal: it takes focus, and the page behind it is out of reach", async ({
  page,
  store,
}) => {
  await store.open();
  const dialog = await ask(page);
  await expect.poll(() => focusedInside(dialog), "focus moved into it").toBe(true);
  expect(
    await dialog.evaluate((d) => d.matches(":modal") || d.getAttribute("aria-modal") === "true"),
    "announced as modal",
  ).toBe(true);
  // A pointer at Clear's place does not reach Clear ...
  const reached = await page.locator("#clear-cart").evaluate((b) => {
    const r = b.getBoundingClientRect();
    const hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
    return hit !== null && hit.closest("#clear-cart") !== null;
  });
  expect(reached, "the pointer reaches the page behind it").toBe(false);
  // ... and neither does the keyboard.
  for (let i = 0; i < 6; i++) {
    await page.keyboard.press("Tab");
    expect(await focusedOnClear(page), "Tab reached the page behind it").toBe(false);
  }
});

test("Keep items closes it, keeps the cart, and gives focus back", async ({ page, store }) => {
  await store.open();
  await store.add();
  await expect(store.count()).toHaveText("1");
  const dialog = await ask(page);
  await dialog.getByRole("button", { name: "Keep items" }).click();
  await expect(dialog).toBeHidden();
  await expect.poll(() => focusedOnClear(page), "focus is back on Clear").toBe(true);
  await expect(store.count()).toHaveText("1");
  await store.open();
  await expect(store.count(), "and the server kept it").toHaveText("1");
});

test("Escape closes it, keeps the cart, and Clear asks again", async ({ page, store }) => {
  await store.open();
  await store.add();
  await expect(store.count()).toHaveText("1");
  const dialog = await ask(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect.poll(() => focusedOnClear(page), "focus is back on Clear").toBe(true);
  await expect(store.count()).toHaveText("1");
  // The page knows it closed: Clear opens it again.
  const again = await ask(page);
  await expect.poll(() => focusedInside(again)).toBe(true);
});

test("from the keyboard alone", async ({ page, store }) => {
  await store.open();
  await page.locator("#clear-cart").focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Clear your cart?" });
  await expect(dialog).toBeVisible();
  await expect.poll(() => focusedInside(dialog)).toBe(true);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect.poll(() => focusedOnClear(page)).toBe(true);
});

test("Clear cart clears, and closes it", async ({ page, store }) => {
  await store.open();
  await store.add();
  await store.add();
  await expect(store.count()).toHaveText("2");
  const dialog = await ask(page);
  const answered = page.waitForResponse((r) => r.request().method() === "POST");
  await dialog.getByRole("button", { name: "Clear cart" }).click();
  await answered;
  await expect(store.count()).toHaveText("0");
  await expect(dialog).toBeHidden();
  await store.open();
  await expect(store.count(), "the server has nothing").toHaveText("0");
});
