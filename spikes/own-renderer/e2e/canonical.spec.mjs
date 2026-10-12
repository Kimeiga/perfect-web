// ADR-0311: a page states its canonical address, and a navigation takes the
// next page's head whole.
//   - the address is in the head, absolute with the deployment's origin
//     (here the development server's own), and Open Graph's `og:url` from
//     it; nothing of either is in the body;
//   - a soft navigation (ADR-0306) leaves nothing of the last page's head
//     but the host's charset and viewport: every `<meta>` the page stated,
//     its canonical address and its style are the next page's. Until
//     ADR-0311 only a description was taken, and the last page's Open Graph
//     properties and `theme-color`, which a browser reads live, stayed.
import { expect, test } from "@playwright/test";

test("a page's canonical address is in its head, absolute with the deployment's origin", async ({
  page,
  baseURL,
}) => {
  await page.goto("/demo/about");
  const address = `${baseURL}/demo/about`;
  await expect(page.locator('head link[rel="canonical"]')).toHaveAttribute("href", address);
  await expect(page.locator('head meta[property="og:url"]')).toHaveAttribute("content", address);
  expect(
    await page.evaluate(() => [
      document.querySelectorAll('link[rel="canonical"]').length,
      document.body.querySelectorAll('link[rel="canonical"], meta[property="og:url"]').length,
    ]),
  ).toEqual([1, 0]);
});

/** A page, ready to be pressed. */
async function ready(page, path) {
  await page.goto(path);
  await settled(page);
}

/** The runtime of the document shown, booted. */
async function settled(page) {
  await page.waitForFunction(() => document.documentElement.dataset.pwReady === "1", null, {
    polling: 50,
  });
}

/** What the document's head holds that a page or its host states. */
function head(page) {
  return page.evaluate(() => {
    const all = (selector, read) => [...document.head.querySelectorAll(selector)].map(read);
    return {
      title: document.title,
      og: all('meta[property="og:title"]', (m) => m.content),
      theme: all('meta[name="theme-color"]', (m) => m.content),
      description: all('meta[name="description"]', (m) => m.content),
      canonical: all('link[rel="canonical"]', (l) => l.getAttribute("href")),
      viewport: all('meta[name="viewport"]', (m) => m.content).length,
      charset: all("meta[charset]", (m) => m.getAttribute("charset")).length,
    };
  });
}

test("a navigation takes the next page's head whole, and leaves the host's", async ({ page }) => {
  // What each page states of itself beyond the store's own, written into
  // its document as its host writes a head: the swap is what is tested,
  // whatever the store's program states.
  const stated = {
    "/":
      '<meta property="og:title" content="Stores">\n<meta name="theme-color" content="#2b2b2b">\n' +
      '<link rel="canonical" href="http://127.0.0.1/stores">\n',
    "/cart": '<meta property="og:title" content="Your cart">\n<link rel="canonical" href="http://127.0.0.1/cart">\n',
  };
  for (const [path, extra] of Object.entries(stated)) {
    await page.route(
      (url) => new URL(url).pathname === path,
      async (route) => {
        const response = await route.fetch();
        const body = (await response.text()).replace("</head>", `${extra}</head>`);
        await route.fulfill({ response, body });
      },
    );
  }
  await ready(page, "/");
  expect(await head(page)).toMatchObject({
    og: ["Stores"],
    theme: ["#2b2b2b"],
    canonical: ["http://127.0.0.1/stores"],
  });
  await page.evaluate(() => {
    window.__kept = true;
  });
  await page.locator("#header-cart").click();
  await expect(page).toHaveURL(/\/cart$/);
  await settled(page);
  // A soft navigation, which keeps the window.
  expect(await page.evaluate(() => window.__kept === true)).toBe(true);
  expect(await head(page)).toEqual({
    title: "Your cart",
    og: ["Your cart"],
    theme: [],
    description: [],
    canonical: ["http://127.0.0.1/cart"],
    viewport: 1,
    charset: 1,
  });
});
