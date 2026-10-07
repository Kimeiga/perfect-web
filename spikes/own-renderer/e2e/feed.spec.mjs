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
//     one whose request fails is taken back (ADR-0222);
//   - a post's text is at most 280 code points: a longer draft is not sent
//     (ADR-0225);
//   - "Load more" shows the longer page, and a post after it is shown over
//     the longer page (ADR-0222, ADR-0224);
//   - a thread's counts are values its template computes, the host's, shown
//     with scripts off, and a like's reaches the open thread (ADR-0226);
//   - what is left of a draft, and whether there is a post to send, are
//     computed from it as it is typed, and the host's first (ADR-0227);
//   - a row's likes are counted in words, and a like shows before the
//     server answers, its count computed again in the browser (ADR-0228);
//   - a condition computed: the browser's for a draft too long, the host's
//     for a thread with no reply (ADR-0229);
//   - a reply reaches its thread and every reader of it, and no timeline:
//     the thread page's form passes the page's parameter (ADR-0231);
//   - a reply shows before the server answers, its count and the thread's
//     "No replies yet." with it, and is the server's after; one whose request
//     fails is taken back (ADR-0236, ruling 0122-d).
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

/** A post's like count, as its row says it in words (ADR-0228). */
async function likes(page, text) {
  const said = await post(page, text).locator(".likes").innerText();
  return Number(said.match(/^(\d+) like/)[1]);
}

/** A count in words, as the feed's `counted` writes it. */
const counted = (n, one, many) => (n === 1 ? `1 ${one}` : `${n} ${many}`);

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
  // The author's own row is "You" until the server's arrives (ADR-0222):
  // waited for, not read at once. In one run of ADR-0229's, Firefox read
  // the pending row.
  await expect(post(author, text).getByRole("link")).toHaveText(/^Guest /);
  const name = await post(author, text).getByRole("link").innerText();
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
  await post(liker, "Hello, feed.").getByRole("button", { name: "Like" }).click();
  const after = counted(before + 1, "like", "likes");
  await expect(post(liker, "Hello, feed.").locator(".likes")).toHaveText(after);
  await expect(post(reader, "Hello, feed.").locator(".likes")).toHaveText(after);
  expect(await unreloaded(reader)).toBe(true);
  await a.close();
  await b.close();
});

test("what is left of a draft is computed as it is typed", async ({ browser }) => {
  // ADR-0227: `{280 - String.length(draft)} left`, and the button disabled
  // while there is no post to send. With scripts off, the host's first
  // values, from the draft's first value.
  const off = await browser.newContext({ javaScriptEnabled: false });
  const still = await off.newPage();
  await still.goto("/");
  await expect(still.locator("#left")).toHaveText("280 left");
  await expect(still.getByRole("button", { name: "Post" })).toBeDisabled();
  await off.close();
  // In the browser, as the draft changes: the page's module, loaded with
  // the first change, computes each again.
  const context = await browser.newContext();
  const page = await context.newPage();
  await home(page);
  const draft = page.getByLabel("What's happening?");
  const button = page.getByRole("button", { name: "Post" });
  await expect(page.locator("#left")).toHaveText("280 left");
  await expect(button).toBeDisabled();
  await draft.fill("Hello");
  await expect(page.locator("#left")).toHaveText("275 left");
  await expect(button).toBeEnabled();
  await draft.fill("");
  await expect(page.locator("#left")).toHaveText("280 left");
  await expect(button).toBeDisabled();
  expect(await unreloaded(page)).toBe(true);
  await context.close();
});

test("a condition is computed: a draft too long, and a thread with no reply", async ({
  page,
}) => {
  // ADR-0229. The home page's `{#if String.length(draft) > 280}` is the
  // browser's, which renders the block again as the draft changes; a
  // thread's `{#if List.length(thread.replies) == 0}` is the host's.
  await home(page);
  const draft = page.getByLabel("What's happening?");
  await expect(page.locator("#over")).toHaveCount(0);
  await draft.fill("x".repeat(281));
  await expect(page.getByRole("alert")).toHaveText("Too long to post.");
  await draft.fill("x".repeat(280));
  await expect(page.locator("#over")).toHaveCount(0);
  expect(await unreloaded(page)).toBe(true);
  await page.goto("/post/p3");
  await expect(page.locator("#quiet")).toHaveText("No replies yet.");
  await page.goto("/post/p1");
  await expect(page.locator("#counts")).toContainText("1 reply");
  await expect(page.locator("#quiet")).toHaveCount(0);
});

