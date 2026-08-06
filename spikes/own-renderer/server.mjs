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

// E6's path, not a stand-in.
//
// The store page declares:
//
//   session query Cart(session)      cached private
//   command add_to_cart(..)          invalidates Cart(current_session())
//                                    emits      CartChanged(current_session())
//
// So a click must change the browser because the DECLARED RESOURCE DEPENDENCY
// changed, not because an endpoint returned a number. The chain here is the
// one `pw-materialize` implements, in miniature and with the same shape:
//
//   command → state + outbox committed together
//           → the materializer drains committed events
//           → the event's arguments select which resource entries invalidate
//           → the resource is refreshed, with a VERSION
//           → the subscriber is told
//
// The version is what makes a late arrival harmless: a subscriber that
// receives an older version than it holds keeps what it has.
const state = new Map(); // session → line count
const outbox = []; // committed events, in commit order
const resources = new Map(); // "Cart(session)" → { value, version }
const subscribers = new Map(); // session → resolve functions waiting for a change

function resourceKey(session) {
  return `Cart(${session})`;
}

function readCart(session) {
  const key = resourceKey(session);
  if (!resources.has(key)) {
    resources.set(key, { value: state.get(session) ?? 0, version: 0 });
  }
  return resources.get(key);
}

/** A command: the state change and the event, committed together or not at all. */
function command(session, apply) {
  const before = state.get(session) ?? 0;
  const events = [];
  try {
    apply({
      set: (v) => state.set(session, v),
      emit: (name, args) => events.push({ name, args }),
    });
  } catch (e) {
    // Rolled back: neither the state nor the event survives, which is the
    // property ADR-0019 exists for.
    state.set(session, before);
    throw e;
  }
  outbox.push(...events.map((e, i) => ({ id: outbox.length + i, ...e, consumed: false })));
}

/** The materializer: consume committed events, invalidate, refresh, notify. */
function drain() {
  const invalidated = new Set();
  for (const e of outbox) {
    if (e.consumed) continue;
    e.consumed = true;
    // The narrowing E6 is about: an event's arguments select which entries.
    if (e.name === "CartChanged") invalidated.add(resourceKey(e.args[0]));
    // MenuChanged reaches no cart entry, whatever its arguments.
  }
  for (const key of invalidated) {
    const session = /Cart\((.*)\)/.exec(key)?.[1];
    const entry = resources.get(key) ?? { value: 0, version: 0 };
    resources.set(key, { value: state.get(session) ?? 0, version: entry.version + 1 });
    for (const resolve of subscribers.get(session) ?? []) resolve();
    subscribers.delete(session);
  }
  return invalidated.size;
}

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
    // The command COMMITS. It returns nothing about the cart: the browser
    // learns the new value from the resource, because that is what the
    // program says it depends on.
    command(session, ({ set, emit }) => {
      set((state.get(session) ?? 0) + 1);
      emit("CartChanged", [session]);
    });
    drain();
    res.writeHead(202, { "content-type": TYPES[".json"] });
    res.end(JSON.stringify({ committed: true }));
    return;
  }

  // A command that fails after writing. Nothing may reach the browser.
  if (req.method === "POST" && req.url === "/command/add_and_fail") {
    try {
      command(session, ({ set }) => {
        set((state.get(session) ?? 0) + 1);
        throw new Error("the command failed after writing");
      });
    } catch {
      /* rolled back */
    }
    drain();
    res.writeHead(500, { "content-type": TYPES[".json"] });
    res.end(JSON.stringify({ committed: false }));
    return;
  }

  // An event nothing about the cart listens for.
  if (req.method === "POST" && req.url === "/command/menu_changed") {
    outbox.push({ id: outbox.length, name: "MenuChanged", args: ["47"], consumed: false });
    drain();
    res.writeHead(202, { "content-type": TYPES[".json"] });
    res.end("{}");
    return;
  }

  // The subscription: long-poll until this session's resource version moves
  // past what the caller holds. A socket would be the real transport; the
  // property under test is that the browser is told BECAUSE the resource
  // changed, and a poll makes that observable without another moving part.
  if (req.method === "GET" && req.url.startsWith("/resource/Cart")) {
    const since = Number(new URL(req.url, "http://x").searchParams.get("since") ?? -1);
    const send = () => {
      const entry = readCart(session);
      res.writeHead(200, { "content-type": TYPES[".json"] });
      res.end(
        JSON.stringify({ "cart.line_count": entry.value, version: entry.version }),
      );
    };
    if (readCart(session).version > since) return send();
    const waiters = subscribers.get(session) ?? [];
    waiters.push(send);
    subscribers.set(session, waiters);
    // A bounded wait, so a test that expects NO update does not hang.
    setTimeout(() => {
      if (!res.writableEnded) send();
    }, 1500);
    return;
  }

  if (req.method === "POST" && req.url === "/command/reset") {
    state.delete(session);
    resources.delete(resourceKey(session));
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
