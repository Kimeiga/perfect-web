// ADR-0220: the feed reference app (ADR-0218) in three engines. A second
// program served by the same host as the store, with its own data layer,
// built by `feed.sh` and served on a host per engine: a post is every
// reader's, so one engine's posts are never another's, and the tests of a
// host run one at a time.
//   - the home page shows the timeline, a thread page a post's replies as
//     deep as they go, and a post that is not there is not found;
//   - a post reaches the author's timeline, and every other open reader's
//     without a reload (ADR-0219), and the draft is cleared;
//   - a like's count reaches every reader;
//   - a post shows before the server answers, and is the server's after;
//     one whose request fails is taken back (ADR-0222).
import { expect, test } from "@playwright/test";
import { FEED_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${FEED_PORTS[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

/** The home page, its handlers attached. */
async function home(page) {
  await page.goto("/");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  // A mark a reload would lose: what reaches the page is sent to it, never
  // read by reading the page again.
  await page.evaluate(() => {
    window.__unreloaded = true;
  });
}

const unreloaded = (page) => page.evaluate(() => window.__unreloaded === true);

/** The timeline's post with `text`. */
const post = (page, text) => page.locator("main > ul > li").filter({ hasText: text });

/** A post's like count, as its button shows it. */
async function likes(page, text) {
  const label = await post(page, text).getByRole("button", { name: /^Like/ }).innerText();
  return Number(label.match(/\((\d+)\)/)[1]);
}

test("the home page shows the timeline, and a thread its replies", async ({ page }) => {
  await home(page);
  await expect(post(page, "Hello, feed.")).toHaveCount(1);
  // As deep as they go (ADR-0203): a reply, and the reply to it.
  await page.goto("/post/p1");
  const thread = page.locator("main");
  await expect(thread).toContainText("Hello, feed.");
  await expect(thread).toContainText("Hello, Ada.");
  await expect(thread).toContainText("And a reply to the reply.");
  const missing = await page.goto("/post/none");
  expect(missing.status()).toBe(404);
});

test("a post reaches the author's timeline and every other reader's", async ({
  browser,
}, testInfo) => {
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [author, reader] = [await a.newPage(), await b.newPage()];
  for (const page of [author, reader]) await home(page);
  const text = `Posted in ${testInfo.project.name} at ${Date.now()}`;
  const draft = author.getByLabel("What's happening?");
  await draft.fill(text);
  await author.getByRole("button", { name: "Post" }).click();
  await expect(post(author, text)).toHaveCount(1);
  await expect(draft).toHaveValue("");
  // Another reader's open timeline, told by the host (ADR-0219), and the
  // author named the same to it as to the author (ADR-0220).
  await expect(post(reader, text)).toHaveCount(1);
  const name = await post(author, text).getByRole("link").innerText();
  expect(name).toMatch(/^Guest /);
  await expect(post(reader, text).getByRole("link")).toHaveText(name);
  expect(await unreloaded(author)).toBe(true);
  expect(await unreloaded(reader)).toBe(true);
  await a.close();
  await b.close();
});

test("a like's count reaches every reader", async ({ browser }) => {
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [liker, reader] = [await a.newPage(), await b.newPage()];
  for (const page of [liker, reader]) await home(page);
  const before = await likes(reader, "Hello, feed.");
  await post(liker, "Hello, feed.").getByRole("button", { name: /^Like/ }).click();
  await expect(post(liker, "Hello, feed.").getByRole("button")).toHaveText(`Like (${before + 1})`);
  await expect(post(reader, "Hello, feed.").getByRole("button")).toHaveText(`Like (${before + 1})`);
  expect(await unreloaded(reader)).toBe(true);
  await a.close();
  await b.close();
});

test("a post shows before the server answers, and is the server's after", async ({
  page,
}, testInfo) => {
  await home(page);
  // The request held at the network: what shows meanwhile is the page's own.
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.post", async (route) => {
    await held;
    await route.continue();
  });
  const text = `Shown at once in ${testInfo.project.name}, ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  await page.getByRole("button", { name: "Post" }).click();
  await expect(post(page, text)).toHaveCount(1);
  await expect(post(page, text).getByRole("link")).toHaveText("You");
  await expect(page.locator("main > ul > li").first()).toContainText(text);
  release();
  // The server's: its author named as the server names it, and one row.
  await expect(post(page, text).getByRole("link")).toHaveText(/^Guest /);
  await expect(post(page, text)).toHaveCount(1);
  await expect(page.getByLabel("What's happening?")).toHaveValue("");
  expect(await unreloaded(page)).toBe(true);
});

test("a post whose request fails is taken back", async ({ page }, testInfo) => {
  await home(page);
  const before = await page.locator("main > ul > li").count();
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.post", async (route) => {
    await held;
    await route.abort("failed");
  });
  const text = `Never sent from ${testInfo.project.name}, ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  await page.getByRole("button", { name: "Post" }).click();
  await expect(post(page, text)).toHaveCount(1);
  release();
  await expect(post(page, text)).toHaveCount(0);
  await expect(page.locator("main > ul > li")).toHaveCount(before);
  // The draft is kept, to send again.
  await expect(page.getByLabel("What's happening?")).toHaveValue(text);
});
