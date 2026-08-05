// spike: browser-resumption — instrumenting proxy.
//
// Starts the built Marko app and fronts it with a proxy that injects
// `public/harness.js` as the first script in <head>.
//
// Why a proxy rather than editing the templates: the thing under test must stay
// the real application. Marko treats `<script>` in a template as a reactive
// effect, not an HTML script tag, so instrumentation cannot simply be authored
// into the page — and authoring it in would change what is being measured.
//
// `?breakHandler=1` makes the proxy return 500 for the interaction artifact,
// which is how pre-registered check 10 (handler-load failure is observable) is
// exercised in both browsers without needing request interception.

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const MARKO_DIR = path.join(HERE, "..", "marko-stream-resume");

export async function startInstrumentedApp({ appPort = 3210, proxyPort = 3211 } = {}) {
  const app = spawn("node", ["dist/index.mjs"], {
    cwd: MARKO_DIR,
    env: { ...process.env, PORT: String(appPort), NODE_ENV: "production" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let appLog = "";
  app.stdout.on("data", (d) => (appLog += d));
  app.stderr.on("data", (d) => (appLog += d));

  const upstream = `http://127.0.0.1:${appPort}`;
  const harness = await readFile(path.join(HERE, "public", "harness.js"), "utf8");

  // Set by a request carrying ?breakHandler=1, so the *next* asset request for
  // the interaction artifact fails. Reset per navigation.
  let breakHandler = false;

  // Results POSTed by pages running in self-running (autorun) mode. Used for
  // browsers we cannot drive over a protocol — see harness.js.
  const collected = [];

  const proxy = createServer(async (req, res) => {
    const url = new URL(req.url, "http://127.0.0.1");

    if (url.pathname === "/__result" && req.method === "POST") {
      let body = "";
      for await (const chunk of req) body += chunk;
      try { collected.push(JSON.parse(body)); } catch { collected.push({ ok: false, raw: body.slice(0, 500) }); }
      res.writeHead(204, { "access-control-allow-origin": "*" });
      res.end();
      return;
    }

    if (url.pathname === "/__harness.js") {
      res.writeHead(200, { "content-type": "text/javascript", "cache-control": "no-store" });
      res.end(harness);
      return;
    }

    if (url.searchParams.has("breakHandler")) breakHandler = true;
    if (url.searchParams.has("resetHandler")) breakHandler = false;

    // Pre-registered check 10: fail the interaction artifact on purpose.
    if (breakHandler && /^\/assets\/.*\.js$/.test(url.pathname)) {
      res.writeHead(500, { "content-type": "text/plain", "cache-control": "no-store" });
      res.end("// deliberately failed by the resumption spike (check 10)");
      return;
    }

    try {
      const upstreamRes = await fetch(upstream + req.url, {
        headers: { ...req.headers, host: `127.0.0.1:${appPort}` },
        redirect: "manual",
      });
      const ct = upstreamRes.headers.get("content-type") || "";
      const headers = {
        "content-type": ct,
        "cache-control": "no-store",
      };

      if (!ct.includes("text/html")) {
        res.writeHead(upstreamRes.status, headers);
        res.end(Buffer.from(await upstreamRes.arrayBuffer()));
        return;
      }

      // HTML: inject the harness and STREAM the rest through unchanged, so the
      // streaming behaviour under test is preserved rather than buffered away.
      res.writeHead(upstreamRes.status, { ...headers, "transfer-encoding": "chunked" });
      const reader = upstreamRes.body.getReader();
      const decoder = new TextDecoder();
      let injected = false;
      let pending = "";
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        let chunk = decoder.decode(value, { stream: true });
        if (!injected) {
          pending += chunk;
          const at = pending.indexOf("<head>");
          if (at >= 0) {
            const insertAt = at + "<head>".length;
            const out =
              pending.slice(0, insertAt) +
              '<script src="/__harness.js"></script>' +
              pending.slice(insertAt);
            res.write(out);
            injected = true;
            pending = "";
            continue;
          }
          // Not seen <head> yet; hold a bounded amount then give up and pass through.
          if (pending.length > 8192) {
            res.write(pending);
            injected = true;
            pending = "";
          }
          continue;
        }
        res.write(chunk);
      }
      if (pending) res.write(pending);
      res.end();
    } catch (e) {
      res.writeHead(502, { "content-type": "text/plain" });
      res.end("proxy error: " + e.message);
    }
  });

  await new Promise((r) => proxy.listen(proxyPort, "127.0.0.1", r));

  // Wait for the upstream app to answer.
  const base = `http://127.0.0.1:${proxyPort}`;
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    try {
      const r = await fetch(`${base}/static`, { signal: AbortSignal.timeout(1000) });
      if (r.ok) break;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 150));
  }

  return {
    base,
    collected,
    waitForResults(n, timeoutMs = 30000) {
      const deadline = Date.now() + timeoutMs;
      return new Promise((resolve) => {
        const tick = () => {
          if (collected.length >= n || Date.now() > deadline) return resolve(collected.slice());
          setTimeout(tick, 200);
        };
        tick();
      });
    },
    stop() {
      try { app.kill("SIGTERM"); } catch { /* gone */ }
      try { proxy.close(); } catch { /* gone */ }
    },
    get log() { return appLog; },
  };
}
