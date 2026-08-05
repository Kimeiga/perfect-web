// spike: browser-resumption — the two-engine driver.
//
// Runs the SAME pre-registered harness in Chrome (via the DevTools Protocol)
// and Safari (via safaridriver's W3C WebDriver endpoint). No Playwright, no
// Puppeteer, no selenium client — both protocols are plain JSON.
//
// Pre-registered decision rule (fixed BEFORE the experiment, per the architect
// ruling; see README):
//
//   all core properties pass in both engines  -> Marko is the E7 behavioural oracle
//   passes Chrome, fails Safari               -> architectural donor, not a cross-browser oracle
//   component replay / eager handler / DOM replacement / lost browser state
//                                             -> stop calling Marko resumable for our purposes;
//                                                keep only the narrower properties that passed
//   only payload scaling passes               -> report payload scaling and nothing more

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { startInstrumentedApp } from "./proxy.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CHROME =
  process.env.CHROME_PATH ||
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------------------
// Chrome, over CDP
// ---------------------------------------------------------------------------

class ChromeSession {
  #ws; #id = 0; #pending = new Map(); #sessionId = null; #proc;

  static async launch() {
    const s = new ChromeSession();
    const userDataDir = path.join(HERE, ".chrome-profile");
    s.#proc = spawn(
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
    let err = "";
    s.#proc.stderr.on("data", (d) => (err += d));
    const wsUrl = await new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error("chrome devtools port never appeared:\n" + err)), 20000);
      s.#proc.stderr.on("data", () => {
        const m = err.match(/DevTools listening on (ws:\/\/\S+)/);
        if (m) { clearTimeout(t); resolve(m[1]); }
      });
    });
    s.#ws = new WebSocket(wsUrl);
    await new Promise((res, rej) => {
      s.#ws.addEventListener("open", res, { once: true });
      s.#ws.addEventListener("error", rej, { once: true });
    });
    s.#ws.addEventListener("message", (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && s.#pending.has(msg.id)) {
        const { resolve, reject } = s.#pending.get(msg.id);
        s.#pending.delete(msg.id);
        msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
      }
    });
    const { targetId } = await s.#send("Target.createTarget", { url: "about:blank" }, false);
    const { sessionId } = await s.#send("Target.attachToTarget", { targetId, flatten: true }, false);
    s.#sessionId = sessionId;
    await s.#send("Page.enable");
    await s.#send("Runtime.enable");
    return s;
  }

  #send(method, params = {}, useSession = true) {
    const id = ++this.#id;
    const payload = { id, method, params };
    if (useSession && this.#sessionId) payload.sessionId = this.#sessionId;
    this.#ws.send(JSON.stringify(payload));
    return new Promise((resolve, reject) => this.#pending.set(id, { resolve, reject }));
  }

  async goto(url) {
    await this.#send("Page.navigate", { url });
    for (let i = 0; i < 120; i++) {
      await sleep(100);
      if ((await this.eval("typeof window.__pw === 'object'")) === true) return;
    }
    throw new Error("harness never installed on " + url);
  }

  async eval(expression) {
    const { result, exceptionDetails } = await this.#send("Runtime.evaluate", {
      expression, awaitPromise: true, returnByValue: true,
    });
    if (exceptionDetails) throw new Error(JSON.stringify(exceptionDetails).slice(0, 400));
    return result.value;
  }

  async close() {
    try { this.#ws.close(); } catch { /* gone */ }
    try { this.#proc.kill("SIGTERM"); } catch { /* gone */ }
  }
}

// ---------------------------------------------------------------------------
// Safari, over W3C WebDriver (safaridriver)
// ---------------------------------------------------------------------------

class SafariSession {
  #base; #session; #proc;

  static async launch(port = 4577) {
    const s = new SafariSession();
    s.#proc = spawn("safaridriver", ["-p", String(port)], { stdio: ["ignore", "pipe", "pipe"] });
    s.#base = `http://127.0.0.1:${port}`;

    const deadline = Date.now() + 15000;
    for (;;) {
      try {
        const r = await fetch(`${s.#base}/status`, { signal: AbortSignal.timeout(1000) });
        const j = await r.json();
        if (j?.value?.ready) break;
      } catch { /* not up yet */ }
      if (Date.now() > deadline) throw new Error("safaridriver did not become ready");
      await sleep(300);
    }

    const res = await fetch(`${s.#base}/session`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ capabilities: { alwaysMatch: { browserName: "safari" } } }),
    });
    const body = await res.json();
    if (!body?.value?.sessionId) {
      throw new Error("safaridriver refused a session: " + JSON.stringify(body).slice(0, 300));
    }
    s.#session = body.value.sessionId;
    return s;
  }

  async goto(url) {
    await fetch(`${this.#base}/session/${this.#session}/url`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ url }),
    });
    for (let i = 0; i < 120; i++) {
      await sleep(100);
      if ((await this.eval("return typeof window.__pw === 'object'")) === true) return;
    }
    throw new Error("harness never installed on " + url);
  }

  /** WebDriver executes a FUNCTION BODY, so scripts must `return`. */
  async eval(body) {
    const res = await fetch(`${this.#base}/session/${this.#session}/execute/async`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ script: body, args: [] }),
    });
    const j = await res.json();
    if (j?.value?.error) throw new Error(j.value.error + ": " + String(j.value.message).slice(0, 300));
    return j.value;
  }

  async close() {
    try {
      await fetch(`${this.#base}/session/${this.#session}`, { method: "DELETE" });
    } catch { /* gone */ }
    try { this.#proc.kill("SIGTERM"); } catch { /* gone */ }
  }
}