test("a thread's counts are computed by the host, and a like's reaches it", async ({
  browser,
}) => {
  // With scripts off: the host computed them into the page.
  const off = await browser.newContext({ javaScriptEnabled: false });
  const still = await off.newPage();
  await still.goto("/post/p3");
  await expect(still.locator("#counts")).toHaveText("0 replies · 1 like");
  await off.close();
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [liker, reader] = [await a.newPage(), await b.newPage()];
  await home(liker);
  await reader.goto("/post/p1");
  await reader.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await reader.evaluate(() => {
    window.__unreloaded = true;
  });
  const before = await likes(liker, "Hello, feed.");
  const counts = reader.locator("#counts");
  await expect(counts).toHaveText(`1 reply · ${counted(before, "like", "likes")}`);
  // Another session's like: the thread is a new value, so a new count, sent
  // to the open page as its text.
  await post(liker, "Hello, feed.").getByRole("button", { name: "Like" }).click();
  await expect(counts).toHaveText(`1 reply · ${counted(before + 1, "like", "likes")}`);
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

test("a like shows before the server answers, and one that fails is taken back", async ({
  page,
}) => {
  // ADR-0228: the like's transition counts it in the timeline the page
  // holds, and each row's count in words is computed again from it, in
  // the browser, as the host computes it for the rows it renders.
  await home(page);
  const before = await likes(page, "Hello, feed.");
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.like", async (route) => {
    await held;
    await route.continue();
  });
  await post(page, "Hello, feed.").getByRole("button", { name: "Like" }).click();
  const count = post(page, "Hello, feed.").locator(".likes");
  await expect(count).toHaveText(counted(before + 1, "like", "likes"));
  // The server's, once it has answered: the same count, in one row.
  const answered = page.waitForResponse((r) => r.url().includes("/command/feed.app.like"));
  release();
  await answered;
  await expect(count).toHaveText(counted(before + 1, "like", "likes"));
  await expect(post(page, "Hello, feed.")).toHaveCount(1);
  await page.unroute("**/command/feed.app.like");
  // A like whose request fails is taken back.
  let fail;
  const failing = new Promise((r) => (fail = r));
  await page.route("**/command/feed.app.like", async (route) => {
    await failing;
    await route.abort("failed");
  });
  await post(page, "Hello, feed.").getByRole("button", { name: "Like" }).click();
  await expect(count).toHaveText(counted(before + 2, "like", "likes"));
  fail();
  await expect(count).toHaveText(counted(before + 1, "like", "likes"));
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

test("a post longer than 280 characters is not sent, and 280 emoji are", async ({ page }) => {
  await home(page);
  const sent = [];
  page.on("request", (r) => {
    if (r.url().includes("/command/feed.app.post")) sent.push(r.postData());
  });
  const draft = page.getByLabel("What's happening?");
  const button = page.getByRole("button", { name: "Post" });
  // The page computes what is left, and whether there is a post to send,
  // from the draft (ADR-0227): one character over, and nothing to press.
  await draft.fill("x".repeat(281));
  await expect(page.locator("#left")).toHaveText("-1 left");
  await expect(button).toBeDisabled();
  // A submit that comes another way meets the page's own check,
  // `post_text`, which finds no post's text: nothing is sent, and the draft
  // is kept to shorten.
  await page.locator("form").evaluate((form) => form.requestSubmit());
  await expect(draft).toHaveValue("x".repeat(281));
  // 280 emoji are 280 code points, as `String.length` counts them, though
  // 560 UTF-16 units: none left, sent, and posted. Its row says the press
  // before it was answered too.
  const emoji = "\u{1F600}".repeat(280);
  await draft.fill(emoji);
  await expect(page.locator("#left")).toHaveText("0 left");
  await button.click();
  await expect(post(page, emoji)).toHaveCount(1);
  await expect(post(page, emoji).getByRole("link")).toHaveText(/^Guest /);
  expect(sent).toHaveLength(1);
});

test("Load more shows the next page, and a post after it is shown over it", async ({
  browser,
  page,
}, testInfo) => {
  // Twenty-five posts by another session, through the command route as a
  // page sends them.
  const other = await browser.newContext();
  for (let i = 0; i < 25; i++) {
    const r = await other.request.post("/command/feed.app.post", {
      headers: { "content-type": "application/json", "pw-interaction": `seed-${Date.now()}-${i}` },
      data: [`Older ${testInfo.project.name} ${i}`],
    });
    expect(r.status()).toBe(202);
  }
  await other.close();
  await home(page);
  const rows = page.locator("main > ul > li");
  await expect(rows).toHaveCount(20);
  await page.getByRole("button", { name: "Load more" }).click();
  await expect.poll(() => rows.count()).toBeGreaterThan(20);
  const longer = await rows.count();
  expect(longer).toBeLessThanOrEqual(40);
  // A post held at the network is shown over the page the browser holds now,
  // the longer one: one row more, the post first.
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.post", async (route) => {
    await held;
    await route.continue();
  });
  const text = `After more in ${testInfo.project.name}, ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  await page.getByRole("button", { name: "Post" }).click();
  await expect(rows.first()).toContainText(text);
  await expect(rows).toHaveCount(longer + 1);
  release();
  await expect(post(page, text).getByRole("link")).toHaveText(/^Guest /);
  expect(await unreloaded(page)).toBe(true);
});

test("a reply reaches its thread and every reader of it, and no timeline", async ({
  browser,
}, testInfo) => {
  // ADR-0231. The thread page's form passes its `id`, the page's parameter,
  // which its handler captures. Until then a page that bound a query was
  // rendered without its parameters, and this one answered 503.
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [author, reader] = [await a.newPage(), await b.newPage()];
  await home(author);
  const text = `A thread in ${testInfo.project.name} at ${Date.now()}`;
  await author.getByLabel("What's happening?").fill(text);
  await author.getByRole("button", { name: "Post" }).click();
  // The server's row, whose link is the post's address, not the pending
  // row's.
  const link = post(author, text).getByRole("link");
  await expect(link).toHaveAttribute("href", /^\/post\/p\d+$/);
  const address = await link.getAttribute("href");
  for (const page of [author, reader]) {
    await page.goto(address);
    await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
    await page.evaluate(() => {
      window.__unreloaded = true;
    });
    await expect(page.locator("#quiet")).toHaveText("No replies yet.");
  }
  const reply = `A reply in ${testInfo.project.name} at ${Date.now()}`;
  const field = author.getByLabel("Your reply");
  await field.fill(reply);
  await author.getByRole("button", { name: "Reply" }).click();
  // In the thread, as its reply, to the author and to the other reader,
  // and counted.
  for (const page of [author, reader]) {
    await expect(page.locator("main article li").filter({ hasText: reply })).toHaveCount(1);
    await expect(page.locator("#counts")).toHaveText("1 reply · 0 likes");
    await expect(page.locator("#quiet")).toHaveCount(0);
    expect(await unreloaded(page)).toBe(true);
  }
  await expect(field).toHaveValue("");
  // And in no timeline: the post is there, its reply is not.
  await home(reader);
  await expect(post(reader, text)).toHaveCount(1);
  await expect(post(reader, reply)).toHaveCount(0);
  await a.close();
  await b.close();
});

/** A thread of its own, posted from the home page, open at its address. */
async function newThread(page, testInfo, what) {
  await home(page);
  const text = `${what} in ${testInfo.project.name} at ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  await page.getByRole("button", { name: "Post" }).click();
  const link = post(page, text).getByRole("link");
  await expect(link).toHaveAttribute("href", /^\/post\/p\d+$/);
  await page.goto(await link.getAttribute("href"));
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await page.evaluate(() => {
    window.__unreloaded = true;
  });
  await expect(page.locator("#quiet")).toHaveText("No replies yet.");
}

test("a reply shows before the server answers, and is the server's after", async ({
  page,
}, testInfo) => {
  // ADR-0236: the thread page binds `Thread(id)`, and its form passes `id`
  // to `reply`'s `to`, unchanged: the clause's `Thread(to)` is the thread the
  // page shows. Held at the network, the reply is the page's own meanwhile.
  await newThread(page, testInfo, "A held thread");
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.reply", async (route) => {
    await held;
    await route.continue();
  });
  const reply = `A held reply in ${testInfo.project.name} at ${Date.now()}`;
  await page.getByLabel("Your reply").fill(reply);
  await page.getByRole("button", { name: "Reply" }).click();
  const row = page.locator("main article li").filter({ hasText: reply });
  await expect(row).toHaveCount(1);
  await expect(row.getByRole("link")).toHaveText("You");
  await expect(page.locator("#counts")).toHaveText("1 reply · 0 likes");
  await expect(page.locator("#quiet")).toHaveCount(0);
  release();
  // The server's, by the session's guest.
  await expect(row.getByRole("link")).toHaveText(/^Guest /);
  await expect(page.locator("#counts")).toHaveText("1 reply · 0 likes");
  expect(await unreloaded(page)).toBe(true);
});

test("a reply whose request fails is taken back", async ({ page }, testInfo) => {
  await newThread(page, testInfo, "A failing thread");
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/command/feed.app.reply", async (route) => {
    await held;
    await route.abort("failed");
  });
  const reply = `A failed reply in ${testInfo.project.name} at ${Date.now()}`;
  const field = page.getByLabel("Your reply");
  await field.fill(reply);
  await page.getByRole("button", { name: "Reply" }).click();
  const row = page.locator("main article li").filter({ hasText: reply });
  await expect(row).toHaveCount(1);
  release();
  // Taken back: the thread as the server has it, and the draft kept.
  await expect(row).toHaveCount(0);
  await expect(page.locator("#counts")).toHaveText("0 replies · 0 likes");
  await expect(page.locator("#quiet")).toHaveText("No replies yet.");
  await expect(field).toHaveValue(reply);
  expect(await unreloaded(page)).toBe(true);
});
