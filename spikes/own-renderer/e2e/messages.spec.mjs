// Track `messages` (ADR-XXXX): direct messages, in three engines. The feed
// is served on hosts of its own (MESSAGES_PORTS), one per engine, its
// sessions signed in through the development identity provider
// (`PW_IDENTITY=dev-accounts`), so Ada reads in two browsers, Ben in a third,
// and Cy, a third user, beside them.
//   - a message reaches its recipient's open home page and its sender's
//     other open conversation, live; it is shown before the server answers
//     and waits until the server's row stands in its place (ADR-0275); the
//     list counts it; Mark read reads it; and an answer reaches the sender;
//   - who may message whom is X's rule: a stranger's composer is hidden,
//     with why, and shown, live, once the other follows them; no one
//     messages themselves;
//   - a third user, signed in and not, sees none of it: by page, and by the
//     conversation read again (`/pw-read`), for their own document and for
//     another's.
import { expect, test } from "@playwright/test";
import { MESSAGES_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MESSAGES_PORTS[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

const PASSWORD = "correct horse battery";

/** The page's handlers attached. */
const ready = (page) =>
  page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");

/** A page at `path`, its handlers attached. */
async function at(page, path) {
  await page.goto(path);
  await ready(page);
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
  await ready(page);
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
  await ready(page);
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

/**
 * **A user the feed knows by a post**, and their id: the user posts, and
 * their post's handle links to their page, `/user/{id}`, once the server's
 * row stands in the speculated one's place (ADR-0275).
 */
async function posts(page, text) {
  await page.getByLabel("What's happening?").fill(text);
  await answered(page, "post", () => page.getByRole("button", { name: "Post" }).click());
  const link = post(page, text).locator(".handle");
  await expect(link).toHaveAttribute("href", /^\/user\/.+$/);
  return (await link.getAttribute("href")).slice("/user/".length);
}

/** `follower` follows the user `id`, on their page. */
async function follows(page, id) {
  await at(page, `/user/${id}`);
  await answered(page, "follow", () => page.locator("#follow").click());
  await expect(page.locator("#unfollow")).toBeVisible();
}

/** Ada in two browsers, Ben in a third and Cy in a fourth, each posted. */
async function people(browser, testInfo) {
  const contexts = await Promise.all([0, 1, 2, 3].map(() => browser.newContext()));
  const [ada, again, ben, cy] = await Promise.all(contexts.map((c) => c.newPage()));
  const engine = testInfo.project.name;
  const who = handle(testInfo, "ada");
  await signUp(ada, who, `Ada ${engine}`);
  await signIn(again, who, `Ada ${engine}`);
  await signUp(ben, handle(testInfo, "ben"), `Ben ${engine}`);
  await signUp(cy, handle(testInfo, "cy"), `Cy ${engine}`);
  const ids = {
    ada: await posts(ada, `Ada here ${Date.now()}`),
    ben: await posts(ben, `Ben here ${Date.now()}`),
    cy: await posts(cy, `Cy here ${Date.now()}`),
  };
  return { contexts, ada, again, ben, cy, ids, engine };
}

/** The conversation's messages. */
const messages = (page) => page.locator("#messages > li");

test("a message reaches both users live, is read, and is answered", async ({
  browser,
}, testInfo) => {
  const { contexts, ada, again, ben, ids, engine } = await people(browser, testInfo);
  // Ben follows Ada, so Ada may message him.
  await follows(ben, ids.ada);
  await at(ben, "/");
  await expect(ben.locator("#dm-unread")).toHaveText("Messages: 0 unread");

  // Ada, from Ben's page, in both her browsers.
  await at(ada, `/user/${ids.ben}`);
  await ada.locator("#message-them a").click();
  await expect(ada).toHaveURL(new RegExp(`/messages/${ids.ben}$`));
  await ready(ada);
  await at(again, `/messages/${ids.ben}`);
  await expect(ada.locator("#composer")).toBeVisible();
  await expect(ada.locator("#no-messages")).toBeVisible();

  // Sent: shown at once, last, and waiting until the server's row stands
  // in its place; then on her other page and Ben's count, live.
  const text = `Hello Ben, from ${engine} ${Date.now()}`;
  await ada.getByLabel("Your message").fill(text);
  // The request held until the speculated message is seen waiting: it says
  // "Sending", and its sender's name links nowhere (ADR-0275).
  let release;
  const held = new Promise((resolve) => (release = resolve));
  await ada.route("**/command/feed.app.send", async (route) => {
    await held;
    await route.continue();
  });
  const sending = ada.waitForResponse("**/command/feed.app.send");
  await ada.getByRole("button", { name: "Send" }).click();
  await expect(messages(ada)).toHaveCount(1);
  await expect(messages(ada).last()).toContainText(text);
  await expect(messages(ada).last().locator(".sending")).toHaveText("Sending");
  await expect(messages(ada).last().locator("a.from")).toHaveCount(0);
  release();
  await sending;
  await ada.unroute("**/command/feed.app.send");
  await expect(ada.getByLabel("Your message")).toHaveValue("");
  await expect(messages(ada)).toHaveCount(1);
  await expect(messages(ada).last()).toContainText(text);
  await expect(messages(ada).last().locator("a.from")).toHaveAttribute("href", /^\/user\//);
  await expect(ada.locator(".sending")).toHaveCount(0);
  await expect(messages(again)).toHaveCount(1);
  await expect(messages(again).last()).toContainText(text);
  await expect(ben.locator("#dm-unread")).toHaveText("Messages: 1 unread");

  // Ben's list counts it; his conversation marks it New; Mark read reads it.
  await ben.locator("#dm-unread a").click();
  await expect(ben).toHaveURL(/\/messages$/);
  await ready(ben);
  await expect(ben.locator("#dm-unread-here")).toHaveText("1 unread message");
  const row = ben.locator("#conversations > li");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText(`Ada ${engine}`);
  await expect(row).toContainText(text);
  await expect(row.locator(".dm-new")).toHaveText("1 unread");
  await row.locator("a.with").click();
  await expect(ben).toHaveURL(new RegExp(`/messages/${ids.ada}$`));
  await ready(ben);
  await expect(messages(ben).last()).toContainText(text);
  await expect(ben.locator(".dm-new")).toHaveCount(1);
  await answered(ben, "read_conversation", () =>
    ben.getByRole("button", { name: "Mark read" }).click(),
  );
  await expect(ben.locator(".dm-new")).toHaveCount(0);

  // Ben answers, which he may: Ada has messaged him. It reaches both of
  // Ada's open conversations, live.
  await expect(ben.locator("#composer")).toBeVisible();
  const answer = `Hello Ada, from ${engine} ${Date.now()}`;
  await ben.getByLabel("Your message").fill(answer);
  await answered(ben, "send", () => ben.getByRole("button", { name: "Send" }).click());
  for (const page of [ada, again]) {
    await expect(messages(page)).toHaveCount(2);
    await expect(messages(page).last()).toContainText(answer);
  }
  await at(ben, "/");
  await expect(ben.locator("#dm-unread")).toHaveText("Messages: 0 unread");
  await at(again, "/");
  await expect(again.locator("#dm-unread")).toHaveText("Messages: 1 unread");
  for (const c of contexts) await c.close();
});

test("who may message whom is X's rule", async ({ browser }, testInfo) => {
  const { contexts, ada, cy, ids } = await people(browser, testInfo);
  // Cy, a stranger to Ada: the composer is hidden, and the page says why.
  await at(cy, `/messages/${ids.ada}`);
  await expect(cy.locator("#composer")).toBeHidden();
  await expect(cy.locator("#may-not-send")).toContainText("once they follow you");
  // A request sent anyway is refused by `MayMessage`.
  const refused = await cy.request.post("/command/feed.app.send", {
    headers: { "content-type": "application/json", "pw-interaction": `i-${Date.now()}` },
    data: JSON.stringify([ids.ada, "Let me in"]),
  });
  expect(refused.status()).toBe(403);
  expect(await refused.text()).toContain('"refused":"MayMessage"');
  // Ada follows Cy: Cy's open conversation opens, live.
  await follows(ada, ids.cy);
  await expect(cy.locator("#composer")).toBeVisible();
  await expect(cy.locator("#may-not-send")).toHaveCount(0);
  // No one messages themselves.
  await at(ada, `/messages/${ids.ada}`);
  await expect(ada.locator("#composer")).toBeHidden();
  await expect(ada.locator("#may-not-send")).toHaveText("You cannot message yourself.");
  for (const c of contexts) await c.close();
});

test("a third user, signed in and not, sees none of it", async ({ browser }, testInfo) => {
  const { contexts, ada, ben, cy, ids, engine } = await people(browser, testInfo);
  await follows(ben, ids.ada);
  const nobody = await browser.newContext();
  const stranger = await nobody.newPage();
  // Cy and a stranger each hold open a conversation with Ada while Ada
  // messages Ben.
  await at(cy, `/messages/${ids.ada}`);
  await at(stranger, `/messages/${ids.ada}`);
  await at(ben, `/messages/${ids.ada}`);
  const text = `Only for Ben ${engine} ${Date.now()}`;
  await at(ada, `/messages/${ids.ben}`);
  await ada.getByLabel("Your message").fill(text);
  await answered(ada, "send", () => ada.getByRole("button", { name: "Send" }).click());
  await expect(messages(ben).last()).toContainText(text);
  for (const page of [cy, stranger]) {
    await expect(page.locator("main")).not.toContainText(text);
    // By page: their list, and their conversations with each.
    await at(page, "/messages");
    await expect(page.locator("#no-conversations")).toBeVisible();
    await expect(page.locator("#conversations > li")).toHaveCount(0);
    for (const other of [ids.ada, ids.ben]) {
      await at(page, `/messages/${other}`);
      await expect(page.locator("#no-messages")).toBeVisible();
      await expect(page.locator("main")).not.toContainText(text);
    }
    // By the conversation read again for more: what the server reads is
    // the session's own.
    const read = page.waitForResponse((r) => r.url().includes("/pw-read"));
    await page.locator("#earlier-messages").click();
    expect((await read).status()).toBe(200);
    await expect(messages(page)).toHaveCount(0);
    await expect(page.locator("main")).not.toContainText(text);
    // And asked for with a document no page of this session's is: nothing.
    const forged = await page.request.get(
      `/pw-read?binding=talk&seq=9&doc=1&key=${encodeURIComponent('{"earlier":100}')}`,
    );
    expect(await forged.text()).not.toContain(text);
  }
  // A guest sends nothing, and is told to sign in.
  await expect(stranger.locator("#sign-in-to-send")).toBeVisible();
  await nobody.close();
  for (const c of contexts) await c.close();
});