// ---------------------------------------------------------------------------
// The experiment
// ---------------------------------------------------------------------------

/**
 * Both engines run the same harness calls. Chrome evaluates an expression;
 * WebDriver evaluates an async function body whose last argument is a callback.
 */
function scriptFor(engine, expr) {
  if (engine === "chrome") return expr;
  // WebDriver async script: resolve the promise into the injected callback.
  return `const done = arguments[arguments.length - 1];
          Promise.resolve(${expr}).then(r => done(r), e => done({ __error: String(e) }));`;
}

async function runEngine(name, session, base) {
  const result = { engine: name, counter: null, stream: null, brokenHandler: null, error: null };
  try {
    // --- counter route: checks 1-8 ---
    await session.goto(`${base}/counter?resetHandler=1`);
    await sleep(500);
    result.counter = await session.eval(scriptFor(name, "window.__pw.runCounter()"));

    // --- streamed route: check 9 ---
    await session.goto(`${base}/stream`);
    result.stream = await session.eval(scriptFor(name, "window.__pw.runStream()"));

    // --- check 10: handler artifact fails to load ---
    await session.goto(`${base}/counter?breakHandler=1`);
    await sleep(500);
    result.brokenHandler = await session.eval(
      scriptFor(name, "window.__pw.runCounter({ breakHandler: true })"),
    );
    // Leave the proxy healthy for any later run.
    await session.goto(`${base}/static?resetHandler=1`);
  } catch (e) {
    result.error = String(e).slice(0, 500);
  }
  return result;
}

const pad = (s, n) => String(s).padEnd(n);
const mark = (v) => (v === true ? "pass" : v === false ? "FAIL" : "n/a");

const app = await startInstrumentedApp();
const results = [];

// --- Chrome ---------------------------------------------------------------
let chrome = null;
try {
  chrome = await ChromeSession.launch();
  results.push(await runEngine("chrome", chrome, app.base));
} catch (e) {
  results.push({ engine: "chrome", error: String(e).slice(0, 300) });
} finally {
  if (chrome) await chrome.close();
}

