// Track `uploads` (ADR-0260): an image on a post, in three engines. The
// feed's build (`feed.sh`), served on hosts of the track's own, one per
// engine, each with its own blob storage (`UPLOADS_PORTS`, PW_BLOB_DIR):
//   - an image attached by the home page's form, a plain multipart form the
//     host reads within the program's limits, is shown to its author before
//     it is posted, and posted with the words for it;
//   - each kind a real encoder wrote (PNG, baseline and progressive JPEG, a
//     JPEG an Exif `Orientation` turns, lossy, lossless and extended WebP,
//     GIF) is shown at the width and height its markup states: the image
//     the engine decodes is that size, so the page keeps its place;
//   - while the image is still on its way, its box is already that size
//     (ADR-0187: no layout shift);
//   - it is served as what its bytes are, `nosniff`, sandboxed and kept by
//     any cache forever, and a file that is not an image, or is too wide, is
//     refused with why and the way back;
//   - an attached image removed is not posted.
import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { UPLOADS_PORTS } from "../playwright.config.mjs";

test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${UPLOADS_PORTS[testInfo.project.name]}`);
  },
});

// One reader's lease at a time on a host: its tests in order.
test.describe.configure({ mode: "serial" });

const FIXTURES = new URL("./uploads/", import.meta.url);
const fixtures = JSON.parse(readFileSync(new URL("fixtures.json", FIXTURES), "utf8"));
const path = (name) => new URL(name, FIXTURES).pathname;

/** The home page, its handlers attached. */
async function home(page) {
  await page.goto("/");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** Attach `file` with the home page's form, and wait for the page again. */
async function attach(page, file) {
  await page.locator("#attach input[type=file]").setInputFiles(path(file));
  await Promise.all([
    page.waitForNavigation(),
    page.locator("#attach").getByRole("button", { name: "Attach", exact: true }).click(),
  ]);
}

/** Post `text` with the image attached, described by `alt`. */
async function postWith(page, text, alt) {
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await page.getByLabel("What's happening?").fill(text);
  await page.getByLabel("The image, in words, for whoever cannot see it").fill(alt);
  await page.getByRole("button", { name: "Post", exact: true }).click();
}

const row = (page, text) => page.locator("main > ul > li").filter({ hasText: text });

test("an image attached is shown to its author, and posted with its words", async ({
  browser,
}, testInfo) => {
  const [a, b] = [await browser.newContext(), await browser.newContext()];
  const [author, reader] = [await a.newPage(), await b.newPage()];
  for (const page of [author, reader]) await home(page);
  await attach(author, "six-by-four.png");
  const preview = author.locator(".attached-image");
  await expect(preview).toHaveAttribute("width", "6");
  await expect(preview).toHaveAttribute("height", "4");
  await expect(reader.locator(".attached-image")).toHaveCount(0);

  const text = `A gradient in ${testInfo.project.name} at ${Date.now()}`;
  await postWith(author, text, "Red to the right, green downward");
  for (const page of [author, reader]) {
    const photo = row(page, text).locator("img.photo");
    await expect(photo).toHaveAttribute("alt", "Red to the right, green downward");
    await expect(photo).toHaveAttribute("width", "6");
    await expect(photo).toHaveAttribute("height", "4");
    await expect(photo).toHaveAttribute("src", /^\/images\/[0-9a-f]{64}\.png$/);
  }
  // Posted, it is no longer attached.
  await expect(author.locator(".attached-image")).toHaveCount(0);
  // Its thread shows it too.
  await row(reader, text).locator("a.author").click();
  await expect(reader.locator("article img.photo").first()).toHaveAttribute("width", "6");
  await a.close();
  await b.close();
});

test("each kind is shown at the size its markup states", async ({ page }, testInfo) => {
  const kinds = fixtures.filter((f) => f.kind && f.file !== "too-wide.png");
  expect(kinds.length).toBe(8);
  await home(page);
  for (const f of kinds) {
    await attach(page, f.file);
    const text = `${f.file} in ${testInfo.project.name} at ${Date.now()}`;
    await postWith(page, text, `A ${f.kind} picture`);
    const photo = row(page, text).locator("img.photo");
    await expect(photo).toHaveAttribute("width", String(f.width));
    await expect(photo).toHaveAttribute("height", String(f.height));
    await expect(photo).toHaveAttribute("src", new RegExp(`\\.${f.kind}$`));
    // What the engine decoded is the size the markup states: an Exif
    // `Orientation` turns a JPEG as it is shown, and the host read it so.
    await expect
      .poll(() => photo.evaluate((img) => (img.complete ? [img.naturalWidth, img.naturalHeight] : null)))
      .toEqual([f.width, f.height]);
  }
});

test("an image on its way already has its box", async ({ page }, testInfo) => {
  await home(page);
  await attach(page, "ten-by-six-lossy.webp");
  const text = `Held back in ${testInfo.project.name} at ${Date.now()}`;
  await postWith(page, text, "A small gradient");
  await expect(row(page, text).locator("img.photo")).toHaveCount(1);
  // Read again with every image held back: the box is the markup's before a
  // byte of the image arrives.
  let release;
  const held = new Promise((r) => (release = r));
  await page.route("**/images/**", async (route) => {
    await held;
    await route.continue();
  });
  // Not "load", which waits for the image held back.
  await page.reload({ waitUntil: "domcontentloaded" });
  const photo = row(page, text).locator("img.photo");
  const box = await photo.evaluate((img) => {
    const r = img.getBoundingClientRect();
    return [img.complete && img.naturalWidth > 0, r.width, r.height];
  });
  expect(box).toEqual([false, 10, 6]);
  release();
  await expect.poll(() => photo.evaluate((img) => img.naturalWidth)).toBe(10);
  const after = await photo.evaluate((img) => {
    const r = img.getBoundingClientRect();
    return [r.width, r.height];
  });
  expect(after).toEqual([10, 6]);
  await page.unroute("**/images/**");
});

test("an image is served as what its bytes are, and nothing else is kept", async ({
  page,
  request,
}, testInfo) => {
  await home(page);
  await attach(page, "five-by-nine.gif");
  const text = `Served in ${testInfo.project.name} at ${Date.now()}`;
  await postWith(page, text, "A tall gradient");
  const src = await row(page, text).locator("img.photo").getAttribute("src");
  const served = await request.get(src);
  expect(served.status()).toBe(200);
  const headers = served.headers();
  expect(headers["content-type"]).toBe("image/gif");
  expect(headers["x-content-type-options"]).toBe("nosniff");
  expect(headers["content-security-policy"]).toBe("default-src 'none'; sandbox");
  expect(headers["cache-control"]).toBe("public, max-age=31536000, immutable");
  expect(Buffer.from(await served.body())).toEqual(readFileSync(path("five-by-nine.gif")));

  // A page named as an image is refused, said why, with the way back.
  await home(page);
  await page.locator("#attach input[type=file]").setInputFiles(path("not-an-image.png"));
  const [refused] = await Promise.all([
    page.waitForNavigation(),
    page.locator("#attach").getByRole("button", { name: "Attach", exact: true }).click(),
  ]);
  expect(refused.status()).toBe(415);
  await expect(page.getByRole("alert")).toContainText("The file is not an image");
  // Nothing of it ran.
  expect(await page.title()).toBe("Not attached");
  await page.getByRole("link", { name: "Back" }).click();
  await expect(page.locator(".attached-image")).toHaveCount(0);

  await page.locator("#attach input[type=file]").setInputFiles(path("too-wide.png"));
  const [wide] = await Promise.all([
    page.waitForNavigation(),
    page.locator("#attach").getByRole("button", { name: "Attach", exact: true }).click(),
  ]);
  expect(wide.status()).toBe(422);
  await expect(page.getByRole("alert")).toContainText("4097 by 1 pixels");
});

test("an attached image removed is not posted", async ({ page }, testInfo) => {
  await home(page);
  await attach(page, "eight-by-five.jpeg");
  await expect(page.locator(".attached-image")).toHaveCount(1);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await page.getByRole("button", { name: "Remove image" }).click();
  await expect(page.locator(".attached-image")).toHaveCount(0);
  const text = `Words alone in ${testInfo.project.name} at ${Date.now()}`;
  await page.getByLabel("What's happening?").fill(text);
  await page.getByRole("button", { name: "Post", exact: true }).click();
  await expect(row(page, text)).toHaveCount(1);
  await expect(row(page, text).locator("img.photo")).toHaveCount(0);
});
