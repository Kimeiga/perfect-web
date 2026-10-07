// Track `identity` (ADR-XXXX): accounts and sign-in on the feed, in three
// engines. The feed is served on hosts of its own (IDENTITY_PORTS), one per
// engine, its sessions signed in through the development identity provider
// (`PW_IDENTITY=dev-accounts`), which this server serves at /dev-idp as a
// deployment's provider would be served at its own origin.
//   - a signed-out reader reads the timeline and is shown how to sign in,
//     where the post form's guard is; a post sent anyway is refused by the
//     server (`requires SignedIn`, 403) and taken back;
//   - sign-up at the provider, back signed in, the session cookie HttpOnly
//     and SameSite=Lax and a new one;
//   - a post is its author's: named for them to every reader, and a Delete
//     button on their row alone; deleting it removes it from every open
//     timeline; another user has no Delete for it;
//   - sign-out forgets the session, and sign-in takes the password.
import { expect, test } from "@playwright/test";
import { IDENTITY_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${IDENTITY_PORTS[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

/** The home page, its handlers attached. */
async function home(page) {
  await page.goto("/");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** A handle no other test or engine has: at most 15 of a-z, 0-9 and _. */
const handle = (testInfo, who) =>
  `${who}${testInfo.project.name.slice(0, 2)}${(Date.now() % 1e7).toString(36)}`.slice(0, 15);

/** The timeline's post with `text`. */
const post = (page, text) => page.locator("main > ul > li").filter({ hasText: text });

const PASSWORD = "correct horse battery";

/** Signed up at the development provider, from the home page, and back. */
async function signUp(page, who, name) {
  await home(page);
  await page.getByRole("button", { name: "Sign up" }).click();
  await expect(page.getByRole("note")).toContainText("not for production");
  await page.getByLabel("Handle").fill(who);
  await page.getByLabel("Name").fill(name);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await expect(page.locator("#me")).toHaveText(`Signed in as ${name}`);
}

/** The session cookie's value, and the cookie. */
async function sessionCookie(context) {
  const cookies = await context.cookies();
  return cookies.find((c) => c.name === "pw-session");
}

test("a signed-out reader reads, and a post sent anyway is refused", async ({ page }) => {
  await home(page);
  await expect(post(page, "Hello, feed.")).toHaveCount(1);
  await expect(page.locator("#signed-out")).toHaveText("Sign in to post, reply or like.");
  await expect(page.getByRole("button", { name: "Sign in" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Sign out" })).toHaveCount(0);
  // A post sent anyway: the server refuses it before the command runs, and
  // the row shown before its answer is taken back.
  const text = `Refused at ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  const refused = page.waitForResponse((r) => r.url().includes("/command/feed.app.post"));
  await page.getByRole("button", { name: "Post" }).click();
  expect((await refused).status()).toBe(403);
  await expect(post(page, text)).toHaveCount(0);
  // What the server answered, asked again with the page's session (the
  // runtime reads no body from a refused command, and Chromium keeps none).
  const again = await page.request.post("/command/feed.app.post", {
    data: [text],
    headers: { "pw-interaction": `refused-${Date.now()}` },
  });
  expect(again.status()).toBe(403);
  expect(await again.json()).toEqual({ committed: false, refused: "SignedIn" });
  await page.reload();
  await expect(post(page, text)).toHaveCount(0);
});

test("a post is its author's, and theirs alone to delete", async ({ browser }, testInfo) => {
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [author, reader] = [await a.newPage(), await b.newPage()];
  await home(author);
  const before = await sessionCookie(a);
  const name = `Ada ${testInfo.project.name}`;
  await signUp(author, handle(testInfo, "ada"), name);
  // A new session, which no script reads, sent cross-site only on a
  // top-level navigation.
  const after = await sessionCookie(a);
  expect(after.value).not.toBe(before.value);
  expect(after.httpOnly).toBe(true);
  expect(after.sameSite).toBe("Lax");

  const text = `By ${name} at ${Date.now()}`;
  await author.getByLabel("What's happening?").fill(text);
  await author.getByRole("button", { name: "Post" }).click();
  await expect(post(author, text).getByRole("link")).toHaveText(name);
  await expect(post(author, text).getByRole("button", { name: "Delete" })).toHaveCount(1);

  // Another reader, signed out, then signed in as someone else: the post
  // is named for its author, and has no Delete for them.
  await home(reader);
  await expect(post(reader, text).getByRole("link")).toHaveText(name);
  await expect(post(reader, text).getByRole("button", { name: "Delete" })).toHaveCount(0);
  await signUp(reader, handle(testInfo, "grace"), `Grace ${testInfo.project.name}`);
  await expect(post(reader, text)).toHaveCount(1);
  await expect(post(reader, text).getByRole("button", { name: "Delete" })).toHaveCount(0);

  // Its author deletes it, and it is gone from every open timeline.
  await post(author, text).getByRole("button", { name: "Delete" }).click();
  await expect(post(author, text)).toHaveCount(0);
  await expect(post(reader, text)).toHaveCount(0);
  await a.close();
  await b.close();
});

test("signing out forgets the session, and signing in takes the password", async ({
  browser,
}, testInfo) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  const who = handle(testInfo, "hopper");
  const name = `Grace Hopper ${testInfo.project.name}`;
  await signUp(page, who, name);
  const signedIn = await sessionCookie(context);
  await page.getByRole("button", { name: "Sign out" }).click();
  await expect(page.locator("#signed-out")).toBeVisible();
  const signedOut = await sessionCookie(context);
  expect(signedOut.value).not.toBe(signedIn.value);
  // The old session, sent again, is no one.
  const replayed = await browser.newContext();
  await replayed.addCookies([{ ...signedIn, sameSite: "Lax" }]);
  const old = await replayed.newPage();
  await home(old);
  await expect(old.locator("#signed-out")).toBeVisible();
  await replayed.close();

  // A wrong password is said so, at the provider.
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.getByLabel("Handle").fill(who);
  await page.getByLabel("Password").fill("not the password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("alert")).toHaveText(
    "That handle and password do not match an account.",
  );
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator("#me")).toHaveText(`Signed in as ${name}`);
  await context.close();
});
