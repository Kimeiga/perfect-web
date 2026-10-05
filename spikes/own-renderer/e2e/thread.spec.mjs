// ADR-0203: a view that contains itself.
//
// `examples/demo/thread.pw` shows a discussion through `Thread`, a view that
// shows each reply as itself, and an outline through `Outline`, another, in
// a block a signal decides. What this shows:
// - each use of such a view is an instance of its template, in a frame of
//   its own, as deep as the data goes, and no element is named after it;
// - a button in an instance three deep is bound in its own instance, and
//   sends its own comment;
// - the browser renders an instance a signal gives again, and a block
//   holding instances, from the view's template the document carries, and
//   binds what it rendered.
import { expect, test } from "@playwright/test";

const THREAD = "/page/demo.thread.ThreadPage";

async function ready(page) {
  await page.goto(THREAD);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** A comment's own button, by its text. */
function upvote(page, text) {
  return page
    .locator("#thread p", { hasText: text })
    .locator("xpath=following-sibling::button[1]");
}

test("each use of a view that contains itself is an instance, as deep as its data", async ({
  page,
  request,
}) => {
  const html = await (await request.get(THREAD)).text();
  const markup = html.split('<script type="application/json"')[0];
  for (const tag of ["<Thread", "<thread", "<Outline", "<outline"]) {
    expect(markup).not.toContain(tag);
  }
  // Each instance and each row in a frame of its own, none named twice.
  const frames = [...markup.matchAll(/<!--pw:s(\d+)@([A-Za-z0-9_-]+)-->/g)].map(
    ([, id, token]) => `${id}@${token}`,
  );
  // Five comments, each an instance, and four replies, each a row; and the
  // outline's heading, shut, an instance with no rows.
  expect(frames.length).toBe(10);
  expect(new Set(frames).size).toBe(10);

  await ready(page);
  await expect(page.locator("#thread li")).toHaveCount(5);
  // Every range, every element a part is on, each instance, the five
  // comments' and the outline's, and the title: each its own address. A
  // view inside itself opens a part of the same id inside one still open,
  // and each is paired with its own.
  const counted = await page.evaluate(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
    let ranges = 0;
    while (walker.nextNode()) if (/^pw:s\d+$/.test(walker.currentNode.data)) ranges += 1;
    const elements = document.querySelectorAll("[data-pw]").length;
    return { ranges, elements, indexed: window.__pw.indexSize() };
  });
  expect(counted.indexed).toBe(counted.ranges + counted.elements + 6 + 1);
  // The deepest reply, four lists down.
  await expect(page.locator("#thread > li > ul > li > ul > li > ul > li > p")).toHaveText(
    "bo: Twenty grams in.",
  );
  // The other branch, beside the first reply.
  await expect(page.locator("#thread > li > ul > li").nth(1).locator("> p")).toHaveText(
    "cy: The cortado is better.",
  );
});

test("a button in an instance sends its own comment, at any depth", async ({ page }) => {
  await ready(page);
  const sent = [];
  page.on("request", (r) => {
    if (r.url().includes("/command/")) {
      sent.push({ path: new URL(r.url()).pathname, body: r.postData() });
    }
  });
  // Bound in the view's instances, by the view's template.
  const log = await page.evaluate(() => window.__pw.log.join("\n"));
  expect(log).toMatch(/attached demo\.thread\.Thread:\d+ to 5 instance\(s\)/);

  // Three deep, so a handler bound only at the top, or to the first
  // comment's button, fails.
  const answered = page.waitForResponse((r) => r.url().includes("/command/demo.thread.upvote"));
  await upvote(page, "How short?").click();
  const response = await answered;
  expect(response.status()).toBe(202);
  expect((await response.json()).committed).toBe(true);
  expect(sent).toEqual([{ path: "/command/demo.thread.upvote", body: JSON.stringify([4]) }]);

  // And the top one, its own.
  const again = page.waitForResponse((r) => r.url().includes("/command/demo.thread.upvote"));
  await upvote(page, "Is the espresso good?").click();
  await again;
  expect(sent[1]).toEqual({ path: "/command/demo.thread.upvote", body: JSON.stringify([1]) });
});

