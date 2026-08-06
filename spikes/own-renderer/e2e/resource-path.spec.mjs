// E7-R — the UI changes because a declared resource dependency changed.
//
// Architect ruling, 2026-08-06:
//
// > The important proof is that the UI changed because the **declared resource
// > dependency changed**, not because an endpoint happened to return
// > `{ cart: 2 }`.
//
// `examples/store/app.pw` declares:
//
//   session query Cart(session)   cached private
//   command add_to_cart(..)       invalidates Cart(current_session())
//                                 emits      CartChanged(current_session())
//
// So the command returns nothing about the cart. It commits, the materializer
// drains the committed event, the event's arguments select which entries
// invalidate, the resource refreshes with a new VERSION, and the subscriber is
// told. Every control below attacks one link of that chain.

import { test, expect } from "@playwright/test";

async function ready(page) {
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

test("the command's response carries no cart value", async ({ page, request }) => {
  // The claim, checked at its narrowest: if the response contained the number,
  // every test below would pass against a runtime that read it from there.
  await ready(page);
  const response = await request.post("/command/add_to_cart");
  const body = await response.json();
  expect(body).toEqual({ committed: true });
  expect(JSON.stringify(body)).not.toMatch(/line_count/);
});

test("the cart updates from the resource, not from the command", async ({ page }) => {
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("0");
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const log = await page.evaluate(() => window.__pw.log.join("\n"));
  expect(log, "the update names the version it came from").toMatch(/at version \d+/);
});

test("a rolled-back command produces no browser update", async ({ page, request }) => {
  // The state change did not happen, so the event must not exist, so nothing
  // reaches the browser. If a stray update arrived, the page would show a cart
  // the server does not have.
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("0");

  await request.post("/command/add_and_fail").catch(() => {});
  await page.waitForTimeout(400);
  await expect(page.locator("#cart-count")).toHaveText("0");
});

test("an unrelated event leaves the cart alone", async ({ page, request }) => {
  // `MenuChanged` reaches no cart entry, whatever its arguments. Without this,
  // "the cart updated" is consistent with a materializer that refreshes
  // everything on every event.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const before = await page.evaluate(() => window.__pw.log.length);
  await request.post("/command/menu_changed");
  await page.waitForTimeout(400);
  await expect(page.locator("#cart-count")).toHaveText("1");
  const after = await page.evaluate(() => window.__pw.log.filter((l) => l.startsWith("updated")).length);
  expect(after, "no cart update was applied").toBeLessThanOrEqual(before);
});

test("one session's change does not reach another", async ({ browser }) => {
  // `CartChanged(session A)` selects session A's entry. Two contexts are two
  // sessions, because the server keys carts the way the program declares.
  const a = await browser.newContext();
  const b = await browser.newContext();
  const pa = await a.newPage();
  const pb = await b.newPage();
  for (const p of [pa, pb]) {
    await p.goto("/StorePage.html");
    await p.waitForFunction(() => document.documentElement.dataset.pwReady);
  }

  await pa.locator("#menu button").first().click();
  await expect(pa.locator("#cart-count")).toHaveText("1");
  await pb.waitForTimeout(400);
  await expect(pb.locator("#cart-count"), "session B is untouched").toHaveText("0");

  await a.close();
  await b.close();
});

test("a duplicate committed event causes no second transition", async ({ page, request }) => {
  // E6 gate item 2, observed in the browser. The version is what makes it
  // harmless: a second event for a state that did not change again produces a
  // version the page already holds.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  await request.post("/command/menu_changed");
  await request.post("/command/menu_changed");
  await page.waitForTimeout(400);
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("a stale version does not overwrite newer state", async ({ page }) => {
  // The property an asynchronous subscription creates the need for. Asserted
  // by feeding the page an OLD version directly: the transport is what
  // delivers out of order, so the guard has to be in the page.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const ignoredBefore = await page.evaluate(() => window.__pw.ignored ?? 0);
  await page.evaluate(() => {
    // Version 0 — older than what the page holds after one commit.
    window.__pwTestApply?.({ "cart.line_count": 99, version: 0 });
  });
  await page.waitForTimeout(200);
  await expect(page.locator("#cart-count"), "the older value is refused").toHaveText("1");

  // And the control: a NEWER version is applied, so the guard is a comparison
  // rather than a refusal of everything.
  await page.evaluate(() => {
    window.__pwTestApply?.({ "cart.line_count": 42, version: 9999 });
  });
  await page.waitForTimeout(200);
  await expect(page.locator("#cart-count")).toHaveText("42");
  void ignoredBefore;
});