// --- Safari ---------------------------------------------------------------
// Preferred path: WebDriver. Fallback: launch Safari at an ?autorun= URL and let
// the page POST its own results back. Safari is a first-class target
// (charter §11.1), so a settings-dependent automation gate must not silently
// remove it from the evidence.
let safari = null;
let safariMode = "webdriver";
let safariBlocker = null;
try {
  safari = await SafariSession.launch();
  results.push(await runEngine("safari", safari, app.base));
} catch (e) {
  safariBlocker = String(e).slice(0, 300);
  safariMode = "autorun";
  if (safari) { try { await safari.close(); } catch { /* gone */ } safari = null; }

  const openSafari = (url) =>
    new Promise((resolve) => {
      const p = spawn("open", ["-a", "Safari", url], { stdio: "ignore" });
      p.on("exit", () => resolve());
    });

  const before = app.collected.length;
  await openSafari(`${app.base}/counter?autorun=1&engine=safari&resetHandler=1`);
  await app.waitForResults(before + 1, 25000);
  await openSafari(`${app.base}/stream?autorun=1&engine=safari`);
  await app.waitForResults(before + 2, 25000);
  await openSafari(`${app.base}/counter?autorun=1&engine=safari&breakHandler=1`);
  await app.waitForResults(before + 3, 25000);
  await openSafari(`${app.base}/static?resetHandler=1`);

  const mine = app.collected.filter((r) => r.engine === "safari");
  const counter = mine.find((r) => r.route === "counter" && !r.observed?.brokenHandler);
  const stream = mine.find((r) => r.route === "stream");
  const brokenHandler = mine.find((r) => r.route === "counter" && r.observed?.brokenHandler);
  results.push({
    engine: "safari",
    mode: "autorun (WebDriver blocked)",
    blocker: safariBlocker,
    userAgent: mine[0]?.userAgent,
    counter: counter || null,
    stream: stream || null,
    brokenHandler: brokenHandler || null,
    error: mine.length === 0 ? "Safari produced no results in autorun mode either" : null,
  });
} finally {
  if (safari) await safari.close();
}

app.stop();

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

const CHECKS = [
  ["1-html-before-js", "page usable before interaction code loads"],
  ["2-no-component-replay", "no component/template replay during startup"],
  ["3-first-click-works", "first click invokes its handler"],
  ["4-handler-lazy-until-needed", "handler artifact absent until needed"],
  ["5-only-expected-artifacts", "only the expected artifact loads"],
  ["6-dom-nodes-retained", "existing DOM nodes retained, not replaced"],
  ["7-focus-survives", "focus survives resumption"],
  ["8-second-interaction-no-reinit", "second interaction does not reinitialize"],
];
const STREAM_CHECKS = [
  ["9-both-regions-arrived", "both streamed regions arrive"],
  ["9-out-of-order-patch-lands-in-order", "out-of-order completion lands in document order"],
  ["9-placeholders-replaced", "placeholders replaced, not appended"],
];

console.log("=================== PRE-REGISTERED RESUMPTION CHECKS ================");
console.log("");
console.log(`  ${pad("check", 42)} ${pad("chrome", 8)} safari`);
console.log(`  ${"-".repeat(42)} ${"-".repeat(8)} ------`);

const get = (engine, bucket, key) => {
  const r = results.find((x) => x.engine === engine);
  return r && r[bucket] && r[bucket].checks ? r[bucket].checks[key] : undefined;
};

for (const [key, label] of CHECKS) {
  console.log(`  ${pad(label, 42)} ${pad(mark(get("chrome", "counter", key)), 8)} ${mark(get("safari", "counter", key))}`);
}
for (const [key, label] of STREAM_CHECKS) {
  console.log(`  ${pad(label, 42)} ${pad(mark(get("chrome", "stream", key)), 8)} ${mark(get("safari", "stream", key))}`);
}
console.log(
  `  ${pad("10-handler failure observable", 42)} ${pad(mark(get("chrome", "brokenHandler", "10-handler-failure-observable")), 8)} ${mark(get("safari", "brokenHandler", "10-handler-failure-observable"))}`,
);
console.log("");

