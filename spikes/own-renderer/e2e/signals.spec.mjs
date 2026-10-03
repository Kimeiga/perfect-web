// ADR-0130, first slice: a page's own UI state, in the browser.
//
// `examples/demo/panel.pw` declares three signals and changes them in its
// handlers. What this shows:
// - the server renders each signal at its first value, and the page reads
//   with no script at all;
// - a press changes a signal in the browser, and the parts that read it are
//   rendered again there: a text part, and a `{#match}` block the browser's
//   build of the server's renderer renders;
// - a handler inside a block rendered again is bound, once;
// - nothing is asked of the server but code;
// - what the browser renders is escaped as the server escapes it.
import { expect, test } from "@playwright/test";

const PAGE = "/page/demo.panel.PanelPage";

async function ready(page) {
  await page.goto(PAGE);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

test("the server renders each signal at its first value", async ({ request }) => {
  const html = await (await request.get(PAGE)).text();
  // The markup, before the manifest: the manifest carries each block's
  // template, every arm of it, for the browser to render.
  const markup = html.split('<script type="application/json"')[0];
  expect(markup).toContain('<h1 id="greeting"><!--pw:s0-->Hello<!--pw:e0--></h1>');
  expect(markup).toContain("<!--pw:s1-->0<!--pw:e1-->");
  expect(markup).toContain('<p id="shut">Nothing is open.</p>');
  expect(markup).not.toContain("cart-panel");
});

test("the page reads with JavaScript disabled", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(PAGE);
  await expect(page.locator("#greeting")).toHaveText("Hello");
  await expect(page.locator("#presses")).toHaveText("0");
  await expect(page.locator("#shut")).toBeVisible();
  await context.close();
});

test("a press changes a signal, and its text part, in the browser", async ({ page }) => {
  const asked = [];
  page.on("request", (r) => {
    const path = new URL(r.url()).pathname;
    if (!/^\/(handler\/|pw-|page\/)/.test(path)) asked.push(path);
  });
  await ready(page);
  await page.locator("#press").click();
  await expect(page.locator("#presses")).toHaveText("1");
  await page.locator("#press").click();
  await expect(page.locator("#presses")).toHaveText("2");
  expect(await page.evaluate(() => window.__pw.signals.presses)).toBe(2);
  expect(asked, "nothing but code was asked of the server").toEqual([]);
});

test("a block a signal decides is rendered again, and its handler bound once", async ({
  page,
}) => {
  await ready(page);
  await page.locator("#open-cart").click();
  await expect(page.locator("#cart-panel")).toBeVisible();
  await expect(page.locator("#shut")).toHaveCount(0);

  await page.locator("#close").click();
  await expect(page.locator("#shut")).toBeVisible();
  await expect(page.locator("#cart-panel")).toHaveCount(0);

  // Opened twice: the Close button made the second time is bound, once.
  await page.locator("#open-cart").click();
  const close = await page.locator("#close").getAttribute("data-pw");
  await page.locator("#close").click();
  await expect(page.locator("#shut")).toBeVisible();
  const attached = await page.evaluate((owner) => {
    const part = window.__pw.parts.parts.find(
      (p) => p.kind === "event" && String(p.owner) === owner,
    );
    return window.__pw.log.filter((l) => l.startsWith(`attached ${part.id} `)).length;
  }, close);
  expect(attached, "bound each time it was made, and no more").toBe(2);
});

test("a block reading a second signal is rendered again when it changes", async ({ page }) => {
  await ready(page);
  await page.locator("#press").click();
  await page.locator("#open-help").click();
  await expect(page.locator("#help-panel")).toContainText("Pressed 1 time(s) so far.");
  await page.locator("#press").click();
  await expect(page.locator("#help-panel")).toContainText("Pressed 2 time(s) so far.");
  await expect(page.locator("#presses")).toHaveText("2");
});

test("what the browser renders is escaped as the server escapes it", async ({ page }) => {
  await ready(page);
  await page.locator("#shout").click();
  await expect(page.locator("#greeting")).toHaveText("<b>Hi</b> & bye");
  await expect(page.locator("#greeting b")).toHaveCount(0);

  // Inside a block, rendered by the browser's build of the server's renderer.
  await page.locator("#open-help").click();
  await expect(page.locator("#help-greeting")).toHaveText("<b>Hi</b> & bye");
  await expect(page.locator("#help-greeting b")).toHaveCount(0);
  const html = await page.locator("#help-greeting").innerHTML();
  expect(html).toContain("&lt;b&gt;Hi&lt;/b&gt; &amp; bye");
});

test("a page of signals alone listens for nothing", async ({ page }) => {
  const streams = [];
  page.on("request", (r) => {
    if (/\/(stream|poll)/.test(new URL(r.url()).pathname)) streams.push(r.url());
  });
  await ready(page);
  await page.locator("#press").click();
  await expect(page.locator("#presses")).toHaveText("1");
  expect(streams).toEqual([]);
});

test("presses run in the order they were made, whichever's code arrives first", async ({
  page,
}) => {
  // ADR-0152: each handler's code loads on its first press. The first press's
  // is slow to arrive here, so the second press's arrives first.
  let first = true;
  await page.route("**/handler/**", async (route) => {
    if (first) {
      first = false;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    await route.continue();
  });
  await ready(page);
  await page.locator("#open-cart").click();
  await page.locator("#open-help").click();
  // Help was pressed last, so Help is open, and stays open.
  await expect(page.locator("#help-panel")).toBeVisible();
  await page.waitForTimeout(700);
  await expect(page.locator("#help-panel")).toBeVisible();
  await expect(page.locator("#cart-panel")).toHaveCount(0);
});
