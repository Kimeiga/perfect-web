// E10 gate item 3 — a subscriber the server can no longer serve is told to
// reload, and the page does.
//
// The server bounds what it holds for a subscriber. One that falls more than
// MAX_WAITING frames behind, or is forgotten after IDLE without asking, cannot
// be told what it missed, and gets `Recovery::Reload` instead: the protocol's
// "this document cannot continue". Until 2026-09-25 the runtime only logged a
// recovery, so such a page would have stayed wrong without saying so.

import { test, expect } from "@playwright/test";

async function ready(page) {
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
}

test("a reload recovery reloads the document", async ({ page }) => {
  await ready(page);
  await page.evaluate(() => {
    window.__beforeRecovery = true;
  });
  const reloaded = page.waitForEvent("load");
  await page.evaluate(() =>
    window.__pwTestApply({ frame: "recovery", protocol: 1, recovery: { recovery: "reload" } }),
  );
  await reloaded;
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  expect(await page.evaluate(() => window.__beforeRecovery ?? null), "a new document").toBeNull();
  expect(
    await page.evaluate(() => performance.getEntriesByType("navigation")[0]?.type),
  ).toBe("reload");
});

test("any other recovery is recorded, not acted on", async ({ page }) => {
  // Only a reload is the page's to perform; the rest need the user or a
  // region the server names. None of them is "try anyway".
  await ready(page);
  await page.evaluate(() => {
    window.__beforeRecovery = true;
    window.__pwTestApply({
      frame: "recovery",
      protocol: 1,
      recovery: { recovery: "retry-interaction" },
    });
  });
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => window.__beforeRecovery)).toBe(true);
  expect(await page.evaluate(() => window.__pw.log.join("\n"))).toMatch(
    /recovery: .*retry-interaction/,
  );
});

test("a page the server has forgotten is told to reload", async ({ browser }) => {
  // A fresh context: a session the server has never seen, polling with a
  // cursor only a served page could have.
  const context = await browser.newContext();
  const response = await context.request.get("/stream?since=99");
  expect(await response.json()).toEqual({
    cursor: 99,
    frames: [{ frame: "recovery", protocol: 1, recovery: { recovery: "reload" } }],
  });

  // The control: cursor zero is a subscriber that has not been served a
  // document yet, and it is subscribed, not told to reload.
  const fresh = await context.request.get("/stream?since=0");
  expect((await fresh.json()).frames).toEqual([]);
  await context.close();
});
