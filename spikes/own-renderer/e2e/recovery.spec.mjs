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

test("a press on a handler from another build reads the page again, once (charter §15.6 test 16)", async ({
  page,
}) => {
  // The document as a cache kept it from an earlier build: its Add names a
  // handler this build does not have, and the resume decision refuses it.
  // Until 2026-10-03 the press did nothing and said nothing.
  let stale = true;
  await page.route("**/StorePage.html", async (route) => {
    const response = await route.fetch();
    let body = await response.text();
    if (stale) {
      stale = false;
      const manifest = JSON.parse(body.match(/id="pw-parts">(.*?)<\/script>/s)[1]);
      const add = manifest.parts.find((p) => p.kind === "event" && p.name === "add_to_cart").value;
      body = body.replaceAll(add, "ffffffffffffffff");
    }
    await route.fulfill({ response, body });
  });
  await ready(page);
  await expect(page.locator("#cart-count")).toHaveText("0");
  // The decision names its recovery in `pw-resume`'s order (ADR-0155): the
  // store's page is a session's (its plan's scope), so its region is a
  // private slot, rendered again from the server. Until the build named a
  // page's scope every page was planned public, a region to refetch, and a
  // list read one place off made that "none", which no press acted on; a
  // session's page made it "refetch-region", which reloads as well, and only
  // this name tells the two apart.
  expect(await page.evaluate(() => window.__pw.log.join("\n"))).toMatch(
    /refused \S+: code \d+ recovery rerender-private-slot/,
  );
  // The press reads the page again, from this build, and is not replayed.
  const reloaded = page.waitForEvent("load");
  await page.locator("#menu button").first().click();
  await reloaded;
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  await expect(page.locator("#cart-count")).toHaveText("0");
  // The page from this build presses as any does.
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("a document of another schema reads the page again, once (ADR-0300)", async ({ page }) => {
  // The decision holds a handler's document to what this build says of its
  // page, from its handler table: its document schema. A document that says
  // another, its parts' places another build's, is refused and read again.
  // Until ADR-0300 every manifest and the decision said `cart-doc`, so this
  // document attached.
  let stale = true;
  await page.route("**/StorePage.html", async (route) => {
    const response = await route.fetch();
    let body = await response.text();
    if (stale) {
      stale = false;
      body = body.replace(/"document":"[^"]*"/, '"document":"another-schema"');
    }
    await route.fulfill({ response, body });
  });
  await ready(page);
  // Told by the table, never by the document; and its recovery named in
  // `pw-resume`'s order, a session's region rendered again (ADR-0155).
  const told = await page.evaluate(() => window.__pw.log.join("\n"));
  // A user's page since track `store-accounts`: its documents are its user's.
  expect(told).toMatch(/knows store\.page\.StorePage's documents: \S+ user:/);
  expect(told).toMatch(/refused \S+: code \d+ recovery rerender-private-slot/);
  await expect(page.locator("#cart-count")).toHaveText("0");
  const reloaded = page.waitForEvent("load");
  await page.locator("#menu button").first().click();
  await reloaded;
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  // Not replayed: the press is the reader's to make again.
  await expect(page.locator("#cart-count")).toHaveText("0");
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");
});

test("a page still stale after being read again is not read again", async ({ page }) => {
  // A server that keeps sending the stale document must not reload the page
  // for ever: the second press finds the first reload, and the button stays
  // inert, saying why.
  await page.route("**/StorePage.html", async (route) => {
    const response = await route.fetch();
    const body = await response.text();
    const manifest = JSON.parse(body.match(/id="pw-parts">(.*?)<\/script>/s)[1]);
    const add = manifest.parts.find((p) => p.kind === "event" && p.name === "add_to_cart").value;
    await route.fulfill({ response, body: body.replaceAll(add, "ffffffffffffffff") });
  });
  await ready(page);
  const reloaded = page.waitForEvent("load");
  await page.locator("#menu button").first().click();
  await reloaded;
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  await page.locator("#menu button").first().click();
  await page.waitForTimeout(500);
  await expect(page.locator("#menu button").first()).toHaveAttribute(
    "data-pw-handler-error",
    "reload-loop",
  );
  await expect(page.locator("#cart-count")).toHaveText("0");
});
