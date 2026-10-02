// ADR-0136: a view used in another is written where it is used.
//
// `examples/demo/panel.pw` shows its signals through two views, and
// `examples/demo/pick.pw` uses one view twice, each time given another item.
// What this shows:
// - a composed view is markup in the page's document, not an element named
//   after it;
// - a signal a view shows is rendered again where the view shows it, at top
//   level and inside a block the browser renders;
// - a handler written in a view is the page's: its button is bound, its code
//   loads, and the document carries what it captures under the name the
//   view's handler reads it by, each use its own;
// - a page's parameter comes from the address, escaped where it is written.
import { expect, test } from "@playwright/test";

const PANEL = "/page/demo.panel.PanelPage";
const PICK = "/page/demo.pick.PickPage";

async function ready(page, url) {
  await page.goto(url);
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1");
}

/** The parts manifest a document carries. */
function manifestOf(html) {
  const json = html.split('<script type="application/json" id="pw-parts">')[1].split("</script>")[0];
  return JSON.parse(json);
}

test("a composed view is markup in the page, not an element named after it", async ({
  request,
}) => {
  const html = await (await request.get(PANEL)).text();
  const markup = html.split('<script type="application/json"')[0];
  expect(markup).toContain('<h1 id="greeting"><!--pw:s0-->Hello<!--pw:e0--></h1>');
  for (const tag of ["<Greeting", "<greeting", "<HelpLines", "<helplines"]) {
    expect(markup).not.toContain(tag);
  }
});

test("a signal a view shows is rendered again where the view shows it", async ({ page }) => {
  await ready(page, PANEL);
  // Top level: the heading `Greeting` shows.
  await page.locator("#shout").click();
  await expect(page.locator("#greeting")).toHaveText("<b>Hi</b> & bye");
  // Inside the block the browser renders: `HelpLines`, given two signals.
  await page.locator("#press").click();
  await page.locator("#open-help").click();
  await expect(page.locator("#help-panel")).toContainText("Pressed 1 time(s) so far.");
  await expect(page.locator("#help-greeting")).toHaveText("<b>Hi</b> & bye");
  await page.locator("#close-help").click();
  await page.locator("#press").click();
  await page.locator("#open-help").click();
  await expect(page.locator("#help-panel")).toContainText("Pressed 2 time(s) so far.");
});

test("each use of a view carries its own item, under the view's name for it", async ({
  page,
}) => {
  await ready(page, `${PICK}?first=espresso&second=cold-brew`);
  const first = page.locator("#first button");
  const second = page.locator("#second button");
  // The page gave `first` and `second`; the view's handler reads `item`.
  expect(JSON.parse(await first.getAttribute("data-pw-captures"))).toEqual({ item: "espresso" });
  expect(JSON.parse(await second.getAttribute("data-pw-captures"))).toEqual({
    item: "cold-brew",
  });
  // Two elements of the page's own numbering, and one handler: the view's.
  const owners = [await first.getAttribute("data-pw"), await second.getAttribute("data-pw")];
  expect(new Set(owners).size).toBe(2);
  const manifest = manifestOf(await page.content());
  const events = manifest.parts.filter((p) => p.kind === "event");
  expect(events.length).toBe(2);
  expect(new Set(events.map((e) => e.value)).size).toBe(1);
});

test("a press on a view's button sends that use's item", async ({ page }) => {
  await ready(page, `${PICK}?first=espresso&second=cold-brew`);
  const sent = [];
  page.on("request", (r) => {
    if (r.url().includes("/command/")) {
      sent.push({ path: new URL(r.url()).pathname, body: r.postData() });
    }
  });
  const answered = page.waitForResponse((r) => r.url().includes("/command/demo.pick.pick"));
  // The SECOND button, so code that always sent the first item fails.
  await page.locator("#second button").click();
  const response = await answered;
  // The command ran, as the component the compiler built.
  expect(response.status()).toBe(202);
  expect((await response.json()).committed).toBe(true);
  expect(sent).toEqual([{ path: "/command/demo.pick.pick", body: JSON.stringify(["cold-brew"]) }]);

  const again = page.waitForResponse((r) => r.url().includes("/command/demo.pick.pick"));
  await page.locator("#first button").click();
  await again;
  expect(sent[1]).toEqual({ path: "/command/demo.pick.pick", body: JSON.stringify(["espresso"]) });
});

test("a page's parameter is the address's, escaped where it is written", async ({
  page,
  request,
}) => {
  // Not given: not rendered with a guess.
  const missing = await request.get(`${PICK}?first=espresso`);
  expect(missing.status()).toBe(404);
  expect(await missing.text()).toContain("is given no `second`");

  // Markup in the address is text in the attribute, and the value the
  // handler reads back.
  await ready(page, `${PICK}?first=%3Cb%3Ex%22&second=cold-brew`);
  const raw = await (await request.get(`${PICK}?first=%3Cb%3Ex%22&second=cold-brew`)).text();
  expect(raw).not.toContain('"<b>x"');
  expect(
    JSON.parse(await page.locator("#first button").getAttribute("data-pw-captures")),
  ).toEqual({ item: '<b>x"' });
  await expect(page.locator("#first b")).toHaveCount(0);
});
