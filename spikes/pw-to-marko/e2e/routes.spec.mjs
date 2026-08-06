// Charter §14 M3 gate, in a real browser.
//
// Every route under src/routes is GENERATED from `.pw` (ADR-0017), so these
// tests exercise the adapter's output rather than a hand-written template.
import { test, expect } from "@playwright/test";

test.describe("static route", () => {
  test("is usable with JavaScript disabled", async ({ browser }) => {
    // The gate's first item. Asserted with JS actually OFF rather than by
    // counting script tags: a page can ship zero scripts and still depend on
    // one, and counting would not notice.
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/static");

    await expect(page.getByRole("heading", { name: "Blue Bottle" })).toBeVisible();
    await expect(page.getByRole("list", { name: "Menu" })).toBeVisible();
    await expect(page.getByText("Espresso")).toBeVisible();
    await expect(page.getByText("$3.50")).toBeVisible();
    await context.close();
  });

  test("ships no script at all", async ({ page }) => {
    const responses = [];
    page.on("response", (r) => responses.push(r.url()));
    await page.goto("/static", { waitUntil: "networkidle" });

    const scripts = await page.locator("script").count();
    expect(scripts, "a static route must contain no <script>").toBe(0);
    const js = responses.filter((u) => u.endsWith(".js") || u.includes("/assets/"));
    expect(js, "and must request none").toEqual([]);
  });

  test("keeps the whitespace between inline elements", async ({ page }) => {
    // The parser treated inter-element whitespace as trivia and dropped it, so
    // the page rendered "Espresso$3.50". Assert the rendered text, because the
    // template can look right and the DOM still be wrong.
    await page.goto("/static");
    const item = page.getByRole("listitem").first();
    await expect(item).toHaveText(/Espresso\s+\$3\.50/);
  });
});

test.describe("counter route", () => {
  test("interaction works without re-executing the page", async ({ page }) => {
    await page.goto("/counter");

    // Mark a node that the server rendered. If the client replayed the whole
    // component tree, this element would be replaced and the marker lost —
    // which is the property charter §8.5 actually claims.
    const marked = await page.evaluate(() => {
      const el = document.querySelector('section[aria-label="Store information"] h2');
      el.dataset.serverRendered = "yes";
      return el.textContent;
    });
    expect(marked).toBe("Menu");

    await expect(page.locator("#count")).toHaveText("0");
    await page.locator("#add").click();
    await expect(page.locator("#count")).toHaveText("1");
    await page.locator("#add").click();
    await expect(page.locator("#count")).toHaveText("2");

    const survived = await page.evaluate(
      () =>
        document.querySelector('section[aria-label="Store information"] h2')?.dataset
          .serverRendered,
    );
    expect(survived, "the inert region must not have been re-rendered").toBe("yes");
  });

  test("is readable before its JavaScript runs", async ({ browser }) => {
    // Resumption's premise: the server sends usable HTML. With JS off the
    // button does nothing, and that is expected — the content is still there.
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/counter");
    await expect(page.getByRole("heading", { name: "Blue Bottle" })).toBeVisible();
    await expect(page.locator("#count")).toHaveText("0");
    await context.close();
  });
});

test.describe("accessibility baseline", () => {
  // Charter §14 M3 task 13. Not a full audit — the structural properties the
  // generated markup must not lose.
  for (const route of ["/static", "/counter"]) {
    test(`${route} keeps its semantic structure`, async ({ page }) => {
      await page.goto(route);
      await expect(page.getByRole("main")).toHaveCount(1);
      await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);

      // Every landmark the source labelled must still be labelled: an adapter
      // that dropped `aria-label` would render identically to a sighted user.
      const labelled = await page.evaluate(() =>
        [...document.querySelectorAll("[aria-label]")].map((e) => e.getAttribute("aria-label")),
      );
      expect(labelled.length, "aria-label must survive lowering").toBeGreaterThan(0);
    });
  }
});

test.describe("streamed route", () => {
  test("the shell is usable while the slow region is still pending", async ({ page }) => {
    // Charter §14 M3 gate 3. Asserted as the browser sees it: the shell must be
    // interactive-ready before the 1200 ms region resolves, not merely present
    // in the final HTML. A buffered response would satisfy the second and fail
    // the first.
    await page.goto("/streamed", { waitUntil: "commit" });

    await expect(page.locator("#shell-marker")).toHaveText("SHELL_READY", { timeout: 1000 });
    await expect(page.locator("#pending")).toBeVisible();
    await expect(page.locator("#recs-marker")).toHaveCount(0);

    // ...and it arrives afterwards, replacing the placeholder.
    await expect(page.locator("#recs-marker")).toBeVisible({ timeout: 5000 });
    await expect(page.locator("#recs-marker")).toContainText("Affogato");
    await expect(page.locator("#pending")).toHaveCount(0);
  });

  test("ships no client JavaScript of its own", async ({ page }) => {
    const requested = [];
    page.on("response", (r) => requested.push(r.url()));
    await page.goto("/streamed");
    await expect(page.locator("#recs-marker")).toBeVisible({ timeout: 5000 });
    expect(
      requested.filter((u) => u.endsWith(".js")),
      "streaming must not require a downloaded bundle",
    ).toEqual([]);
  });

  test("the keyed list renders one item per record", async ({ page }) => {
    // `{#each items as item (item.id)}` becomes `<for|item| of=items by="id">`.
    // Marko's `by` takes a property NAME, and `by=item.id` would be an
    // undefined variable that compiles and silently re-keys every render.
    await page.goto("/streamed");
    await expect(page.locator("#recs-marker li")).toHaveCount(2, { timeout: 5000 });
    await expect(page.locator("#recs-marker li").first()).toHaveText("Affogato");
  });
});
