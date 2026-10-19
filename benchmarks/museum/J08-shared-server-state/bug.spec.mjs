// J08: one customer's data served to another from state the server shares.
//
// Passes when the bug happens: Bob, in a session of his own, who has added
// nothing, is shown the count of Alice's cart.
import { test, expect } from "./fixtures.mjs";

test("a second customer is shown the first customer's cart", async ({ browser, baseURL, store }) => {
  const alice = await browser.newContext({ baseURL });
  const bob = await browser.newContext({ baseURL });
  const pa = await alice.newPage();
  const pb = await bob.newPage();

  await store.open(pa);
  await store.add(pa);
  await store.add(pa);
  await expect(store.count(pa)).toHaveText("2");

  await store.open(pb);
  await expect(store.count(pb), "Bob has added nothing, and is shown Alice's 2").toHaveText("2");

  await alice.close();
  await bob.close();
});
