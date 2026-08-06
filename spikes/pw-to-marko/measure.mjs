// spike: pw-to-marko — the E3 measurement harness.
//
// Charter §14 M3 task 8 asks for per-route bytes. This starts the built server,
// fetches each route, and reports HTML bytes, script tags, and client JS.
//
// The transitive import walk is carried over from the E0 harness deliberately.
// A naive version there counted only the entry chunk and reported /counter as
// 176 bytes of client JS; the entry `import`s a shared 3,745-byte runtime
// chunk, so measuring only the entry under-reported the real cost by 20x. The
// same mistake is available here and would make E3's zero-JS claim look better
// than it is.
//
// Exit code 0 always: this measures, it does not decide. The gate decision is
// made by a human reading docs/milestones/E3.md against these numbers.

import { spawn } from "node:child_process";
import { gzipSync } from "node:zlib";
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";

const PORT = Number(process.env.PORT || 3121);
const BASE = `http://127.0.0.1:${PORT}`;
const ROOT = path.dirname(new URL(import.meta.url).pathname);

const gz = (buf) => gzipSync(buf).length;
const pad = (s, n) => String(s).padEnd(n);

async function waitForServer(timeoutMs = 20000) {
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

/** Every built JS asset a page pulls in, following static imports transitively. */
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
      files.push({ src, bytes: null, entry, note: "not found on disk" });
      return;
    }
    const buf = readFileSync(file);
    raw += buf.length;
    gzipped += gz(buf);
    files.push({ src, bytes: buf.length, gzip: gz(buf), entry });

    const text = buf.toString("utf8");
    for (const m of text.matchAll(/(?:from|import)\s*["']([^"']+)["']/g)) {
      const d = m[1];
      if (!d.startsWith(".")) continue; // bare specifier: not a built asset
      visit(path.posix.normalize(path.posix.join(path.posix.dirname(src), d)), false);
    }
  };

  for (const s of srcs) visit(s, true);

  // Inline scripts count too: a runtime shim in a <script> with no src is still
  // JavaScript the browser downloads and runs.
  const inline = [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)]
    .map((m) => m[1])
    .filter((s) => s.trim().length > 0);
  const inlineBytes = inline.reduce((n, s) => n + Buffer.byteLength(s, "utf8"), 0);

  return { files, raw, gzipped, scriptTags: srcs.length, inline: inline.length, inlineBytes };
}

const server = spawn("node", ["dist/index.mjs"], {
  cwd: ROOT,
  env: { ...process.env, PORT: String(PORT) },
  stdio: ["ignore", "pipe", "pipe"],
});
server.stdout.on("data", () => {});
server.stderr.on("data", (d) => process.stderr.write(`[server] ${d}`));

try {
  if (!(await waitForServer())) {
    console.log("MEASUREMENT FAILED: server did not start");
    process.exit(0);
  }

  for (const route of ["/static", "/counter"]) {
    const res = await fetch(`${BASE}${route}`);
    const html = await res.text();
    const htmlBuf = Buffer.from(html, "utf8");
    const js = clientJs(html);

    console.log(`route ${route}`);
    console.log(`  status               ${res.status}`);
    console.log(`  HTML bytes           ${htmlBuf.length} (${gz(htmlBuf)} gzipped)`);
    console.log(`  <script src> tags    ${js.scriptTags}`);
    console.log(`  inline <script>      ${js.inline} (${js.inlineBytes} bytes)`);
    console.log(`  client JS bytes      ${js.raw} (${js.gzipped} gzipped)`);
    for (const f of js.files) {
      console.log(
        `      ${pad(f.entry ? "entry" : "import", 7)} ${pad(f.src, 40)} ${
          f.bytes === null ? f.note : `${f.bytes} B`
        }`,
      );
    }

    // The content the page must show is present in the FIRST response, which is
    // what "usable with JavaScript disabled" means for a static route.
    const marker = route === "/static" ? "Blue Bottle" : "In cart:";
    console.log(`  contains ${JSON.stringify(marker)}  ${html.includes(marker)}`);
    console.log();
  }

  console.log("Read these numbers against charter §14 M3's gate:");
  console.log("  - the static route must be usable with JavaScript disabled;");
  console.log("  - it should emit zero application JS, and any framework JS is");
  console.log("    a measured exception to explain, not to hide (§18.4).");
} finally {
  server.kill("SIGTERM");
}
