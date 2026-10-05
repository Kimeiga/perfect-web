// ADR-0142: a field bound to a signal.
//
// `examples/demo/bind.pw` binds two fields, one at top level and one inside
// a block a signal decides. What this shows, in each engine:
// - the server renders the first value;
// - typing sets the signal and what reads it, in place;
// - a handler that changes the signal changes the field, even while it has
//   focus, and its own typing is never written back over it;
// - a field inside a signal's block keeps its focus, and stays the same
//   element, while typed in;
// - markup typed is text.
//
// ADR-0221: and a `<textarea>`'s first value is its text, shown with
// scripts off; typing sets its signal, and a handler sets it back.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.bind.BindPage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

test("the server renders the first value", async ({ request }) => {
  const html = await (await request.get(PAGE)).text();
  const markup = html.split('<script type="application/json"')[0];
  expect(markup).toMatch(/<input[^>]*id="name"[^>]*value="Ada"/);
  expect(markup).toContain("Hello, <!--pw:");
  expect(markup).toContain(">Ada<!--pw:");
});

test("a textarea's first value is its text, shown with scripts off", async ({
  browser,
  request,
}) => {
  const html = await (await request.get(PAGE)).text();
  const markup = html.split('<script type="application/json"')[0];
  const textarea = markup.match(/<textarea[^>]*id="bio"[^>]*>([^<]*)<\/textarea>/);
  expect(textarea, markup).not.toBeNull();
  expect(textarea[0]).not.toMatch(/<textarea[^>]* value=/);
  expect(textarea[1]).toBe("Writes about\ntype systems.");
  const off = await browser.newContext({ javaScriptEnabled: false });
  const page = await off.newPage();
  await page.goto(PAGE);
  await expect(page.locator("#bio")).toHaveValue("Writes about\ntype systems.");
  await off.close();
});

test("typing in a textarea sets its signal, and a handler sets it back", async ({ page }) => {
  await ready(page);
  const bio = page.locator("#bio");
  await expect(bio).toHaveValue("Writes about\ntype systems.");
  await bio.fill("Grace writes compilers.");
  await expect(page.locator("#bio-echo")).toHaveText("Grace writes compilers.");
  await page.locator("#clear-bio").click();
  await expect(bio).toHaveValue("");
  await expect(page.locator("#bio-echo")).toHaveText("");
});

test("typing sets the signal, and what reads it", async ({ page }) => {
  await ready(page);
  await page.locator("#name").fill("Margaret");
  await expect(page.locator("#hello")).toHaveText("Hello, Margaret");
  await expect(page.locator("#name")).toHaveValue("Margaret");
});

test("a handler that changes the signal changes the field", async ({ page }) => {
  await ready(page);
  await page.locator("#grace").click();
  await expect(page.locator("#name")).toHaveValue("Grace");
  await expect(page.locator("#hello")).toHaveText("Hello, Grace");
  // Even while the field has focus: Enter clears it, by its own handler.
  await page.locator("#name").focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#name")).toHaveValue("");
  await expect(page.locator("#hello")).toHaveText("Hello,");
});

test("the field's own typing is never written back over it", async ({ page }) => {
  await ready(page);
  const name = page.locator("#name");
  // Every value the page sets on the field, recorded. A person's typing is
  // the browser's, and passes no setter.
  await name.evaluate((el) => {
    window.__sets = [];
    const d = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
    Object.defineProperty(el, "value", {
      get() {
        return d.get.call(this);
      },
      set(v) {
        window.__sets.push(v);
        d.set.call(this, v);
      },
    });
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  });
  // Typed while the first key's handler is still loading, so each handler
  // runs after the keys that followed its own.
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/handler/**", async (route) => {
    await held;
    await route.continue();
  });
  await page.keyboard.type("bcd");
  release();
  await expect(page.locator("#hello")).toHaveText("Hello, Adabcd");
  await expect(name).toHaveValue("Adabcd");
  expect(await page.evaluate(() => window.__sets), "typing set nothing").toEqual([]);
  // A handler's change is set.
  await page.locator("#grace").click();
  await expect(name).toHaveValue("Grace");
  expect(await page.evaluate(() => window.__sets)).toEqual(["Grace"]);
});

test("a field in a signal's block keeps its focus while typed in", async ({ page }) => {
  await ready(page);
  await page.locator("#edit").click();
  const note = page.locator("#note");
  await expect(note).toBeVisible();
  await note.evaluate((el) => {
    el.__mark = "same";
  });
  await note.focus();
  await page.keyboard.type("hello");
  await expect(page.locator("#note-echo")).toHaveText("hello");
  expect(await note.evaluate((el) => el.__mark)).toBe("same");
  expect(await page.evaluate(() => document.activeElement?.id)).toBe("note");
});

test("markup typed is text", async ({ page }) => {
  await ready(page);
  await page.locator("#name").fill("<b>x</b> & y");
  await expect(page.locator("#hello")).toHaveText("Hello, <b>x</b> & y");
  await expect(page.locator("#hello b")).toHaveCount(0);
});
