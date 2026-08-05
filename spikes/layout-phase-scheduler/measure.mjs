// spike: layout-phase-scheduler — the measurement harness.
//
// Charter §14 Milestone 0 task 13 requires a "reproducible baseline proving the
// project can detect forced synchronous layout before attempting to prevent it".
// "Reproducible" means scriptable, so this drives headless Chrome over the
// DevTools Protocol directly — using Node 22's built-in WebSocket and fetch, with
// no Playwright or Puppeteer dependency.
//
// Measures:
//   1. thrash vs phase-scheduled, at two element counts
//   2. Long Animation Frame attribution (and whether the browser exposes
//      forcedStyleAndLayoutDuration at all)
//   3. ResizeObserver delivery + the feedback-loop error
//   4. CSS containment / content-visibility on a large subtree
//
// Usage: node measure.mjs [--chrome <path>]

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC = path.join(HERE, "public");
const PORT = Number(process.env.PORT || 8099);
const CHROME =
  process.env.CHROME_PATH ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const med = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
const r1 = (v) => Number(v.toFixed(1));
const r2 = (v) => Number(v.toFixed(2));

// --- static server ----------------------------------------------------------

const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript" };
const server = createServer(async (req, res) => {
  const name = (req.url || "/").split("?")[0].replace(/^\//, "") || "index.html";
  try {
    const body = await readFile(path.join(PUBLIC, name));
    res.writeHead(200, { "content-type": MIME[path.extname(name)] || "text/plain" });
    res.end(body);
  } catch {
    res.writeHead(404).end("not found");
  }
});
await new Promise((r) => server.listen(PORT, "127.0.0.1", r));

// --- headless chrome over CDP ------------------------------------------------

const userDataDir = path.join(HERE, ".chrome-profile");
const chrome = spawn(
  CHROME,
  [
    "--headless=new",
    "--remote-debugging-port=0",
    `--user-data-dir=${userDataDir}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--window-size=1280,900",
    "about:blank",
  ],
  { stdio: ["ignore", "pipe", "pipe"] },
);

let chromeErr = "";
chrome.stderr.on("data", (d) => (chromeErr += d));

/** Chrome prints the actual devtools port to stderr on startup. */
const wsUrl = await new Promise((resolve, reject) => {
  const t = setTimeout(() => reject(new Error("chrome did not report a devtools port:\n" + chromeErr)), 20000);
  chrome.stderr.on("data", () => {
    const m = chromeErr.match(/DevTools listening on (ws:\/\/\S+)/);
    if (m) {
      clearTimeout(t);
      resolve(m[1]);
    }
  });
});

const cleanup = () => {
  try { chrome.kill("SIGTERM"); } catch { /* gone */ }
  try { server.close(); } catch { /* gone */ }
};
process.on("exit", cleanup);
process.on("SIGINT", () => { cleanup(); process.exit(130); });

/** Minimal CDP client over the browser-level endpoint. */
class CDP {
  #ws; #id = 0; #pending = new Map(); #sessionId = null;
  static async connect(url) {
    const c = new CDP();
    c.#ws = new WebSocket(url);
    await new Promise((res, rej) => {
      c.#ws.addEventListener("open", res, { once: true });
      c.#ws.addEventListener("error", rej, { once: true });
    });
    c.#ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && c.#pending.has(msg.id)) {
        const { resolve, reject } = c.#pending.get(msg.id);
        c.#pending.delete(msg.id);
        msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
      }
    });
    return c;
  }
  send(method, params = {}, useSession = true) {
    const id = ++this.#id;
    const payload = { id, method, params };
    if (useSession && this.#sessionId) payload.sessionId = this.#sessionId;
    this.#ws.send(JSON.stringify(payload));
    return new Promise((resolve, reject) => this.#pending.set(id, { resolve, reject }));
  }
  async attachToNewTab(url) {
    const { targetId } = await this.send("Target.createTarget", { url }, false);
    const { sessionId } = await this.send("Target.attachToTarget", { targetId, flatten: true }, false);
    this.#sessionId = sessionId;
    await this.send("Page.enable");
    await this.send("Runtime.enable");
  }
  async goto(url) {
    await this.send("Page.navigate", { url });
    // Wait for the module script to define window.__run.
    for (let i = 0; i < 100; i++) {
      await new Promise((r) => setTimeout(r, 100));
      const ok = await this.eval("typeof window.__run === 'function'");
      if (ok === true) return;
    }
    throw new Error("page never defined window.__run: " + url);
  }
  async eval(expression) {
    const { result, exceptionDetails } = await this.send("Runtime.evaluate", {
      expression, awaitPromise: true, returnByValue: true,
    });
    if (exceptionDetails) throw new Error(JSON.stringify(exceptionDetails));
    return result.value;
  }
}

const cdp = await CDP.connect(wsUrl);
await cdp.attachToNewTab("about:blank");

const base = `http://127.0.0.1:${PORT}`;
const out = {};

// --- 1 + 2. thrash vs phased, with LoAF -------------------------------------

async function runMode(page, n, iterations = 7) {
  await cdp.goto(`${base}/${page}?n=${n}`);
  const res = await cdp.eval(`(async () => {
    const doFrame = () => new Promise(res => requestAnimationFrame(() => res(window.__run().ms)));
    const runs = [];
    for (let i = 0; i < ${iterations}; i++) runs.push(await doFrame());
    await new Promise(r => setTimeout(r, 400));
    return {
      runs,
      checksum: window.__run().checksum,
      loafSupported: window.__loafSupported,
      loafEntries: window.__loaf.length,
      forcedAttrExposed: window.__loaf.length
        ? window.__loaf[0].forcedStyleAndLayoutDuration !== undefined : null,
      loaf: window.__loaf.slice(-3),
    };
  })()`);
  return res;
}

console.log("=========== 1. FORCED SYNCHRONOUS LAYOUT: THRASH vs PHASED ==========");
console.log("");
console.log(`  ${"elements".padEnd(10)} ${"mode".padEnd(8)} ${"median ms".padEnd(11)} ${"runs".padEnd(34)} checksum`);
for (const n of [400, 1200]) {
  out[n] = {};
  for (const [mode, page] of [["thrash", "thrash.html"], ["phased", "phased.html"]]) {
    const r = await runMode(page, n);
    out[n][mode] = r;
    console.log(
      `  ${String(n).padEnd(10)} ${mode.padEnd(8)} ${String(r1(med(r.runs))).padEnd(11)} ` +
        `${JSON.stringify(r.runs.map(r1)).padEnd(34)} ${r.checksum}`,
    );
  }
  const ratio = med(out[n].thrash.runs) / med(out[n].phased.runs);
  const sameWork = out[n].thrash.checksum === out[n].phased.checksum;
  console.log(`  ${"".padEnd(10)} -> phase scheduling is ${r1(ratio)}x faster; identical checksum: ${sameWork}`);
  console.log("");
}

console.log("=========== 2. LONG ANIMATION FRAME ATTRIBUTION =====================");
console.log("");
for (const n of [400, 1200]) {
  for (const mode of ["thrash", "phased"]) {
    const r = out[n][mode];
    console.log(
      `  n=${String(n).padEnd(5)} ${mode.padEnd(7)} loafSupported=${r.loafSupported} ` +
        `longFrames=${r.loafEntries}` +
        (r.loafEntries
          ? `  duration=${r1(r.loaf.at(-1).duration)}ms blocking=${r1(r.loaf.at(-1).blockingDuration)}ms`
          : "  (no frame exceeded the 50ms LoAF threshold)"),
    );
  }
}
const exposed = out[1200].thrash.forcedAttrExposed;
console.log("");
console.log(`  forcedStyleAndLayoutDuration exposed by this browser: ${exposed}`);
if (exposed === false) {
  console.log("  -> the charter's preferred signal is UNAVAILABLE here.");
  console.log("     Long-frame COUNT plus blockingDuration is the usable proxy:");
  console.log(`     thrash produced ${out[1200].thrash.loafEntries} long frames, phased produced ${out[1200].phased.loafEntries}.`);
}
console.log("");

// --- 3 + 4. observers and containment ---------------------------------------

console.log("=========== 3. RESIZE OBSERVER + FEEDBACK LOOP ======================");
console.log("");
await cdp.goto(`${base}/observers.html?n=3000`);
const obs = await cdp.eval("window.__run()");
console.log(`  resizeObserver deliveries      : ${obs.resizeDeliveries}`);
console.log(`  feedback-loop iterations       : ${obs.feedbackIterations}`);
console.log(`  browser reported the RO loop   : ${obs.feedbackLoopErrorSeen}`);
console.log(`  check:feedback-loop-detectable=${obs.feedbackLoopErrorSeen ? "pass" : "fail"}`);
console.log("");

console.log("=========== 4. CSS CONTAINMENT / content-visibility =================");
console.log("");
const cont = await cdp.eval(`(async () => {
  const N = 3000;
  const host = document.getElementById("subtree");
  async function trial(contained) {
    host.innerHTML = "";
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    const t0 = performance.now();
    const frag = document.createDocumentFragment();
    for (let i = 0; i < N; i++) {
      const d = document.createElement("div");
      d.className = "row" + (contained ? " contained" : "");
      d.textContent = "row " + i + " — some text that requires intrinsic sizing and font metrics";
      frag.appendChild(d);
    }
    host.appendChild(frag);
    host.offsetHeight;
    return performance.now() - t0;
  }
  const plain = [], contained = [];
  for (let i = 0; i < 5; i++) { plain.push(await trial(false)); contained.push(await trial(true)); }
  return { plain, contained,
           supported: CSS.supports("contain","layout style paint"),
           cvSupported: CSS.supports("content-visibility","auto") };
})()`);
const pm = med(cont.plain), cm = med(cont.contained);
console.log(`  supported: contain=${cont.supported}  content-visibility=${cont.cvSupported}`);
console.log(`  build + first layout of 3000 rows:`);
console.log(`    plain     median ${r2(pm)} ms   ${JSON.stringify(cont.plain.map(r1))}`);
console.log(`    contained median ${r2(cm)} ms   ${JSON.stringify(cont.contained.map(r1))}`);
console.log(`    -> ${r2(pm / cm)}x faster with contain + content-visibility`);
console.log(`  check:containment-helps-initial-layout=${pm / cm > 1.5 ? "pass" : "fail"}`);
console.log("");
console.log("  CAVEAT (measured): containment does NOT help a forced layout that");
console.log("  follows an unrelated mutation on the host element. An earlier");
console.log("  version of this harness measured that case and found the contained");
console.log("  subtree marginally SLOWER. Containment helps building and laying out");
console.log("  a large independent subtree, which is a different thing.");
console.log("");

cleanup();
