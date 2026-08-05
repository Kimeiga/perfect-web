// spike: marko-stream-resume — the measurement harness.
//
// Charter §14 Milestone 0 task 9 requires *measured* answers, and §18.5 requires
// every measurement to record how it was produced. This script starts the built
// server, fetches each route, and reports:
//
//   - HTML bytes (uncompressed and gzipped)
//   - number of <script> tags and the client JS actually referenced
//   - client JS bytes (uncompressed and gzipped)
//   - streaming arrival times of each marker, relative to the first byte
//
// Exit code 0 always: this is a measurement, and the gate decision is made from
// the printed numbers by a human reading docs/milestones/M0.md.

import { spawn } from "node:child_process";
import { gzipSync } from "node:zlib";
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";

const PORT = Number(process.env.PORT || 3111);
const BASE = `http://127.0.0.1:${PORT}`;
const ROOT = path.dirname(new URL(import.meta.url).pathname);

const bytes = (s) => Buffer.byteLength(s, "utf8");
const gz = (s) => gzipSync(Buffer.from(s, "utf8")).length;
const pad = (s, n) => String(s).padEnd(n);

async function waitForServer(timeoutMs = 15000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(`${BASE}/static`, { signal: AbortSignal.timeout(1000) });
      if (r.ok) return true;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  return false;
}

/** Fetch a route and time when each marker string arrives in the stream. */
async function streamProfile(route, markers) {
  const t0 = performance.now();
  const res = await fetch(`${BASE}${route}`);
  const ttfbAt = performance.now() - t0;

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let html = "";
  let firstChunkAt = null;
  const arrivals = Object.create(null);
  const chunkSizes = [];

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    const now = performance.now() - t0;
    if (firstChunkAt === null) firstChunkAt = now;
    chunkSizes.push(value.byteLength);
    html += decoder.decode(value, { stream: true });
    for (const m of markers) {
      if (arrivals[m] === undefined && html.includes(m)) arrivals[m] = now;
    }
  }
  html += decoder.decode();

  return {
    html,
    status: res.status,
    contentType: res.headers.get("content-type"),
    transferEncoding: res.headers.get("transfer-encoding"),
    ttfbAt,
    firstChunkAt,
    totalAt: performance.now() - t0,
    chunks: chunkSizes.length,
    arrivals,
  };
}

/**
 * Resolve every <script src> the HTML references AND everything those modules
 * import, transitively.
 *
 * A naive version of this function counted only the entry chunk and reported
 * /counter as 176 bytes of client JS. The entry actually
 * `import`s a shared 3,745-byte runtime chunk. Measuring only the entry
 * under-reports the real cost by 20x, so the graph must be walked.
 */
function clientJs(html) {
  const srcs = [...html.matchAll(/<script[^>]*\bsrc="?([^"\s>]+)"?/g)].map((m) => m[1]);
  const seen = new Set();
  const files = [];
  let raw = 0;
  let gzipped = 0;

  const visit = (src, entry) => {
    if (seen.has(src)) return;
    seen.add(src);
    const file = path.join(ROOT, "dist", "public", src.replace(/^\//, ""));
    if (!existsSync(file)) {
      files.push({ src, bytes: null, gzip: null, entry, note: "not found on disk" });
      return;
    }
    const buf = readFileSync(file);
    raw += buf.length;
    gzipped += gzipSync(buf).length;
    files.push({ src, bytes: buf.length, gzip: gzipSync(buf).length, entry });

    // Follow static ESM imports: `from"./x.js"`, `import"./x.js"`.
    const text = buf.toString("utf8");
    const deps = [
      ...text.matchAll(/(?:from|import)\s*["']([^"']+)["']/g),
    ].map((m) => m[1]);
    for (const d of deps) {
      if (!d.startsWith(".")) continue; // bare specifier: not a built asset
      const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(src), d));
      visit(resolved, false);
    }
  };

  for (const src of srcs) visit(src, true);
  return { srcs, raw, gzipped, files };
}

function countScriptTags(html) {
  const all = [...html.matchAll(/<script\b[^>]*>/g)];
  const external = all.filter((m) => /\bsrc=/.test(m[0])).length;
  return { total: all.length, external, inline: all.length - external };
}

