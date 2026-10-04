// ADR-0187: nothing the store contains is on screen when its page is first
// laid out.
//
// Until 2026-10-04 every menu item was contained (`content-visibility: auto`)
// with a 42 px placeholder. The browser laid each out at 42 px, then rendered
// the ones on screen at their height, and what followed them moved: the cart,
// 249 px on a phone. That happened before the first paint, so no one saw it,
// and the Layout Instability API reported it all the same (Lighthouse:
// CLS 0.136). An item is contained now only far enough down its page that
// nothing on screen follows it.
import { expect, test } from "@playwright/test";

async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

/** Two frames, so a shift the page would make in its first is reported. */
const frames = (page) =>
  page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );

const PHONE = { width: 412, height: 823 };
const DESKTOP = { width: 1350, height: 940 };
// The first screen the thresholds are drawn for (ADR-0187): 2,400 CSS pixels.
const TALL = { width: 1920, height: 2400 };

for (const [device, viewport] of [
  ["a phone", PHONE],
  ["a desktop", DESKTOP],
]) {
  test(`the store's page does not shift as it loads, on ${device}`, async ({
    page,
    browserName,
  }) => {
    test.skip(browserName !== "chromium", "the Layout Instability API is Chromium's");
    await page.setViewportSize(viewport);
    await page.addInitScript(() => {
      window.__shifts = [];
      new PerformanceObserver((list) => {
        for (const e of list.getEntries()) window.__shifts.push(e.value);
      }).observe({ type: "layout-shift", buffered: true });
    });
    await ready(page, "/stores/47");
    await frames(page);
    expect(await page.evaluate(() => window.__shifts)).toEqual([]);
  });
}

/**
 * A page of the store's own style and menu item, with `lists[c]` items in
 * category `c`: the shapes a store's menu may have, from what the store
 * renders.
 */
async function menuOf(page, lists) {
  await ready(page, "/stores/47");
  const { style, item } = await page.evaluate(() => ({
    style: document.head.querySelector("style")?.outerHTML ?? "",
    item: document.querySelector("#menu li").outerHTML,
  }));
  const menu = lists
    .map((n, c) => `<h2>Category ${c}</h2><ul>${item.repeat(n)}</ul>`)
    .join("");
  const html =
    `<!doctype html><html lang="en"><head><meta charset="utf-8">` +
    `<meta name="viewport" content="width=device-width, initial-scale=1">` +
    `<title>Menu</title>${style}</head><body><main><h1>Store</h1>` +
    `<section aria-label="Menu"><div id="menu">${menu}</div></section>` +
    `<section aria-label="Cart"><h2>Cart</h2></section></main></body></html>`;
  await page.unroute("**/a-menu-of-many");
  await page.route("**/a-menu-of-many", (route) =>
    route.fulfill({ contentType: "text/html", body: html }),
  );
  await page.goto("/a-menu-of-many");
  await frames(page);
}

/** The menu's items: how many are contained, and how many of those are on screen. */
const containment = (page) =>
  page.evaluate(() => {
    const contained = [...document.querySelectorAll("#menu li")].filter(
      (li) => getComputedStyle(li).contentVisibility === "auto",
    );
    return {
      contained: contained.length,
      onScreen: contained.filter((li) => li.getBoundingClientRect().top < innerHeight).length,
    };
  });

for (const [shape, lists, contained] of [
  ["a long category", [1000], 972],
  ["many short categories", Array(40).fill(3), 72],
  ["many one-item categories, then a long one", [...Array(16).fill(1), 50], 50],
]) {
  test(`no item contained is on screen when the page is first laid out: ${shape}`, async ({
    page,
  }) => {
    for (const viewport of [PHONE, DESKTOP, TALL]) {
      await page.setViewportSize(viewport);
      await menuOf(page, lists);
      expect(await containment(page), `${viewport.width}x${viewport.height}`).toEqual({
        contained,
        onScreen: 0,
      });
    }
  });
}

test("the store's own menu is not contained, as it is short", async ({ page }) => {
  for (const viewport of [PHONE, DESKTOP]) {
    await page.setViewportSize(viewport);
    await ready(page, "/stores/47");
    expect(await containment(page)).toEqual({ contained: 0, onScreen: 0 });
  }
});

test("a contained item's placeholder is an item's height", async ({ page }) => {
  // So the scrollbar is as long as the page is: until 2026-10-04 a
  // thousand-item page grew by 64% as it was scrolled through.
  await page.setViewportSize(PHONE);
  await menuOf(page, [1000]);
  const { first, rendered } = await page.evaluate(() => {
    const first = document.documentElement.scrollHeight;
    for (const li of document.querySelectorAll("#menu li")) li.style.contentVisibility = "visible";
    return { first, rendered: document.documentElement.scrollHeight };
  });
  expect(Math.abs(first - rendered) / rendered, `${first} px, against ${rendered}`).toBeLessThan(
    0.1,
  );
});

test("the store's style is in its head, where it is read before the body", async ({ page }) => {
  // HTML puts `<style>` where metadata content is, and only one in the head
  // can hold back rendering until it is read.
  await ready(page, "/stores/47");
  expect(await page.locator("body style").count()).toBe(0);
  expect(await page.locator("head style").textContent()).toContain("content-visibility");
});
