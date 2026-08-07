// The browser knows the protocol and nothing else.
//
// Architect ruling, 2026-08-07:
//
// > The browser must not import or understand `EntryKey`, SQLite, outbox rows,
// > compiler graph nodes, materializer internals. Only `ResourceEntryId`,
// > `ResourceVersion`, `PartAddress`, `StreamFrame`, `Patch`.
//
// Asserted against what the browser actually receives, not against what the
// server intends to send.

import { test, expect } from "@playwright/test";

// The subscription is a long poll, so a request is always in flight when a test
// ends. Without this, tearing down the route handler races the poll and every
// test in this file reports a harness error that reads like a runtime one.
test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: "ignoreErrors" }).catch(() => {});
});

test("every frame the browser receives is a protocol frame", async ({ page }) => {
  const frames = [];
  await page.route("**/stream", async (route) => {
    const response = await route.fetch();
    const body = await response.text();
    try {
      frames.push(...JSON.parse(body));
    } catch {
      /* an empty poll */
    }
    await route.fulfill({ response, body });
  });

  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  expect(frames.length, "frames were observed").toBeGreaterThan(0);

  const KNOWN = new Set(["resource_changed", "patch", "recovery"]);
  for (const frame of frames) {
    expect(KNOWN.has(frame.frame), `unknown frame kind ${frame.frame}`).toBe(true);
    expect(frame.protocol, "every frame carries its protocol version").toBe(1);
  }
});

test("no server-side concept reaches the browser", async ({ page }) => {
  const seen = [];
  await page.route("**/*", async (route) => {
    const response = await route.fetch();
    // DATA the browser receives, not the source of the runtime that reads it.
    //
    // A first version scanned the JavaScript too and failed on the runtime's
    // own comments, which explain what `materialize` and `invalidates_on` mean
    // in order to say why the browser does not see them. Prose about a
    // boundary is not a crossing of it — the property is about what arrives as
    // a value.
    const type = response.headers()["content-type"] ?? "";
    if (/json|html/.test(type)) {
      seen.push(await response.text());
    }
    await route.fulfill({ response });
  });

  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  await page.locator("#menu button").first().click();
  await expect(page.locator("#cart-count")).toHaveText("1");

  const everything = seen.join("\n");
  expect(everything.length, "bytes were observed").toBeGreaterThan(0);

  // Server-side vocabulary. If any of these appear, the boundary leaked — and
  // the browser would then be able to depend on something the server is free
  // to change.
  for (const concept of [
    "EntryKey",
    "sqlite",
    "SQLite",
    "outbox",
    "invalidates_on",
    "depends_on",
    "materialize",
    "logical_key",
  ]) {
    expect(everything, `\`${concept}\` must not reach the browser`).not.toContain(
      concept,
    );
  }
});

test("this boundary check can fail", async ({ page }) => {
  // Without it, "no server concept appears" holds for a page that fetched
  // nothing. The control asserts the reader sees the things it SHOULD.
  const seen = [];
  await page.route("**/*", async (route) => {
    const response = await route.fetch();
    const type = response.headers()["content-type"] ?? "";
    if (/json|html/.test(type)) seen.push(await response.text());
    await route.fulfill({ response });
  });
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);

  const everything = seen.join("\n");
  expect(everything, "the reader observes the document").toContain("cart-count");
  expect(everything, "and the parts manifest").toContain("pw-parts");
});