const server = spawn("node", ["dist/index.mjs"], {
  cwd: ROOT,
  env: { ...process.env, PORT: String(PORT), NODE_ENV: "production" },
  stdio: ["ignore", "pipe", "pipe"],
});
let serverLog = "";
server.stdout.on("data", (d) => (serverLog += d));
server.stderr.on("data", (d) => (serverLog += d));

const shutdown = () => {
  try {
    server.kill("SIGTERM");
  } catch {
    /* already gone */
  }
};
process.on("exit", shutdown);
process.on("SIGINT", () => {
  shutdown();
  process.exit(130);
});

if (!(await waitForServer())) {
  console.error("measure: server did not start on " + BASE);
  console.error(serverLog);
  shutdown();
  process.exit(1);
}

console.log(`server: node dist/index.mjs (PORT=${PORT}, NODE_ENV=production)`);
console.log(`node:   ${process.version}`);
console.log("");

// ---------------------------------------------------------------------------
// 1. Per-route byte budget
// ---------------------------------------------------------------------------

console.log("=================== 1. PER-ROUTE BYTE BUDGET ======================");
console.log(
  `${pad("route", 10)} ${pad("html", 10)} ${pad("html.gz", 9)} ${pad("scripts", 20)} ${pad("clientJS", 10)} ${pad("clientJS.gz", 11)}`,
);

const results = {};
for (const route of ["/static", "/stream", "/counter", "/counter-large"]) {
  const markers =
    route === "/stream" ? ["SHELL_READY", "ESTIMATE_READY", "RECS_READY"] : [];
  const p = await streamProfile(route, markers);
  const js = clientJs(p.html);
  const tags = countScriptTags(p.html);
  results[route] = { p, js, tags };
  console.log(
    `${pad(route, 10)} ${pad(bytes(p.html), 10)} ${pad(gz(p.html), 9)} ` +
      `${pad(`${tags.total} (${tags.external} ext/${tags.inline} inline)`, 20)} ` +
      `${pad(js.raw, 10)} ${pad(js.gzipped, 11)}`,
  );
}
console.log("");
for (const [route, { js }] of Object.entries(results)) {
  if (js.files.length === 0) {
    console.log(`  ${route}: NO external client JS referenced`);
  } else {
    for (const f of js.files) {
      console.log(`  ${route}: ${f.src} -> ${f.bytes} bytes (${f.gzip} gzip)${f.note ? " " + f.note : ""}`);
    }
  }
}
console.log("");

// ---------------------------------------------------------------------------
// 2. Streaming behavior
// ---------------------------------------------------------------------------

console.log("=================== 2. STREAMING BEHAVIOR =========================");
const s = results["/stream"].p;
console.log(`  transfer-encoding : ${s.transferEncoding ?? "(none)"}`);
console.log(`  response chunks   : ${s.chunks}`);
console.log(`  time to first byte: ${s.firstChunkAt?.toFixed(1)} ms`);
for (const m of ["SHELL_READY", "ESTIMATE_READY", "RECS_READY"]) {
  const t = s.arrivals[m];
  console.log(`  ${pad(m, 18)}: ${t === undefined ? "NEVER ARRIVED" : t.toFixed(1) + " ms"}`);
}
console.log(`  ${pad("response complete", 18)}: ${s.totalAt.toFixed(1)} ms`);
console.log("");
console.log("  expected (charter §15.5): estimate ~400 ms, recommendations ~1200 ms,");
console.log("  shell well before both, and the shell must NOT wait on either.");
console.log("");

const shellAt = s.arrivals.SHELL_READY ?? Infinity;
const estAt = s.arrivals.ESTIMATE_READY ?? Infinity;
const recsAt = s.arrivals.RECS_READY ?? Infinity;
console.log(`  check:shell-not-blocked=${shellAt < 300 ? "pass" : "fail"} (shell at ${shellAt.toFixed(1)} ms)`);
console.log(`  check:estimate-streamed=${estAt >= 380 && estAt < 900 ? "pass" : "fail"} (at ${estAt.toFixed(1)} ms)`);
console.log(`  check:recs-streamed=${recsAt >= 1180 ? "pass" : "fail"} (at ${recsAt.toFixed(1)} ms)`);
console.log(`  check:out-of-order-ok=${estAt < recsAt ? "pass" : "fail"} (faster subtree arrived first)`);
console.log("");

