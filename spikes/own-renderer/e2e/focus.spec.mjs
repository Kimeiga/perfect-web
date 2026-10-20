// An element a block shows with `autofocus` takes focus (track `todomvc`,
// ADR-XXXX).
//
// HTML's `autofocus` runs when a page loads. `examples/demo/focus.pw` shows a
// field after it has loaded, as TodoMVC shows its edit field when a person
// asks to edit a todo ("the edit field should be focused"), and the field
// that appears is the one they type in. What this shows, in each engine:
// - a field a block shows with `autofocus` takes focus as it appears, and
//   again each time it appears;
// - a field shown without it does not: focus stays where it was;
// - one that is already focused, or is shown with the page, is left alone.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.focus.FocusPage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

test("a field a block shows with autofocus takes focus as it appears", async ({ page }) => {
  await ready(page);
  await expect(page.locator("#note")).toHaveCount(0);
  await page.locator("#edit").click();
  await expect(page.locator("#note")).toBeFocused();
  // It is the one typed in, with no click on it.
  await page.keyboard.type("!");
  await expect(page.locator("#echo")).toContainText("!");
});

test("it takes focus each time it appears", async ({ page }) => {
  await ready(page);
  await page.locator("#edit").click();
  await expect(page.locator("#note")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.locator("#note")).toHaveCount(0);
  await page.locator("#edit").click();
  await expect(page.locator("#note")).toBeFocused();
});

test("a field shown without autofocus does not take focus", async ({ page }) => {
  await ready(page);
  await page.locator("#hint").click();
  await expect(page.locator("#hint-field")).toBeVisible();
  await expect(page.locator("#hint-field")).not.toBeFocused();
  await expect(page.locator("#hint")).toBeFocused();
});

test("a field that has focus keeps it where another block shows", async ({ page }) => {
  await ready(page);
  await page.locator("#edit").click();
  await expect(page.locator("#note")).toBeFocused();
  // Another block shows without autofocus: focus is the first field's.
  await page.locator("#hint").evaluate((b) => b.click());
  await expect(page.locator("#hint-field")).toBeVisible();
  await expect(page.locator("#note")).toBeFocused();
});
