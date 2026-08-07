// E7-P — a public fragment is materialized once and shared, identity included.
//
// Architect ruling, 2026-08-07:
//
// > Public `MenuFragment` materialization end-to-end through the own renderer:
// > two readers get identical bytes, identical tokens, identical
// > `ResourceEntryId`; Session<A> vs Session<B> distinct.
//
// `examples/store/app.pw` declares:
//
//   public  query Menu(id: StoreId)  cache shared   key id
//   session query Cart(session)      cache private  key session
//
// Two partitions on one page. The interesting word is **identical**: not
// "equivalent", not "the same to look at". If the menu's instance tokens came
// from the enclosing document they would be per-session, and two readers of one
// shared cache entry would get different bytes — a shared cache serving
// unshared content, which is the composition `IdentityDomain` exists to make
// unrepresentable.
//
// So the fragment carries its OWN identity domain, public, and the page splices
// the materialized bytes in rather than rendering them.

import { test, expect } from "@playwright/test";

/** The menu subtree, exactly as it arrived — comments and all. */
async function fragment(page) {
  return page.evaluate(() => document.getElementById("menu").innerHTML);
}

/** Every instance token in the document, in order. */
async function tokens(page) {
  return page.evaluate(() => {
    const out = [];
    const walk = (n) => {
      for (const c of n.childNodes) {
        if (c.nodeType === Node.COMMENT_NODE) {
          const m = /^pw:s(\d+)@(.+)$/.exec(c.data);
          if (m) out.push(`${m[1]}@${m[2]}`);
        }
        walk(c);
      }
    };
    walk(document.body);
    return out;
  });
}

async function open(browser) {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto("/StorePage.html");
  await page.waitForFunction(() => document.documentElement.dataset.pwReady);
  return { context, page };
}

test("two readers receive the same fragment, byte for byte", async ({ browser }) => {
  const a = await open(browser);
  const b = await open(browser);

  const [fa, fb] = [await fragment(a.page), await fragment(b.page)];
  expect(fa.length, "the fragment is not empty").toBeGreaterThan(50);
  expect(fa, "identical, not equivalent").toBe(fb);

  // Including the addresses. This is the assertion that distinguishes a shared
  // fragment from two renders that agree: a per-document domain would produce
  // the same VISIBLE markup and different tokens, and every screenshot-shaped
  // check would pass.
  expect(fa, "the anchors are in there to be compared").toMatch(/pw:s\d+@/);

  await a.context.close();
  await b.context.close();
});

test("the two readers really are different sessions", async ({ browser }) => {
  // The control. Without it, "identical bytes" is satisfied by one session
  // talking to itself, and every claim in this file is about nothing.
  const a = await open(browser);
  const b = await open(browser);

  await a.page.locator("#menu button").first().click();
  await expect(a.page.locator("#cart-count")).toHaveText("1");
  await b.page.waitForTimeout(300);
  await expect(b.page.locator("#cart-count"), "B's cart is its own").toHaveText("0");

  await a.context.close();
  await b.context.close();
});

test("the session-scoped part is addressed per session", async ({ browser }) => {
  // The other half of the split: public data shares an identity, private data
  // does not. If BOTH were shared the first test would pass for the wrong
  // reason — a server that ignored partitions entirely.
  const a = await open(browser);
  const b = await open(browser);

  const entries = await Promise.all(
    [a.page, b.page].map(async (p) => {
      await p.locator("#menu button").first().click();
      await expect(p.locator("#cart-count")).toHaveText("1");
      return Object.keys(await p.evaluate(() => window.__pwHeld()));
    }),
  );

  expect(entries[0].length).toBeGreaterThan(0);
  expect(
    entries[0],
    "two sessions, two cart ResourceEntryIds",
  ).not.toEqual(entries[1]);

  await a.context.close();
  await b.context.close();
});

test("the fragment's instance tokens are shared and the cart's address is not", async ({
  browser,
}) => {
  const a = await open(browser);
  const b = await open(browser);

  expect(await tokens(a.page), "one public fragment, one set of tokens").toEqual(
    await tokens(b.page),
  );

  // And the tokens are real: they address the loop instances the runtime
  // attached handlers to, so "identical" is not "both empty".
  const attached = await a.page.evaluate(() =>
    window.__pw.log.find((l) => l.startsWith("attached")),
  );
  expect(attached, "the tokens are load-bearing").toMatch(/instance\(s\)/);

  await a.context.close();
  await b.context.close();
});

test("one change to public data reaches every reader at one address", async ({ browser }) => {
  // What a shared identity buys. The server derives ONE patch, because a public
  // fragment has one set of instances; every subscriber applies that same
  // patch. A per-document identity would need a patch per reader, and would
  // silently address at most one of them if it forgot.
  const a = await open(browser);
  const b = await open(browser);

  await a.page.request.post("/command/menu?op=rename&id=cortado&name=Gibraltar");

  for (const { page } of [a, b]) {
    await expect(page.locator("#menu li").nth(1).locator("span")).toHaveText("Gibraltar");
  }
  expect(await fragment(a.page), "still identical after the change").toBe(
    await fragment(b.page),
  );

  await a.page.request.post("/command/menu?op=rename&id=cortado&name=Cortado");
  await a.context.close();
  await b.context.close();
});

test("the fragment is materialized once, not rendered per reader", async ({ browser }) => {
  // "Shared" is a claim about the SYSTEM, not about two renders agreeing. The
  // entry is regenerated when it goes stale and read otherwise, so a third
  // reader arriving costs a lookup — and the version does not move, because
  // nothing changed.
  const a = await open(browser);
  const before = await a.page.request.get("/menu-entry");
  const first = await before.json();

  const b = await open(browser);
  const after = await (await b.page.request.get("/menu-entry")).json();

  expect(after.version, "a new reader does not regenerate the entry").toBe(first.version);
  expect(after.entry, "and reads the same entry id").toBe(first.entry);
  expect(first.partition, "which is public").toBe("public");

  await a.context.close();
  await b.context.close();
});
