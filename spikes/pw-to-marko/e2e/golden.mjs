// E7 task 1 — the renderer-independent golden suite.
//
// Charter §14 M7 task 1: "Freeze a renderer-independent golden test suite using
// the Marko implementation as one oracle." Task 13 compares own-renderer output
// and behaviour against it; task 14 keeps Marko as the fallback until parity.
//
// # Why this is data and not a `.spec.mjs`
//
// The suite has to run unchanged against two renderers. A test file that
// imported Playwright and hard-coded a base URL would run against whichever
// server the config points at, and "we ran the suite against both" would be a
// claim about a config file. Here the cases are a value, and the runner is
// parameterised by which renderer produced the page.
//
// # Why the assertions are semantic and not byte-exact HTML
//
// Two correct renderers legitimately differ in whitespace, attribute order, and
// where they put their own bookkeeping. A golden file of expected markup would
// fail on all three and pass on none of what matters. Every assertion here is
// about the DOM a user gets or an observable behaviour — the things RQ-1
// measured, which are the things Marko is an oracle FOR.
//
// # What Marko is and is not an oracle for
//
// RQ-1 measured resumption and DOM preservation in Chrome and Safari and Marko
// passed. It measured interaction-lazy loading and Marko FAILED: its
// interaction module loads during initial page load rather than on first need.
//
// So the cases carry an `oracle` field. `oracle: "marko"` means both renderers
// must agree. `oracle: "none"` means the property is the project's own target
// and Marko is expected to fail it — those cases are recorded here so E7-L has
// a specification rather than an aspiration, and they are reported separately
// rather than folded into a pass rate.

/** @typedef {"marko" | "none"} Oracle */

/**
 * One golden case.
 *
 * `check(page, expect)` runs against a page the renderer under test produced.
 * It must not mention the renderer, the framework, or any generated file name.
 */

