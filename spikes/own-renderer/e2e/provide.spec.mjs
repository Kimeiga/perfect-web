// ADR-0144: a view's own signals, and a signal a page provides.
//
// `examples/demo/provide.pw` uses the views of `examples/demo/drawer.pw`.
// What this shows, in each engine:
// - each use of a view holds its own signal, and one compiled handler
//   changes each use's own;
// - a button in one view opens a drawer another view shows, through the
//   signal the page provides, and neither view is given it;
// - a view that provides the signal again keeps what it contains apart;
// - a view's signal inside a block starts again when the block shows again.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.provide.ProvidePage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** The manifest the document carries. */
function manifestOf(html) {
  const m = html.match(/<script type="application\/json" id="pw-parts">([\s\S]*?)<\/script>/);
  return JSON.parse(m[1]);
}

test("the server renders each instance at its first value", async ({ browser, request }) => {
  const html = await (await request.get(PAGE)).text();
  const manifest = manifestOf(html);
  // The page's own drawer, and each use's count: the two counters, the
  // counter in the page's drawer, and the panel's drawer and its counter.
  expect(manifest.signals).toEqual({
    drawer: false,
    "count~1": 0,
    "count~2": 0,
    "count~3": 0,
    "drawer~4": false,
    "count~5": 0,
  });
  // As the server wrote it, with no script run: both drawers shut, and the
  // two counters outside them at 0.
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(PAGE);
  await expect(page.locator(".drawer")).toHaveCount(0);
  await expect(page.locator(".count")).toHaveText(["0", "0"]);
  await context.close();
});

test("two uses of a view count apart", async ({ page }) => {
  await ready(page);
  await page.locator("#apples .add").click();
  await page.locator("#apples .add").click();
  await expect(page.locator("#apples .count")).toHaveText("2");
  await expect(page.locator("#pears .count")).toHaveText("0");
  await page.locator("#pears .add").click();
  await expect(page.locator("#pears .count")).toHaveText("1");
  await expect(page.locator("#apples .count")).toHaveText("2");
});

test("one compiled handler changes each use's own signal", async ({ page }) => {
  await ready(page);
  const apples = page.locator("#apples .add");
  const pears = page.locator("#pears .add");
  // The element says which instance its handler's `count` is.
  const a = JSON.parse(await apples.getAttribute("data-pw-signals"));
  const p = JSON.parse(await pears.getAttribute("data-pw-signals"));
  expect(Object.keys(a)).toEqual(["count"]);
  expect(Object.keys(p)).toEqual(["count"]);
  expect(a.count).not.toBe(p.count);
  // And the handler is one: the same identity, so one module.
  const manifest = manifestOf(await page.content());
  const owner = async (el) => Number(await el.getAttribute("data-pw"));
  const handlerOf = async (el) => {
    const o = await owner(el);
    return manifest.parts.find((x) => x.kind === "event" && x.owner === o)?.handler;
  };
  expect(await handlerOf(apples)).toBe(await handlerOf(pears));
  await apples.click();
  await expect
    .poll(() => page.evaluate(() => window.__pw.signals))
    .toMatchObject({ [a.count]: 1, [p.count]: 0 });
});

test("a button in one view opens a drawer another view shows", async ({ page }) => {
  await ready(page);
  const drawer = page.locator("#page-drawer .drawer");
  await expect(drawer).toHaveCount(0);
  await page.locator("header .open-cart").click();
  await expect(drawer).toBeVisible();
  await expect(drawer).toContainText("Your cart is empty.");
  // The panel's drawer is the panel's, and stays shut.
  await expect(page.locator(".panel .drawer")).toHaveCount(0);
  await page.locator("#page-drawer .close-cart").click();
  await expect(drawer).toHaveCount(0);
});

test("a view that provides the signal again keeps what it contains apart", async ({ page }) => {
  await ready(page);
  await page.locator(".panel .open-cart").click();
  await expect(page.locator(".panel .drawer")).toBeVisible();
  await expect(page.locator("#page-drawer .drawer")).toHaveCount(0);
  await page.locator("header .open-cart").click();
  await expect(page.locator("#page-drawer .drawer")).toBeVisible();
  await page.locator(".panel .close-cart").click();
  await expect(page.locator(".panel .drawer")).toHaveCount(0);
  await expect(page.locator("#page-drawer .drawer")).toBeVisible();
});

test("a view's signal starts again when its block shows again", async ({ page }) => {
  await ready(page);
  await page.locator("header .open-cart").click();
  const add = page.locator("#page-drawer .add");
  await add.click();
  await add.click();
  await add.click();
  await expect(page.locator("#page-drawer .count")).toHaveText("3");
  await page.locator("#page-drawer .close-cart").click();
  await expect(page.locator("#page-drawer .drawer")).toHaveCount(0);
  await page.locator("header .open-cart").click();
  await expect(page.locator("#page-drawer .count")).toHaveText("0");
  // A counter outside any block keeps its count throughout.
  await page.locator("#apples .add").click();
  await page.locator("#page-drawer .close-cart").click();
  await page.locator("header .open-cart").click();
  await expect(page.locator("#apples .count")).toHaveText("1");
});
