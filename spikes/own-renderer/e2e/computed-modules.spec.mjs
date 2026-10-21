// ADR-0382: the modules a page loads when it first needs them are loaded
// within the deadline, and a load that failed is not kept. A feed page
// computes what is left of a draft in the browser, by a module it loads with
// the draft's first change (ADR-0227). Until 2026-10-21 one failed load of it
// was kept, and nothing was computed again until the page was reloaded; and a
// load that never answered was waited for without end. The press's modules
// are `press-modules.spec.mjs`'s.
//
// Each fault is the page's own, so the suite shares its host.
import { test } from "@playwright/test";
import { expect } from "../assertion-expect.mjs";
import { FEED_PORTS } from "../host-metadata.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${FEED_PORTS[testInfo.project.name]}`);
  },
});

/** The deadline these tests give a load, in milliseconds (ADR-0381's). */
const DEADLINE = 1500;

/** A page, ready to be typed in. Polled: WebKit runs no animation frame
 * before its first paint (`slots.spec.mjs`). */
async function ready(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

const logged = (page, prefix) =>
  page.evaluate((p) => (window.__pw?.log ?? []).filter((l) => l.startsWith(p)).length, prefix);

/** No answer, and no failure either: the request is held open. */
const neverAnswered = () => new Promise(() => {});

/** The feed's home page, ready, and what is left of its draft. */
async function drafting(page, path) {
  await ready(page, path);
  const left = page.locator("#left");
  await expect(left).toHaveText("280 left");
  return { draft: page.getByLabel("What's happening?"), left };
}

test("a computed values module that failed to load is loaded again at the next change", async ({
  page,
}) => {
  let calls = 0;
  await page.route("**/computed/**", (route) => (++calls === 1 ? route.abort("failed") : route.continue()));
  try {
    const { draft, left } = await drafting(page, "/");
    await draft.fill("Hello");
    await expect.poll(() => logged(page, "computed values not loaded"), { timeout: 8_000 }).toBe(1);
    // The next change asks the network again, and is computed.
    await draft.fill("Hello!");
    await expect(left).toHaveText("274 left");
    expect(calls).toBe(2);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});

test("a computed values module that never loads is given up at the deadline, and loaded again", async ({
  page,
}) => {
  let calls = 0;
  await page.route("**/computed/**", (route) => (++calls === 1 ? neverAnswered() : route.continue()));
  try {
    const { draft, left } = await drafting(page, `/?deadline=${DEADLINE}`);
    await draft.fill("Hello");
    await expect.poll(() => logged(page, "computed values not loaded"), { timeout: 8_000 }).toBe(1);
    await draft.fill("Hello!");
    await expect(left).toHaveText("274 left");
    expect(calls).toBe(2);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  }
});