test("the browser renders an instance a signal gives again", async ({ page }) => {
  await ready(page);
  // Shut: the top heading alone.
  await expect(page.locator("#outline li")).toHaveCount(1);
  await expect(page.locator("#outline > li")).toHaveText("Espresso");
  await page.locator("#expand").click();
  // Rendered again in the browser, from the view's template the document
  // carries, each instance in a frame of its own.
  await expect(page.locator("#outline li")).toHaveCount(3);
  await expect(page.locator("#outline > li > ol > li > ol > li")).toHaveText("Short");
  const frames = await page.evaluate(() => {
    const walker = document.createTreeWalker(
      document.getElementById("outline"),
      NodeFilter.SHOW_COMMENT,
    );
    const out = [];
    while (walker.nextNode()) {
      if (/^pw:s\d+@/.test(walker.currentNode.data)) out.push(walker.currentNode.data);
    }
    return out;
  });
  // Three instances and two rows, each its own frame.
  expect(frames.length).toBe(5);
  expect(new Set(frames).size).toBe(5);
  await page.locator("#expand").click();
  await expect(page.locator("#outline li")).toHaveCount(1);
});

test("the browser renders a block holding instances again, and binds it", async ({ page }) => {
  await ready(page);
  await expect(page.locator("#no-summary")).toBeVisible();
  await page.locator("#toggle").click();
  await expect(page.locator("#summary li")).toHaveCount(1);
  // The block reads the outline's signals too: expanded, it is rendered
  // again with them.
  await page.locator("#expand").click();
  await expect(page.locator("#summary li")).toHaveCount(3);
  await expect(page.locator("#summary > ol > li > ol > li > ol > li")).toHaveText("Short");

  // Hidden and shown again, the same.
  await page.locator("#toggle").click();
  await expect(page.locator("#no-summary")).toBeVisible();
  await page.locator("#toggle").click();
  await expect(page.locator("#summary li")).toHaveCount(3);

  // The thread's buttons are still its own after the page was indexed again.
  const answered = page.waitForResponse((r) => r.url().includes("/command/demo.thread.upvote"));
  await upvote(page, "Twenty grams in.").click();
  const response = await answered;
  expect(JSON.parse(response.request().postData())).toEqual([5]);
});

test("a change the server sends replaces an instance whole, and binds it", async ({ page }) => {
  // A host renders an instance a query's value gives again, whole, when the
  // value changed (ADR-0203): one patch, to the instance's own range, at its
  // part in the page.
  await ready(page);
  const manifest = await page.evaluate(() => window.__pw.parts);
  const part = manifest.parts.find((p) => p.kind === "instance");
  expect(part.value).toBe("demo.thread.Thread");
  const html = (await page.locator("#thread").innerHTML()).replace(
    "Twenty grams in.",
    "Eighteen grams in.",
  );
  expect(html.startsWith(`<!--pw:s${part.id}@`)).toBe(true);
  await page.evaluate(
    (f) => window.__pwTestApply(f),
    {
      frame: "patch_set",
      protocol: 1,
      basis: { resources: [{ entry: "thread-test", version: 1 }] },
      patches: [
        {
          target: { template: manifest.schema, instances: [], part: part.id },
          operation: { op: "replace_range", html },
        },
      ],
    },
  );
  await expect(page.locator("#thread > li > ul > li > ul > li > ul > li > p")).toHaveText(
    "bo: Eighteen grams in.",
  );
  await expect(page.locator("#thread li")).toHaveCount(5);
  // What it put in place is bound, in its own instance.
  const answered = page.waitForResponse((r) => r.url().includes("/command/demo.thread.upvote"));
  await upvote(page, "Eighteen grams in.").click();
  const response = await answered;
  expect(JSON.parse(response.request().postData())).toEqual([5]);
});
