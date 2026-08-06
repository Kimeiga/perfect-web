// A file server with one command endpoint.
//
// Deliberately this small. The E7-R claim is about the RENDERER and the browser
// runtime, and a framework here would be a second thing under test — the
// pages are files `pw-render` produced, and the one dynamic route exists
// because a command has to go somewhere.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join } from "node:path";

const DIST = new URL("./dist/", import.meta.url).pathname;
const PORT = Number(process.env.PORT ?? 3142);

// The cart, keyed by session — which is what `examples/store/app.pw` declares:
// `session query Cart(session: SessionId)`.
//
// A first version kept ONE number per process, and the browser tests run six
// workers against one server, so they observed each other's clicks. The
// symptom was an off-by-one that looked like a runtime bug and was a harness
// bug; keying it the way the program says removes both.
const carts = new Map();

function sessionOf(req, res) {
  const cookie = /pw-session=([^;]+)/.exec(req.headers.cookie ?? "")?.[1];
  if (cookie) return cookie;
  const fresh = `s-${Math.random().toString(36).slice(2)}`;
  res.setHeader("set-cookie", `pw-session=${fresh}; Path=/; SameSite=Lax`);
  return fresh;
}

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
};

const server = createServer(async (req, res) => {
  const session = sessionOf(req, res);

  if (req.method === "POST" && req.url === "/command/add_to_cart") {
    const next = (carts.get(session) ?? 0) + 1;
    carts.set(session, next);
    res.writeHead(200, { "content-type": TYPES[".json"] });
    // Keyed by the PATH the template reads, so the runtime updates by identity
    // rather than by a name this file and the page agreed on separately.
    res.end(JSON.stringify({ "cart.line_count": next }));
    return;
  }
  if (req.method === "POST" && req.url === "/command/reset") {
    carts.delete(session);
    res.writeHead(200, { "content-type": TYPES[".json"] });
    res.end("{}");
    return;
  }

  const path = req.url === "/" ? "/StorePage.html" : req.url.split("?")[0];
  try {
    const body = await readFile(join(DIST, path));
    res.writeHead(200, {
      "content-type": TYPES[extname(path)] ?? "application/octet-stream",
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/plain" });
    res.end("not found");
  }
});

server.listen(PORT, "127.0.0.1", () => {
  process.stdout.write(`own-renderer server on ${PORT}\n`);
});