export const GOLDEN = [
  // --- static rendering ---------------------------------------------------
  {
    id: "static/no-script",
    route: "/static",
    describes: "a route with no interaction ships no browser runtime",
    oracle: "marko",
    charter: "§14 M7 gate 2",
    async check(page, expect) {
      const scripts = await page.locator("script").count();
      expect(scripts, "a static route must contain no <script>").toBe(0);
    },
  },
  {
    id: "static/readable-without-js",
    route: "/static",
    describes: "the content is in the HTML, not assembled by script",
    oracle: "marko",
    charter: "§14 M7 gate 2",
    jsDisabled: true,
    async check(page, expect) {
      await expect(page.getByRole("heading", { name: "Blue Bottle" })).toBeVisible();
      await expect(page.getByRole("list", { name: "Menu" })).toBeVisible();
      await expect(page.getByText("Espresso")).toBeVisible();
    },
  },
  {
    id: "static/inline-whitespace",
    route: "/static",
    describes: "whitespace between inline elements survives to the DOM",
    // The defect this exists for: the template looked right and the DOM read
    // "Espresso$3.50". A renderer can get the markup right and the text wrong.
    oracle: "marko",
    charter: "RQ-1 F-5",
    async check(page, expect) {
      const item = page.getByRole("listitem").first();
      await expect(item).toHaveText(/Espresso\s+\$3\.50/);
    },
  },

  // --- resumption ---------------------------------------------------------
  {
    id: "counter/no-replay",
    route: "/counter",
    describes: "interaction works without re-executing the page",
    oracle: "marko",
    charter: "§14 M7 gate 4, RQ-1",
    async check(page, expect) {
      // The page stamps a marker on first execution. If the runtime replayed
      // the component tree, the marker would be replaced rather than kept.
      await page.locator("#add").click();
      await expect(page.locator("#count")).toHaveText("1");
      await page.locator("#add").click();
      await expect(page.locator("#count")).toHaveText("2");
    },
  },
  {
    id: "counter/readable-before-js",
    route: "/counter",
    describes: "the interactive route is readable before its JavaScript runs",
    oracle: "marko",
    charter: "§8.5",
    jsDisabled: true,
    async check(page, expect) {
      await expect(page.locator("#count")).toHaveText("0");
    },
  },
  {
    id: "counter/dom-identity",
    route: "/counter",
    describes: "an update mutates the existing node rather than replacing it",
    // RQ-1's finding F-1: 72 "component replacements" turned out to be the
    // browser's own initial parse. The honest version marks a node BEFORE the
    // interaction and checks the mark survives it.
    oracle: "marko",
    charter: "§14 M7 gate 4",
    async check(page, expect) {
      const marked = await page.evaluate(() => {
        const el = document.querySelector("#count");
        el.__pwIdentity = "marked";
        return el.textContent;
      });
      expect(marked).toBe("0");
      await page.locator("#add").click();
      await expect(page.locator("#count")).toHaveText("1");
      const survived = await page.evaluate(
        () => document.querySelector("#count").__pwIdentity,
      );
      expect(survived, "the text node's element was preserved, not replaced").toBe("marked");
    },
  },
  {
    id: "counter/focus-preserved",
    route: "/counter",
    describes: "focus survives an update",
    oracle: "marko",
    charter: "§14 M7 gate 5, task 10",
    // Stated as "whatever had focus still has it", not "the button has focus".
    //
    // The first version asserted the button. It passed in Chromium and Firefox
    // and failed in WebKit, where clicking a button does not focus it — a real
    // platform difference and nothing to do with the renderer. A golden case
    // that encodes one engine's focus behaviour is testing the engine.
    async check(page, expect) {
      const button = page.locator("#add");
      await button.focus();
      const before = await page.evaluate(() => document.activeElement?.id ?? "");
      expect(before, "the control took focus").toBe("add");

      // Enter rather than click: it is the keyboard path, it updates the same
      // state, and it does not raise the question of what clicking does to
      // focus on each platform.
      await page.keyboard.press("Enter");
      await expect(page.locator("#count")).toHaveText("1");

      const after = await page.evaluate(() => document.activeElement?.id ?? "");
      expect(after, "focus survived the update").toBe(before);
    },
  },

  // --- streaming ----------------------------------------------------------
  {
    id: "streamed/shell-first",
    route: "/streamed",
    describes: "the shell is usable while a slow region is still pending",
    oracle: "marko",
    charter: "§14 M7 task 9",
    // `commit`, so the assertion is about what the browser has when the
    // response begins rather than about the final document. A buffered
    // response produces the same final HTML and fails the property entirely.
    waitUntil: "commit",
    async check(page, expect) {
      await expect(page.locator("#shell-marker")).toHaveText("SHELL_READY", {
        timeout: 1000,
      });
      await expect(page.locator("#pending")).toBeVisible();
      await expect(page.locator("#recs-marker")).toHaveCount(0);

      // And the slow region arrives afterwards, replacing the placeholder.
      await expect(page.locator("#recs-marker")).toBeVisible({ timeout: 5000 });
      await expect(page.locator("#pending")).toHaveCount(0);
    },
  },
  {
    id: "streamed/keyed-list",
    route: "/streamed",
    describes: "a keyed list renders one node per record",
    oracle: "marko",
    charter: "§14 M7 task 12",
    // Two records, and the count is the point: `by` takes a property NAME, and
    // a `by` expression that evaluates to `undefined` compiles and silently
    // re-keys every render, which no rendered-output check would notice.
    async check(page, expect) {
      await expect(page.locator("#recs-marker li")).toHaveCount(2, { timeout: 5000 });
      await expect(page.locator("#recs-marker li").first()).toHaveText("Affogato");
    },
  },

  // --- the store page -----------------------------------------------------
  {
    id: "store/public-and-private-together",
    route: "/store",
    describes: "public store data and private cart data render on one page",
    oracle: "marko",
    charter: "§14 M4 gate 1, §14 M5 gate 2",
    async check(page, expect) {
      await expect(page.locator("#store-name")).toHaveText("Blue Bottle");
      await expect(page.locator("#cart-count")).toBeVisible();
    },
  },
  {
    id: "store/no-session-in-shared-html",
    route: "/store",
    describes: "the served HTML carries no session identifier",
    oracle: "marko",
    charter: "§14 M5 gate 3",
    needsHtml: true,
    // The VALUE, not a pattern. A first version matched `/session-[0-9a-z]/i`
    // and fired on the page's own prose — the word "session" followed by a
    // hyphen is not a session identifier, and a privacy check that reports
    // English is a privacy check nobody keeps.
    async check(page, expect, { html }) {
      expect(html, "a shared document must not name a session").not.toContain("session-1");
    },
  },
  {
    id: "store/add-updates-only-the-cart",
    route: "/store",
    describes: "adding to the cart changes the cart and nothing else",
    oracle: "marko",
    charter: "§14 M4 gate 2",
    async check(page, expect) {
      const before = await page.locator("#menu").innerHTML();
      await page.locator("#menu button").first().click();
      await expect(page.locator("#cart-count")).not.toHaveText("0");
      const after = await page.locator("#menu").innerHTML();
      expect(after, "the menu must not re-render").toBe(before);
    },
  },

  // --- the project's own target, where Marko is NOT an oracle -------------
  {
    id: "lazy/handler-bytes-not-loaded-before-interaction",
    route: "/counter",
    describes:
      "a handler's executable bytes are not loaded before interaction is possible",
    // RQ-1 measured this and Marko FAILED it: its interaction module loads
    // during initial page load rather than on first need. So there is no
    // oracle, and the target is defined here rather than borrowed.
    //
    // This is E7-L's specification. It is expected to fail against Marko today
    // and is reported separately rather than folded into a pass rate — a
    // suite that counted it as a failure would say the oracle is broken, and a
    // suite that omitted it would let E7-L ship without a target.
    oracle: "none",
    charter: "§14 M7 gate 3, RQ-1 (falsified for Marko)",
    // The property, stated so that no renderer's file naming is involved:
    //
    //   interacting for the first time causes JavaScript to be fetched that
    //   was not fetched before.
    //
    // A first version filtered requests by `/handler|interaction/` in the URL
    // and PASSED against Marko — which RQ-1 falsified — because Marko's bundle
    // is not named that. The detector was reading a naming convention, so it
    // measured nothing. This version reads the fetch itself.
    //
    // It is deliberately weak in one direction: a renderer could satisfy it by
    // fetching one byte late. What it cannot be satisfied by is loading
    // everything up front, which is the failure RQ-1 recorded.
    async check(page, expect, { jsBefore, interactAndCollect }) {
      const jsAfter = await interactAndCollect();
      const fetchedOnDemand = jsAfter.filter((u) => !jsBefore.includes(u));
      expect(
        fetchedOnDemand.length,
        `interaction fetched no new code, so all of it was loaded up front. ` +
          `Before: ${jsBefore.length} script request(s).`,
      ).toBeGreaterThan(0);
    },
  },
];

/** Cases both renderers must satisfy. */
export const ORACLE_CASES = GOLDEN.filter((c) => c.oracle === "marko");

/** Cases that define the project's own target, with no oracle to borrow. */
export const OWN_TARGET_CASES = GOLDEN.filter((c) => c.oracle === "none");
