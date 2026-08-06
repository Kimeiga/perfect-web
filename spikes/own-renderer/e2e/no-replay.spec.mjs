// E7-R — "no component replay", proved structurally and then observed.
//
// Architect ruling, 2026-08-06:
//
// > The generated browser artifact should not contain a component/template
// > renderer capable of replaying the page at all. […] Then replay is not
// > merely something the runtime promises not to do. The code necessary to
// > perform ordinary component replay is absent from the browser artifact.
//
// The same move as `Authorised` in E7V: enforce the architecture rather than
// count whether someone happened to call the wrong thing. A render counter is
// the implementation reporting on itself.
//
// The claim this file establishes is deliberately narrow:
//
// > The generated client does not contain or invoke the component/template
// > render program. Interactions operate on previously indexed document parts,
// > and browser observations show only the declared parts mutate.
//
// Not: "no computation resembling rendering ever happens."

import { test, expect } from "@playwright/test";
import { readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const DIST = fileURLToPath(new URL("../dist/", import.meta.url));

/** Everything the browser can execute on the pw route. */
async function clientClosure() {
  const names = (await readdir(DIST)).filter(
    (n) => n.endsWith(".mjs") || n.endsWith(".js") || n.endsWith(".wasm"),
  );
  const parts = [];
  for (const name of names) {
    parts.push({ name, text: await readFile(`${DIST}${name}`, "latin1") });
  }
  return parts;
}

// Names that only exist in a template/component render program. Chosen to be
// specific rather than broad: a check that cries wolf gets disabled, which is
// worse than not having one.
const RENDERER_SYMBOLS = [
  "template_ir",
  "TemplateIR",
  "render_page",
  "render_component",
  "renderTemplate",
  "Chunk::Static",
  "escape_static_attribute",
];

test.describe("the structural half", () => {
  test("the client artifact contains no template renderer", async () => {
    const closure = await clientClosure();
    expect(closure.length, "there is a client artifact to check").toBeGreaterThan(0);

    const found = [];
    for (const { name, text } of closure) {
      for (const symbol of RENDERER_SYMBOLS) {
        if (text.includes(symbol)) found.push(`${name}: ${symbol}`);
      }
    }
    expect(
      found,
      "a template renderer in the browser artifact makes replay possible, " +
        "whatever the runtime promises",
    ).toEqual([]);
  });

  test("the client does not fetch the server renderer", async ({ page }) => {
    const requested = [];
    page.on("request", (r) => requested.push(r.url()));
    await page.goto("/StorePage.html");
    await page.waitForFunction(() => document.documentElement.dataset.pwReady);

    const renderer = requested.filter((u) => /pw-render|template-ir/i.test(u));
    expect(renderer).toEqual([]);
  });

  test("the structural check can fail", async () => {
    // Architect ruling: give it a negative control. A client that DID depend on
    // the renderer must make the gate go red — otherwise the gate passes
    // because the symbol list is wrong rather than because the artifact is
    // clean.
    //
    // The mutation is the artifact a dependency would produce, applied to the
    // check rather than to the repository: writing a poisoned file into `dist`
    // would be a test that leaves the working tree broken when it fails.
    const poisoned = [
      { name: "poisoned.mjs", text: "export function render_component(ir) { return ir; }" },
    ];
    const found = [];
    for (const { name, text } of poisoned) {
      for (const symbol of RENDERER_SYMBOLS) {
        if (text.includes(symbol)) found.push(`${name}: ${symbol}`);
      }
    }
    expect(
      found.length,
      "the symbol list must detect a client that can render",
    ).toBeGreaterThan(0);
  });
});

test.describe("the behavioural half", () => {
  test("an interaction mutates only the declared part's range", async ({ page }) => {
    // A MutationObserver over the whole document, so the claim is about what
    // the browser saw rather than about what the runtime says it did.
    await page.goto("/StorePage.html");
    await page.waitForFunction(() => document.documentElement.dataset.pwReady);

    await page.evaluate(() => {
      window.__mutations = [];
      new MutationObserver((records) => {
        for (const r of records) {
          // Where, in terms a reader can check: the id of the nearest element
          // with one, or its tag.
          const target = r.target.nodeType === 1 ? r.target : r.target.parentElement;
          window.__mutations.push({
            type: r.type,
            where: target?.id || target?.tagName || "?",
            added: r.addedNodes.length,
            removed: r.removedNodes.length,
          });
        }
      }).observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        characterData: true,
      });
    });

    await page.locator("#menu button").first().click();
    await expect(page.locator("#cart-count")).toHaveText("1");
    // Let the observer drain.
    await page.waitForTimeout(100);

    const mutations = await page.evaluate(() => window.__mutations);
    expect(mutations.length, "something was observed").toBeGreaterThan(0);
    const elsewhere = mutations.filter((m) => m.where !== "cart-count");
    expect(
      elsewhere,
      "every mutation must be inside the cart's part; a replay would touch the menu",
    ).toEqual([]);
  });

  test("the mutation observation can fail", async ({ page }) => {
    // Without this, "only the cart mutated" holds for a page where nothing
    // mutated at all.
    await page.goto("/StorePage.html");
    await page.waitForFunction(() => document.documentElement.dataset.pwReady);
    await page.evaluate(() => {
      window.__mutations = [];
      new MutationObserver((records) => {
        for (const r of records) {
          const target = r.target.nodeType === 1 ? r.target : r.target.parentElement;
          window.__mutations.push({ where: target?.id || target?.tagName || "?" });
        }
      }).observe(document.body, { subtree: true, childList: true, characterData: true });
      // The mutation a replaying runtime would cause: the menu is rewritten.
      const menu = document.querySelector("#menu");
      menu.appendChild(menu.firstElementChild.cloneNode(true));
    });
    await page.waitForTimeout(100);

    const mutations = await page.evaluate(() => window.__mutations);
    const elsewhere = mutations.filter((m) => m.where !== "cart-count");
    expect(
      elsewhere.length,
      "the observer must see a mutation outside the cart when there is one",
    ).toBeGreaterThan(0);
  });
});
