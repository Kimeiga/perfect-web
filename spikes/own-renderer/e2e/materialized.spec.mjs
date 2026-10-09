// ADR-0277: a materialization is kept, and a page reads it. The store's page
// shows its menu counted, `MenuLine(id)`, which reads `MenuSize(id)`, which
// reads the menu: a chain the host keeps, every reader's, made again when the
// menu changes, the count before the line, and each open page that reads it
// told, and no other.
//   - the store's page shows the line, the host's, with scripts off too;
//   - an item added reaches every open page of the store without a reload,
//     and an item taken away brings it back;
//   - another store's open page, which reads its own line, is told nothing.
import { expect, test } from "@playwright/test";
import { MUTABLE_PORTS } from "../playwright.config.mjs";

// The menu a test changes is one per server: a host of its own per engine.
test.use({
  baseURL: async ({}, use, testInfo) => {
    await use(`http://127.0.0.1:${MUTABLE_PORTS.materialized[testInfo.project.name]}`);
  },
});

test.describe.configure({ mode: "serial" });

/** A store's page, its handlers attached, and a mark a reload would lose. */
async function store(page, id) {
  await page.goto(`/stores/${id}`);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
  await page.evaluate(() => {
    window.__unreloaded = true;
  });
}

const unreloaded = (page) => page.evaluate(() => window.__unreloaded === true);

test("the store's page shows its menu counted, the host's", async ({ browser }) => {
  // With scripts off, as the host rendered it.
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto("/stores/47");
  await expect(page.locator("#menu-line")).toHaveText("3 items in 1 section");
  await context.close();
});

test("an item added reaches every open page of the store, and one taken away", async ({
  browser,
}) => {
  const [a, b, other] = [
    await browser.newContext(),
    await browser.newContext(),
    await browser.newContext(),
  ];
  const [first, second, elsewhere] = [await a.newPage(), await b.newPage(), await other.newPage()];
  await store(first, 47);
  await store(second, 47);
  await store(elsewhere, 48);
  for (const page of [first, second]) {
    await expect(page.locator("#menu-line")).toHaveText("3 items in 1 section");
  }
  // Store 48's menu is its own.
  await expect(elsewhere.locator("#menu-line")).toHaveText(/ items? in /);
  const theirs = await elsewhere.locator("#menu-line").textContent();
  const added = await first.request.post(
    "/command/menu?op=insert_before&id=flat-white&name=Flat%20White&at=cortado",
  );
  expect(added.ok()).toBe(true);
  // The count made again, then the line from it: each page of store 47 told.
  for (const page of [first, second]) {
    await expect(page.locator("#menu-line")).toHaveText("4 items in 1 section");
    expect(await unreloaded(page)).toBe(true);
  }
  // Store 48's line reads store 48's count, which the change to 47's menu
  // did not reach: it is told nothing.
  await expect(elsewhere.locator("#menu-line")).toHaveText(theirs);
  expect(await unreloaded(elsewhere)).toBe(true);
  const removed = await first.request.post("/command/menu?op=remove&id=flat-white");
  expect(removed.ok()).toBe(true);
  for (const page of [first, second]) {
    await expect(page.locator("#menu-line")).toHaveText("3 items in 1 section");
    expect(await unreloaded(page)).toBe(true);
  }
  for (const c of [a, b, other]) {
    await c.close();
  }
});
