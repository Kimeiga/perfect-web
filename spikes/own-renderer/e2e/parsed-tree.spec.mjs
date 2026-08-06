// E7 task 2 gate 4 — the browser's parsed tree matches the intended one.
//
// > The serializer can emit bytes that *look* plausible but the HTML parser
// > repairs into a different DOM.
//
// So every assertion here reads the DOM after parsing, never the source bytes.
// "The generated string contains `<tr>`" is true of a document whose rows the
// parser moved somewhere else.
//
// The pages are static files produced by `pw-render`. No server-side framework
// runs, because there is nothing to run — which is gate 1 and is asserted too.

import { test, expect } from "@playwright/test";

test.describe("gate 1 — a static page, rendered without Marko", () => {
  test("ships no script at all", async ({ page }) => {
    await page.goto("/HelloStatic.html");
    expect(await page.locator("script").count()).toBe(0);
  });

  test("is usable with JavaScript disabled", async ({ browser }) => {
    // With JS actually off rather than by counting tags: a page can ship zero
    // scripts and still depend on one, and counting would not notice.
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/HelloStatic.html");
    await expect(page.getByRole("heading", { name: "Blue Bottle" })).toBeVisible();
    await expect(page.getByRole("list", { name: "Menu" })).toBeVisible();
    await expect(page.getByText("Espresso")).toBeVisible();
    await context.close();
  });

  test("the document is in standards mode", async ({ page }) => {
    // Without a doctype the browser parses in quirks mode, where the tree and
    // the layout both differ — so every other assertion here would be about a
    // document nobody ships.
    await page.goto("/HelloStatic.html");
    expect(await page.evaluate(() => document.compatMode)).toBe("CSS1Compat");
  });

  test("an attribute value is the value, not the value in quotes", async ({ page }) => {
    // The defect this caught: the HIR keeps an attribute value AS WRITTEN,
    // delimiters included, and the first IR passed them straight through. The
    // list's accessible name came out as `"Menu"` with the quote marks in it,
    // which a screen reader announces.
    await page.goto("/HelloStatic.html");
    const label = await page.locator("ul").getAttribute("aria-label");
    expect(label).toBe("Menu");
  });
});

test.describe("gate 4 — structures the parser repairs", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/Tricky.html");
  });

  test("table rows land inside the section they were written in", async ({ page }) => {
    // A `<tr>` written outside a section gets a `<tbody>` inserted around it,
    // so a serializer that omits sections produces a different tree from the
    // one written. Read from the DOM: `thead` has one row, `tbody` has two,
    // and no row escaped to the table itself.
    const shape = await page.evaluate(() => {
      const t = document.querySelector("#t");
      return {
        thead: t.tHead?.rows.length ?? 0,
        bodies: t.tBodies.length,
        rows: t.tBodies[0]?.rows.length ?? 0,
        // Rows whose parent is the table itself would mean the parser moved
        // them, which is the failure this is looking for.
        stray: [...t.children].filter((c) => c.tagName === "TR").length,
      };
    });
    expect(shape).toEqual({ thead: 1, bodies: 1, rows: 2, stray: 0 });
  });

  test("a select contains exactly its options", async ({ page }) => {
    // The parser MOVES anything that is not an option out of a select, and the
    // page still renders. The count is what notices.
    const shape = await page.evaluate(() => {
      const s = document.querySelector("#size");
      return {
        options: s.options.length,
        values: [...s.options].map((o) => o.value),
        onlyOptions: [...s.children].every((c) => c.tagName === "OPTION"),
      };
    });
    expect(shape).toEqual({
      options: 2,
      values: ["s", "m"],
      onlyOptions: true,
    });
  });

  test("two paragraphs are two siblings", async ({ page }) => {
    const shape = await page.evaluate(() => {
      const d = document.querySelector("#paras");
      return {
        count: d.querySelectorAll("p").length,
        nested: d.querySelectorAll("p p").length,
      };
    });
    expect(shape).toEqual({ count: 2, nested: 0 });
  });

  test("a button keeps its phrasing content and its accessible name", async ({ page }) => {
    const b = page.locator("#labelled");
    await expect(b).toHaveAccessibleName("Add");
    expect(await page.evaluate(() => document.querySelector("#labelled").children.length)).toBe(1);
  });

  test("void elements produce one node and no stray closing tag", async ({ page }) => {
    // `</br>` is discarded by the parser and `<br/>`'s slash is ignored, so a
    // wrong serialization looks fine in the bytes. Counting the nodes is what
    // separates them: writing `<br></br>` yields ONE `br`, and writing
    // `<img></img>` yields one `img` whose following content moved.
    const shape = await page.evaluate(() => {
      const d = document.querySelector("#voids");
      return {
        img: d.querySelectorAll("img").length,
        br: d.querySelectorAll("br").length,
        hr: d.querySelectorAll("hr").length,
        input: d.querySelectorAll("input").length,
        // A void element cannot have children. If the serializer emitted a
        // closing tag, the parser would have reparented what followed.
        imgChildren: d.querySelector("img").childNodes.length,
        inputValue: d.querySelector("input").value,
      };
    });
    expect(shape).toEqual({
      img: 1,
      br: 1,
      hr: 1,
      input: 1,
      imgChildren: 0,
      inputValue: "tea",
    });
  });

  test("a form submits with no JavaScript", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/Tricky.html");
    const shape = await page.locator("#search").evaluate((f) => ({
      action: new URL(f.action).pathname,
      method: f.method,
      field: f.elements.q?.name,
    }));
    expect(shape).toEqual({ action: "/search", method: "get", field: "q" });
    await context.close();
  });

  test("every labelled control has its accessible name", async ({ page }) => {
    await expect(page.locator("#size")).toHaveAccessibleName("Size");
    await expect(page.locator("#q2")).toHaveAccessibleName("Search");
    await expect(page.locator("#voids img")).toHaveAccessibleName("Blue Bottle");
  });
});

test.describe("the parsed-tree checks can fail", () => {
  // Architect ruling: "DOM-equivalence test → deliberately move one child →
  // test must go red." Without this, every assertion above is satisfied by a
  // page that happens to look right for a reason unrelated to the serializer.
  test("moving one row makes the table assertion red", async ({ page }) => {
    await page.goto("/Tricky.html");

    const before = await page.evaluate(() => document.querySelector("#t").tBodies[0].rows.length);
    expect(before).toBe(2);

    // The mutation: one row is moved out of the body, which is exactly what a
    // parser repair would have done to a badly serialized table.
    await page.evaluate(() => {
      const t = document.querySelector("#t");
      t.appendChild(t.tBodies[0].rows[0]);
    });

    const after = await page.evaluate(() => {
      const t = document.querySelector("#t");
      return {
        rows: t.tBodies[0].rows.length,
        stray: [...t.children].filter((c) => c.tagName === "TR").length,
      };
    });
    expect(
      after,
      "the assertion must distinguish a moved row from a correct tree",
    ).not.toEqual({ rows: 2, stray: 0 });
  });
});
