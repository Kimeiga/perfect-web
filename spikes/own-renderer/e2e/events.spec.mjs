// ADR-0138: a handler is given its event.
//
// `examples/demo/events.pw` handles an input's `input` and `keydown`, and a
// form's `submit` with `|prevent`. What this shows:
// - each handler listens for its own event: until 2026-10-02 every handler
//   listened for a click, whatever it was written for;
// - a handler is given its event's record, read in the listener;
// - two handlers on one element are each bound;
// - a form's submission is stopped in the listener, before any code loads.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.events.EventsPage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

test("an input's handler runs on input, not on a click", async ({ page }) => {
  await ready(page);
  // A click is not an input: until 2026-10-02 this ran the handler.
  await page.locator("#q").click();
  await page.waitForTimeout(150);
  await expect(page.locator("#echo")).toHaveText("");
  await expect(page.locator("#last")).toHaveText("none");

  await page.locator("#q").pressSequentially("abc");
  await expect(page.locator("#echo")).toHaveText("abc");
});

test("a handler is given what was typed, as text", async ({ page }) => {
  await ready(page);
  await page.locator("#q").pressSequentially("<b>x</b> & y");
  await expect(page.locator("#echo")).toHaveText("<b>x</b> & y");
  await expect(page.locator("#echo b")).toHaveCount(0);
});

test("a key's handler is given the key, beside the input's on one element", async ({
  page,
}) => {
  await ready(page);
  await page.locator("#q").focus();
  await page.keyboard.press("Escape");
  await expect(page.locator("#last")).toHaveText("Escape");
  await page.locator("#q").pressSequentially("z");
  await expect(page.locator("#last")).toHaveText("z");
  await expect(page.locator("#echo")).toHaveText("z");
});

test("a form's submission is stopped before the handler's code loads", async ({ page }) => {
  await ready(page);
  // A mark on this document: a submission that went through would replace it.
  await page.evaluate(() => {
    window.__stayed = true;
  });
  await page.locator("#go").click();
  await expect(page.locator("#sent")).toHaveText("1");
  await page.locator("#go").click();
  await expect(page.locator("#sent")).toHaveText("2");
  expect(await page.evaluate(() => window.__stayed === true)).toBe(true);
  expect(new URL(page.url()).pathname).toBe(PAGE);
});

test("the manifest says each part's event and modifiers", async ({ request }) => {
  const html = await (await request.get(PAGE)).text();
  const json = html.split('<script type="application/json" id="pw-parts">')[1].split("</script>")[0];
  const events = JSON.parse(json)
    .parts.filter((p) => p.kind === "event")
    .map((p) => [p.event, p.modifiers ?? []]);
  expect(events).toEqual([
    ["input", []],
    ["keydown", []],
    ["submit", ["prevent"]],
  ]);
});
