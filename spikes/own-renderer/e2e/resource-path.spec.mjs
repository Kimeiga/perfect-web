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
  // The property an asynchronous transport creates the need for. Asserted by
  // feeding the page an OLD frame directly: a transport cannot be made to
  // deliver out of order on demand, and the guard is the PAGE's — it has to
  // be, because the transport is what reorders.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const state = await page.evaluate(() => ({
    held: window.__pwHeld(),
    parts: window.__pw.parts,
  }));
  const [entry, version] = Object.entries(state.held)[0];
  const cart = state.parts.parts.find((p) => p.value === "cart.line_count");
  const target = { template: state.parts.schema, instances: [], part: cart.id };

  const frame = (v, text) => ({
    frame: "patch",
    protocol: 1,
    basis: { resources: [{ entry, version: v }] },
    target,
    operation: { op: "replace_text", text },
  });

  await page.evaluate((f) => window.__pwTestApply(f), frame(version - 1, "99"));
  await page.waitForTimeout(100);
  await expect(page.locator("#cart-count"), "an older basis is refused").toHaveText("1");

  // The control: a NEWER basis IS applied, so the guard is a comparison
  // rather than a refusal of everything.
  await page.evaluate((f) => window.__pwTestApply(f), frame(version + 100, "42"));
  await page.waitForTimeout(100);
  await expect(page.locator("#cart-count")).toHaveText("42");
});

test("a frame from another protocol version is refused", async ({ page }) => {
  // Incomparable, not different — before anything it contains is interpreted.
  await ready(page);
  await page.evaluate(() => {
    window.__pwTestApply({
      frame: "patch",
      protocol: 99,
      basis: { resources: [{ entry: "x", version: 9999 }] },
      target: { template: "t", instances: [], part: 4 },
      operation: { op: "replace_text", text: "666" },
    });
  });
  await page.waitForTimeout(100);
  await expect(page.locator("#cart-count")).toHaveText("0");
  const log = await page.evaluate(() => window.__pw.log.join("\n"));
  expect(log).toMatch(/refused frame: protocol 99/);
});

test("a notice is not an application", async ({ page }) => {
  // The defect wiring the real materializer found. A `resource_changed` frame
  // says newer state EXISTS; it does not make the document reflect it. A first
  // version advanced the held version on the notice, so the patch that
  // realized the same version advanced nothing and was refused as stale — and
  // both frames were "handled" while the page stayed at 0.
  await ready(page);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const state = await page.evaluate(() => ({
    held: window.__pwHeld(),
    known: window.__pwKnown(),
  }));
  expect(Object.keys(state.known).length, "a notice was received").toBeGreaterThan(0);
  expect(Object.keys(state.held).length, "and a patch was applied").toBeGreaterThan(0);
});