// ---------------------------------------------------------------------------
// 3. Zero-JS claim for non-interactive routes
// ---------------------------------------------------------------------------

console.log("=================== 3. ZERO-JS CLAIM =============================");
console.log("  Charter §18.4 budget: 'static page: 0 application JS and 0 runtime JS'.");
console.log("  Two things must be counted separately, because they cost differently:");
console.log("    downloaded JS = extra requests + parse/execute + cache");
console.log("    inline JS     = bytes in the HTML, no extra request");
console.log("");
for (const route of ["/static", "/stream", "/counter", "/counter-large"]) {
  const { js, tags, p } = results[route];
  const inlineBytes = [...p.html.matchAll(/<script\b(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)]
    .reduce((n, m) => n + bytes(m[1]), 0);
  console.log(
    `  ${pad(route, 16)} downloaded=${pad(js.raw + " B", 9)} ` +
      `inline=${pad(inlineBytes + " B", 8)} ` +
      `tags=${tags.external} ext / ${tags.inline} inline`,
  );
}
console.log("");
const st = results["/static"];
console.log(
  `  check:static-truly-zero-js=${st.js.raw === 0 && st.tags.total === 0 ? "pass" : "fail"}` +
    `  (no script tags at all, no downloaded JS)`,
);
const sr = results["/stream"];
console.log(
  `  check:stream-zero-downloaded-js=${sr.js.raw === 0 ? "pass" : "fail"}` +
    `  (streaming needs an INLINE shim, but downloads nothing)`,
);
console.log("");

// ---------------------------------------------------------------------------
// 4. Does the interactive payload scale with page size?
// ---------------------------------------------------------------------------

const c = results["/counter"];
console.log("=================== 4. PAYLOAD vs PAGE SIZE ======================");
console.log("  Charter §8.5: SSR should emit semantic HTML plus only the metadata needed");
console.log("  to resume interactions — the client payload must track the INTERACTIVE");
console.log("  ISLAND, not the size of the surrounding inert document.");
console.log("");
console.log("  /counter and /counter-large contain the SAME single button. /counter-large");
console.log("  wraps it in 200 extra inert table rows.");
console.log("");
const big = results["/counter-large"];
console.log(`  ${pad("route", 16)} ${pad("html bytes", 12)} ${pad("client JS", 11)} ${pad("client JS.gz", 12)}`);
console.log(`  ${pad("/counter", 16)} ${pad(bytes(c.p.html), 12)} ${pad(c.js.raw, 11)} ${pad(c.js.gzipped, 12)}`);
console.log(`  ${pad("/counter-large", 16)} ${pad(bytes(big.p.html), 12)} ${pad(big.js.raw, 11)} ${pad(big.js.gzipped, 12)}`);
console.log("");
const htmlGrowth = bytes(big.p.html) / bytes(c.p.html);
const jsGrowth = c.js.raw === 0 ? 0 : big.js.raw / c.js.raw;
console.log(`  HTML grew ${htmlGrowth.toFixed(1)}x;  client JS grew ${jsGrowth.toFixed(3)}x`);

// Split the total into the shared runtime chunk (amortized across all
// interactive routes) and the code unique to this route.
const routeOwn = (r) => r.js.files.filter((f) => f.entry).reduce((n, f) => n + (f.bytes ?? 0), 0);
const shared = (r) => r.js.files.filter((f) => !f.entry).reduce((n, f) => n + (f.bytes ?? 0), 0);
console.log("");
console.log(`  shared runtime chunk (amortized) : ${shared(c)} B`);
console.log(`  /counter route-specific code     : ${routeOwn(c)} B`);
console.log(`  /counter-large route-specific    : ${routeOwn(big)} B`);
console.log("");
// The route-specific code is what must not grow with the document.
const ownGrowth = routeOwn(c) === 0 ? 0 : routeOwn(big) / routeOwn(c);
console.log(
  `  check:payload-tracks-island=${ownGrowth < 1.1 ? "pass" : "fail"} ` +
    `(route code grew ${ownGrowth.toFixed(3)}x across a ${htmlGrowth.toFixed(1)}x larger document)`,
);
console.log("");
console.log("  This is the charter's 'no whole-tree hydration' budget (§18.4) holding:");
console.log("  a hydrating framework would need code proportional to the rendered tree.");
console.log("");

shutdown();
