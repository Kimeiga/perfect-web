// ADR-0141: a `<dialog>` a signal's block renders is the browser's modal
// dialog.
//
// `examples/demo/dialog.pw` opens a confirmation from a button. What this
// shows, in each engine:
// - the dialog is modal, named by its heading, and takes focus where its
//   `autofocus` says;
// - the page behind it is inert;
// - Escape closes it, its `close` handler tells the signal, and focus goes
//   back to the button that opened it, which opens it again;
// - a button in it closes it the same way, and its action runs.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.dialog.DialogPage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

async function open(page) {
  await page.locator("#clear").click();
  const dialog = page.getByRole("dialog", { name: "Clear your cart?" });
  await expect(dialog).toBeVisible();
  return dialog;
}

const focused = (page) => page.evaluate(() => document.activeElement?.id ?? null);

test("the dialog is modal, named, and takes focus where it says", async ({ page }) => {
  await ready(page);
  await expect(page.locator("dialog")).toHaveCount(0);
  await open(page);
  expect(await page.locator("#confirm").evaluate((d) => d.matches(":modal"))).toBe(true);
  await expect.poll(() => focused(page)).toBe("keep");
});

test("the page behind it is inert", async ({ page }) => {
  await ready(page);
  await open(page);
  // What a pointer at the opener's place reaches is not the opener.
  const reached = await page.locator("#clear").evaluate((b) => {
    const r = b.getBoundingClientRect();
    return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.id ?? null;
  });
  expect(reached).not.toBe("clear");
  // And focus does not move into it.
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("Tab");
    expect(await focused(page)).not.toBe("clear");
  }
});

test("Escape closes it, the signal hears, and focus goes back", async ({ page }) => {
  await ready(page);
  await open(page);
  await page.keyboard.press("Escape");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect.poll(() => focused(page)).toBe("clear");
  // The signal says it is shut, so the button opens it again.
  await open(page);
  await expect.poll(() => focused(page)).toBe("keep");
});

test("a button in it closes it the same way", async ({ page }) => {
  await ready(page);
  await open(page);
  await page.locator("#keep").click();
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect.poll(() => focused(page)).toBe("clear");
  await expect(page.locator("#cleared")).toHaveText("0");
});

test("its action runs, and it closes", async ({ page }) => {
  await ready(page);
  await open(page);
  await page.locator("#yes").click();
  await expect(page.locator("#cleared")).toHaveText("1");
  await expect(page.locator("dialog")).toHaveCount(0);
  await expect.poll(() => focused(page)).toBe("clear");
});
