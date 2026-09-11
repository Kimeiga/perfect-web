// Layout-phase spike measurement harness. Uses Node 22's CDP client primitives.
// Charter section 14 requires a reproducible, falsifiable layout baseline.
// Missing LoAF observations are not a measurement of zero forced layout.
// Usage: CHROME_PATH=/path/to/chrome node measure.mjs

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { summarizeLayoutAttribution } from "./public/loaf.js";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PUBLIC = path.join(HERE, "public");
const PORT = Number(process.env.PORT || 8099);
const CHROME = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const med = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
const r1 = (v) => Number(v.toFixed(1));
const r2 = (v) => Number(v.toFixed(2));

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

const chrome = spawn(CHROME, [
  "--headless=new", "--remote-debugging-port=0",
  `--user-data-dir=${path.join(HERE, ".chrome-profile")}`,
  "--no-first-run", "--no-default-browser-check", "--disable-extensions",
  "--disable-background-timer-throttling", "--disable-renderer-backgrounding",
  "--window-size=1280,900", "about:blank",
], { stdio: ["ignore", "pipe", "pipe"] });
let chromeErr = "";
chrome.stderr.on("data", (d) => (chromeErr += d));
const cleanup = () => {
  try { chrome.kill("SIGTERM"); } catch { /* already gone */ }
  try { server.close(); } catch { /* already gone */ }
};
process.on("exit", cleanup);
process.on("SIGINT", () => { cleanup(); process.exit(130); });

// Register cleanup before awaiting startup, including spawn/timeout failures.
let wsUrl;
try {
  wsUrl = await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("chrome did not report a devtools port:\n" + chromeErr)), 20000);
    chrome.once("error", (error) => { clearTimeout(t); reject(error); });
    chrome.stderr.on("data", () => {
      const m = chromeErr.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) { clearTimeout(t); resolve(m[1]); }
    });
  });
} catch (error) {
  cleanup();
  throw error;
}

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
    for (let i = 0; i < 100; i++) {
      await new Promise((r) => setTimeout(r, 100));
      if (await this.eval("typeof window.__run === 'function'") === true) return;
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

try {
  const cdp = await CDP.connect(wsUrl);
  await cdp.attachToNewTab("about:blank");
  const base = `http://127.0.0.1:${PORT}`;
  const out = {};

  async function runMode(page, n, iterations = 7) {
    await cdp.goto(`${base}/${page}?n=${n}`);
    const result = await cdp.eval(`(async () => {
      if (window.__loafObservation === 'failed') throw new Error(window.__loafError);
      const doFrame = () => new Promise(resolve => requestAnimationFrame(() => resolve(window.__run())));
      const samples = [];
      window.__loaf.length = 0;
      for (let i = 0; i < ${iterations}; i++) samples.push(await doFrame());
      await new Promise(resolve => setTimeout(resolve, 400));
      return {
        runs: samples.map(sample => sample.ms),
        checksums: samples.map(sample => sample.checksum),
        checksum: samples.at(-1).checksum,
        loafSupported: window.__loafSupported,
        loafObservation: window.__loafObservation,
        loafEntries: window.__loaf.length,
        loaf: window.__loaf,
      };
    })()`);
    return { ...result, attribution: summarizeLayoutAttribution(result.loaf) };
  }

  console.log("=========== 1. LAYOUT: THRASH vs PHASED ============================");
  console.log(`  ${"elements".padEnd(10)} ${"mode".padEnd(8)} ${"median ms".padEnd(11)} ${"runs".padEnd(34)} checksum`);
  for (const n of [400, 1200]) {
    out[n] = {};
    for (const [mode, page] of [["thrash", "thrash.html"], ["phased", "phased.html"]]) {
      const r = await runMode(page, n);
      out[n][mode] = r;
      console.log(`  ${String(n).padEnd(10)} ${mode.padEnd(8)} ${String(r1(med(r.runs))).padEnd(11)} ` +
        `${JSON.stringify(r.runs.map(r1)).padEnd(34)} ${r.checksum}`);
    }
    const ratio = med(out[n].thrash.runs) / med(out[n].phased.runs);
    const sameWork = out[n].thrash.checksums.every((value, index) => value === out[n].phased.checksums[index]);
    if (!sameWork) throw new Error(`workload checksums disagree at n=${n}; no speedup claim is valid`);
    console.log(`  phase scheduling: ${r1(ratio)}x faster in this run; per-sample checksums agree`);
  }

  console.log("=========== 2. LONG ANIMATION FRAME SCRIPT ATTRIBUTION =============");
  for (const n of [400, 1200]) {
    for (const mode of ["thrash", "phased"]) {
      const r = out[n][mode];
      console.log(`  n=${n} ${mode} observer=${r.loafObservation} longFrames=${r.loafEntries}`);
      console.log(`  ${JSON.stringify(r.attribution)}`);
    }
  }
  const positive = out[1200].thrash.attribution.reportedForcedStyleAndLayoutDuration;
  console.log(`  check:forced-layout-positive-control=${positive !== null && positive > 0 ? "pass" : "inconclusive"}`);
  console.log("  Durations cover reported script records in observed long frames, not all page layout.");
  console.log("  No records or missing attribution cannot establish zero layout or lack of browser support.");

  console.log("=========== 3. RESIZE OBSERVER + FEEDBACK LOOP ======================");
  await cdp.goto(`${base}/observers.html?n=3000`);
  const obs = await cdp.eval("window.__run()");
  console.log(`  resizeObserver deliveries: ${obs.resizeDeliveries}`);
  console.log(`  feedback-loop iterations: ${obs.feedbackIterations}`);
  console.log(`  browser reported the RO loop: ${obs.feedbackLoopErrorSeen}`);
  console.log(`  check:feedback-loop-detectable=${obs.feedbackLoopErrorSeen ? "pass" : "fail"}`);

  console.log("=========== 4. CSS CONTAINMENT / content-visibility =================");
  const cont = await cdp.eval(`(async () => {
    const N = 3000;
    const host = document.getElementById("subtree");
    async function trial(contained) {
      host.innerHTML = "";
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
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
    return { plain, contained, supported: CSS.supports("contain", "layout style paint"),
      cvSupported: CSS.supports("content-visibility", "auto") };
  })()`);
  const pm = med(cont.plain), cm = med(cont.contained);
  console.log(`  supported: contain=${cont.supported} content-visibility=${cont.cvSupported}`);
  console.log(`  build + first layout of 3000 rows:`);
  console.log(`    plain median ${r2(pm)} ms ${JSON.stringify(cont.plain.map(r1))}`);
  console.log(`    contained median ${r2(cm)} ms ${JSON.stringify(cont.contained.map(r1))}`);
  console.log(`    ${r2(pm / cm)}x faster with contain + content-visibility in this run`);
  console.log(`  check:containment-helps-initial-layout=${pm / cm > 1.5 ? "pass" : "fail"}`);
  console.log("  Containment here measures initial layout, not every later layout or semantic equivalence.");
} finally {
  cleanup();
}
