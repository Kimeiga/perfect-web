// Track `notifications` (ADR-XXXX): what others do that involves you, in
// three engines. The feed is served on hosts of its own
// (NOTIFICATIONS_PORTS), one per engine, its sessions signed in through the
// development identity provider (`PW_IDENTITY=dev-accounts`), so one user
// reads in two browsers and another user beside them.
//   - a like and a reply to Ada's post reach the unread count on each of her
//     open home pages, live, and her notifications page lists them;
//   - Mark all read shows none unread at once, and reaches her other open
//     page, live;
//   - one's own like notifies no one;
//   - another user's session, signed in and not, sees none of it: by page,
//     and by the list read again (`/pw-read`).
import { expect, test } from "@playwright/test";
import { NOTIFICATIONS_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${NOTIFICATIONS_PORTS[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

const PASSWORD = "correct horse battery";

/** A page at `path`, its handlers attached. */
async function at(page, path) {
  await page.goto(path);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** A handle no other test or engine has: at most 15 of a-z, 0-9 and _. */
const handle = (testInfo, who) =>
  `${who}${testInfo.project.name.slice(0, 2)}${(Date.now() % 1e7).toString(36)}`.slice(0, 15);

/** Signed up at the development provider, from the home page, and back. */
async function signUp(page, who, name) {
  await at(page, "/");
  await page.getByRole("button", { name: "Sign up" }).click();
  await page.getByLabel("Handle").fill(who);
  await page.getByLabel("Name").fill(name);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await expect(page.locator("#me")).toHaveText(`Signed in as ${name}`);
}

/** Signed in to an account made before, from the home page, and back. */
async function signIn(page, who, name) {
  await at(page, "/");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.getByLabel("Handle").fill(who);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await expect(page.locator("#me")).toHaveText(`Signed in as ${name}`);
}

/**
 * **A press, and its command's answer** (ADR-0268): the page shows what a
 * press speculates before its request leaves, so a test that goes on to
 * navigate waits for the server's answer first.
 */
async function answered(page, command, press) {
  const answer = page.waitForResponse(`**/command/feed.app.${command}`);
  await press();
  await answer;
}

/** The timeline's post with `text`. */
const post = (page, text) => page.locator("main > ul > li").filter({ hasText: text });

/** Ada in two browsers, Ben in a third, and a post of Ada's. */
async function people(browser, testInfo) {
  const contexts = [await browser.newContext(), await browser.newContext(), await browser.newContext()];
  const [ada, again, ben] = await Promise.all(contexts.map((c) => c.newPage()));
  const engine = testInfo.project.name;
  const who = handle(testInfo, "ada");
  await signUp(ada, who, `Ada ${engine}`);
  await signIn(again, who, `Ada ${engine}`);
  await signUp(ben, handle(testInfo, "ben"), `Ben ${engine}`);
  const text = `Ada's at ${Date.now()}`;
  await ada.getByLabel("What's happening?").fill(text);
  await answered(ada, "post", () => ada.getByRole("button", { name: "Post" }).click());
  await expect(post(ada, text)).toHaveCount(1);
  // The row shown before the server answered is `pending-..`, which no like
  // finds: the server's row, by its id, before anyone presses on it.
  await expect(post(ada, text).locator(".author")).toHaveAttribute("href", /^\/post\/p\d+$/);
  return { contexts, ada, again, ben, text, engine };
}

test("a like and a reply reach each of the author's pages, and reading them each", async ({
  browser,
}, testInfo) => {
  const { contexts, ada, again, ben, text, engine } = await people(browser, testInfo);
  await at(again, "/");
  for (const page of [ada, again, ben]) {
    await expect(page.locator("#unread")).toHaveText("Notifications: 0 unread");
  }

  // Ben likes it, and replies on its page.
  await at(ben, "/");
  await answered(ben, "like", () => post(ben, text).getByRole("button", { name: "Like" }).click());
  await expect(ada.locator("#unread")).toHaveText("Notifications: 1 unread");
  await expect(again.locator("#unread")).toHaveText("Notifications: 1 unread");
  await post(ben, text).locator(".author").click();
  await ben.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await ben.getByLabel("Your reply").fill(`Ben answers ${engine}`);
  await answered(ben, "reply", () => ben.getByRole("button", { name: "Reply" }).click());
  // Live, on both of Ada's open pages; Ben's own is his.
  await expect(ada.locator("#unread")).toHaveText("Notifications: 2 unread");
  await expect(again.locator("#unread")).toHaveText("Notifications: 2 unread");
  await at(ben, "/");
  await expect(ben.locator("#unread")).toHaveText("Notifications: 0 unread");

  // Her page lists them, newest first.
  await again.locator("#unread a").click();
  await expect(again).toHaveURL(/\/notifications$/);
  await again.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  const rows = again.locator("#notifications > li");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText(`Ben ${engine}`);
  await expect(rows.nth(0)).toContainText("replied to your post");
  await expect(rows.nth(1)).toContainText("liked your post");
  await expect(again.locator("#unread-here")).toHaveText("2 unread");
  await expect(again.locator(".new")).toHaveCount(2);

  // Read: none unread at once, and on her other open page, live.
  await answered(again, "mark_read", () =>
    again.getByRole("button", { name: "Mark all read" }).click(),
  );
  await expect(again.locator("#unread-here")).toHaveText("0 unread");
  await expect(again.locator(".new")).toHaveCount(0);
  await expect(ada.locator("#unread")).toHaveText("Notifications: 0 unread");
  await at(again, "/notifications");
  await expect(again.locator("#unread-here")).toHaveText("0 unread");
  await expect(rows).toHaveCount(2);
  for (const c of contexts) await c.close();
});

test("one's own like notifies no one", async ({ browser }, testInfo) => {
  const { contexts, ada, again, text } = await people(browser, testInfo);
  await answered(ada, "like", () => post(ada, text).getByRole("button", { name: "Like" }).click());
  await expect(post(ada, text).locator(".likes")).toHaveText("1 like");
  await at(again, "/");
  await expect(again.locator("#unread")).toHaveText("Notifications: 0 unread");
  await at(again, "/notifications");
  await expect(again.locator("#no-notifications")).toBeVisible();
  for (const c of contexts) await c.close();
});

test("another user's session, signed in and not, sees none of it", async ({
  browser,
}, testInfo) => {
  const { contexts, ada, ben, text } = await people(browser, testInfo);
  await at(ben, "/");
  await answered(ben, "like", () => post(ben, text).getByRole("button", { name: "Like" }).click());
  await expect(ada.locator("#unread")).toHaveText("Notifications: 1 unread");
  const nobody = await browser.newContext();
  const stranger = await nobody.newPage();
  for (const page of [ben, stranger]) {
    // By page.
    await at(page, "/notifications");
    await expect(page.locator("#no-notifications")).toBeVisible();
    await expect(page.locator("#unread-here")).toHaveText("0 unread");
    await expect(page.locator("main")).not.toContainText("liked your post");
    // By the list read again for more: what the server reads is the
    // session's own, and nothing of Ada's reaches the page.
    const read = page.waitForResponse((r) => r.url().includes("/pw-read"));
    await page.locator("#more-notifications").click();
    expect((await read).status()).toBe(200);
    await expect(page.locator("#notifications > li")).toHaveCount(0);
    await expect(page.locator("main")).not.toContainText("liked your post");
    // And asked for with a document no page of this session's is: nothing.
    const forged = await page.request.get(
      `/pw-read?binding=notes&seq=9&doc=1&key=${encodeURIComponent('{"listed":60}')}`,
    );
    expect(await forged.text()).not.toContain("liked");
  }
  await nobody.close();
  for (const c of contexts) await c.close();
});