for (const r of results) {
  console.log(`=================== ${r.engine.toUpperCase()} DETAIL ============================`);
  if (r.mode) console.log(`  mode: ${r.mode}`);
  if (r.userAgent) console.log(`  userAgent: ${r.userAgent}`);
  if (r.blocker) console.log(`  webdriver blocker: ${r.blocker}`);
  if (r.error) {
    console.log(`  ERROR: ${r.error}`);
    if (r.blocker) console.log(`  BLOCKER: ${r.blocker}`);
    console.log("");
    continue;
  }
  const c = r.counter || {};
  console.log("  counter route:");
  console.log(`    content at DOMContentLoaded : ${JSON.stringify(c.observed?.contentPresentAtDomReady)}`);
  console.log(`    mutations during parse      : ${c.observed?.mutationsDuringInitialParse} (browser parsing the server HTML — not replay)`);
  console.log(`    mutations AFTER parse       : ${JSON.stringify(c.observed?.mutationsAfterParse)}  <- replay would show here`);
  console.log(`    first click                 : ${JSON.stringify(c.observed?.firstClick)}`);
  console.log(`    second click                : ${JSON.stringify(c.observed?.secondClick)}`);
  console.log(`    node identity               : ${JSON.stringify(c.observed?.nodeIdentity)}`);
  console.log(`    focus                       : ${JSON.stringify(c.observed?.focus)}`);
  console.log(`    scripts loaded              : ${JSON.stringify(c.observed?.scripts)}`);
  console.log(`    earliest app script start   : ${c.observed?.earliestAppScriptStart} ms`);
  console.log(`    page errors                 : ${JSON.stringify(c.errors)}`);
  const s = r.stream || {};
  console.log("  stream route:");
  console.log(`    arrival ms                  : ${JSON.stringify(s.observed?.arrivalMs)}`);
  if (r.mode) {
    console.log("      NOTE: autorun mode starts the harness 400ms after DOMContentLoaded,");
    console.log("      so both regions have usually already landed. Arrival TIMING is not");
    console.log("      measured here; document order and placeholder replacement are.");
  }
  console.log(`    document order              : ${JSON.stringify(s.observed?.documentOrder)}`);
  console.log(`    placeholders remaining      : ${s.observed?.placeholdersRemaining}`);
  const b = r.brokenHandler || {};
  console.log("  handler-load failure:");
  console.log(`    ${JSON.stringify(b.observed?.brokenHandler)}`);
  console.log("");
}

// --- apply the pre-registered decision rule --------------------------------
const CORE = ["2-no-component-replay", "3-first-click-works", "6-dom-nodes-retained", "7-focus-survives", "8-second-interaction-no-reinit"];
const corePassed = (engine) => CORE.every((k) => get(engine, "counter", k) === true);
const chromeOk = corePassed("chrome");
const safariRan = !results.find((r) => r.engine === "safari")?.error;
const safariOk = safariRan && corePassed("safari");

console.log("=================== PRE-REGISTERED DECISION ========================");
console.log("");
console.log(`  core properties pass in chrome : ${chromeOk}`);
console.log(`  safari session established     : ${safariRan}`);
console.log(`  core properties pass in safari : ${safariRan ? safariOk : "n/a (blocked)"}`);
console.log("");
if (chromeOk && safariOk) {
  console.log("  RULING: Marko is the behavioural oracle for E7 (both engines pass).");
} else if (chromeOk && safariRan && !safariOk) {
  console.log("  RULING: Marko is an architectural donor, NOT a cross-browser oracle.");
} else if (chromeOk && !safariRan) {
  console.log("  RULING: Chrome-only evidence. Marko may not be called a cross-browser");
  console.log("          oracle until Safari is measured. Safari blocker recorded above.");
} else {
  console.log("  RULING: core resumption properties did NOT hold. Stop describing Marko");
  console.log("          as resumable for our purposes; keep only what actually passed.");
}
console.log("");
