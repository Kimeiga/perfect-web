// ADR-0148's measurements: how each engine treats an out-of-order streamed
// patch, and when a script starts on a response that is still open.
//
//   node spikes/own-renderer/probes/streaming.mjs
//
// A Node server writes a document's shell, waits, then writes a
// `<template for>` and ends the response. Each engine is asked what its DOM
// holds before and after the patch arrives, with JavaScript off too; when
// each way of loading a script ran; and when it first painted a shell of each
// length. Chrome is the host's installed Chrome
// (`channel: "chrome"`), when there is one: Playwright's own Chromium is an
// older build without the platform's patching.
import http from "node:http";
import { chromium, firefox, webkit } from "@playwright/test";

const SHELL =
  '<!doctype html><html><head><meta charset="utf-8"><title>probe</title></head><body>\n' +
  "<p id=shell>shell</p>\n" +
  '<section><!--pw:s7--><?start name="pw-7"><p id=ph>placeholder</p><?end><!--pw:e7--></section>\n';
const PATCH = '<template for="pw-7"><p id=ready>ready</p></template>\n';
const LATER_MS = 1500;

// The ways a page can load its runtime. The runtime was loaded as a deferred
// module until ADR-0148.
const LOADERS = {
  "deferred module": '<script type="module" src="/m.mjs"></script>',
  "async module": '<script type="module" async src="/m.mjs"></script>',
  "async classic": '<script async src="/c.js"></script>',
  "inline import()": '<script>import("/m.mjs")</script>',
};

const server = http.createServer((req, res) => {
  const mark = "window.ranAt = window.ranAt ?? performance.now();";
  if (req.url === "/m.mjs" || req.url === "/c.js") {
    res.writeHead(200, { "content-type": "text/javascript" });
    res.end(mark);
    return;
  }
  const url = new URL(req.url, "http://probe");
  // A shell this many characters of text longer: when an engine paints.
  const words = "word ".repeat(Number(url.searchParams.get("chars") ?? 0) / 5);
  const loader = LOADERS[decodeURIComponent(url.pathname.slice(1))] ?? "";
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.write(SHELL + `<p>${words}</p>` + loader + "\n");
  setTimeout(() => res.end(PATCH + "</body></html>\n"), LATER_MS);
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}/`;

/** The body's nodes, as kinds: an element, a comment, a processing
 * instruction, a template's contents. */
function shape() {
  const out = [];
  const walk = (n) => {
    for (const c of n.childNodes) {
      if (c.nodeType === 1) {
        out.push(c.id ? `<${c.localName}#${c.id}>` : `<${c.localName}>`);
        walk(c.nodeName === "TEMPLATE" ? c.content : c);
      } else if (c.nodeType === 7) out.push(`<?${c.target} ${c.data}?>`);
      else if (c.nodeType === 8) out.push(`<!--${c.data}-->`);
    }
  };
  walk(document.body);
  return out.join(" ");
}

const engines = [
  ["chrome (installed)", chromium, { channel: "chrome" }],
  ["chromium (playwright)", chromium, {}],
  ["firefox (playwright)", firefox, {}],
  ["webkit (playwright)", webkit, {}],
];
for (const [name, type, options] of engines) {
  let browser;
  try {
    browser = await type.launch(options);
  } catch (e) {
    console.log(`${name}: not available here (${String(e.message).split("\n")[0]})`);
    continue;
  }
  console.log(`${name} ${browser.version()}`);

  const page = await browser.newPage();
  await page.goto(base + "none", { waitUntil: "commit" });
  await page.waitForSelector("#shell");
  console.log(`  before the patch:  ${await page.evaluate(shape)}`);
  await page.waitForLoadState("load");
  console.log(`  after the patch:   ${await page.evaluate(shape)}`);
  await page.close();

  const off = await browser.newContext({ javaScriptEnabled: false });
  const still = await off.newPage();
  await still.goto(base + "none", { waitUntil: "load" });
  const html = await still.content();
  const applied = html.includes('id="ready"') && !html.includes("<template");
  console.log(`  JavaScript off:    ${applied ? "patched" : "the placeholder stays"}`);
  await off.close();

  for (const loader of Object.keys(LOADERS)) {
    const p = await browser.newPage();
    await p.goto(base + encodeURIComponent(loader), { waitUntil: "commit" });
    await p.waitForSelector("#shell");
    // Half the wait: the response is still open.
    await p.waitForTimeout(LATER_MS / 2);
    const early = await p.evaluate(() => window.ranAt ?? null);
    await p.waitForLoadState("load");
    const late = await p.evaluate(() => window.ranAt ?? null);
    const when = early !== null ? "while the response is open" : "only after it ends";
    console.log(`  ${loader.padEnd(17)} ran ${when} (at ${Math.round(late)} ms)`);
    await p.close();
  }

  // When it first paints a shell of 0 to 1000 characters of text, while the
  // response is still open or only once it has ended.
  for (const chars of [0, 200, 300, 1000]) {
    const p = await browser.newPage();
    await p.goto(`${base}none?chars=${chars}`, { waitUntil: "commit" });
    await p.waitForTimeout(LATER_MS / 2);
    const early = await p.evaluate(() => performance.getEntriesByType("paint").length > 0);
    await p.waitForLoadState("load");
    const at = await p.evaluate(
      () => performance.getEntriesByName("first-contentful-paint")[0]?.startTime,
    );
    const when = early ? "while the response is open" : "only after it ends";
    console.log(`  ${String(chars).padStart(4)} characters   painted ${when} (at ${Math.round(at)} ms)`);
    await p.close();
  }
  await browser.close();
}
server.close();
