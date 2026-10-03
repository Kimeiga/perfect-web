// E7-R — the browser runtime.
//
// It reads the parts manifest the document carries, asks `decide` whether a
// handler may attach, attaches the ones it authorises, and updates **only the
// parts whose values changed**.
//
// # What it never does
//
// No component tree, no replay, no re-render. The document arrived complete
// from the server; this attaches behaviour to elements that already exist and
// replaces the contents of ranges that already exist. Charter §14 M7 gate 4 —
// "no full-tree hydration occurs" — is a property of there being no tree here
// to hydrate.
//
// # Identity comes from the manifest, never from the DOM
//
// A part is found by the id the compiler assigned: `data-pw="7"` for an
// element-anchored part, `<!--pw:s3-->…<!--pw:e3-->` for a range. Nothing is
// located by tag name, class or position, because those change when a designer
// edits the page and an identity that moves is not an identity.

const parts = JSON.parse(document.getElementById("pw-parts")?.textContent ?? "{}");

// --- addressing ----------------------------------------------------------
//
// Architect ruling, 2026-08-06:
//
//   `TemplateSchemaId + LocalPartId` identifies a part POSITION. A live
//   document needs `TemplateSchemaId + InstancePath + LocalPartId`.
//
// The distinction is not about loops. A template part DEFINITION is one thing
// and a document part INSTANCE is another, and the same gap appears with a
// component used twice, a conditional region recreated after toggling, and a
// streamed instance. So `InstancePath` is generic: a frame is a repeatable
// scope, and today that scope is a keyed `{#each}`.
//
// ONE traversal builds the index. After that an address is a map lookup, not a
// DOM walk — the comments are the structural truth and this is the cheap view
// of it.

/** `[[eachPartId, token], ...]` → a stable string key. */
function addressOf(path, part) {
  // Matches `pw_document::PartAddress::key` exactly. A second spelling would
  // be a second entry in the index for one location, and a patch aimed at the
  // other spelling would find nothing — silently, because "no such address" and
  // "nothing to do" look the same from here.
  const frames = path.map(([e, t]) => `${e}@${t}`);
  return `${parts.schema}/${frames.join("/")}|${part}`;
}

/** address → { element } or { start, end } */
const index = new Map();

function buildIndex() {
  index.clear();
  const path = [];
  const open = new Map(); // partId → start comment, for ranges being walked
  const walker = document.createTreeWalker(
    document.body,
    NodeFilter.SHOW_COMMENT | NodeFilter.SHOW_ELEMENT,
  );
  let node;
  while ((node = walker.nextNode())) {
    if (node.nodeType === Node.ELEMENT_NODE) {
      const owner = node.dataset?.pw;
      if (owner !== undefined) {
        // An element-anchored part is addressed by the path it sits in, which
        // is what distinguishes the three Add buttons that all carry
        // `data-pw="0"`.
        index.set(addressOf(path, `e${owner}`), { element: node });
      }
      continue;
    }
    const data = node.data ?? "";
    if (!data.startsWith("pw:")) continue;

    const m = /^pw:([se])(\d+)(?:@([A-Za-z0-9_-]+))?$/.exec(data);
    if (!m) continue;
    const [, side, id, token] = m;

    if (token) {
      // A loop INSTANCE boundary: push a frame, or pop it.
      if (side === "s") path.push([Number(id), token]);
      else path.pop();
      continue;
    }
    if (side === "s") {
      open.set(id, { start: node, path: [...path] });
    } else {
      const started = open.get(id);
      if (started) {
        index.set(addressOf(started.path, id), { start: started.start, end: node });
        open.delete(id);
      }
    }
  }
  return index.size;
}

/** Every address for a part id, across every instance. */
function addressesFor(part) {
  const suffix = `|${part}`;
  return [...index.keys()].filter((k) => k.endsWith(suffix));
}

/**
 * Replace a range's contents, leaving its anchors and everything else alone.
 *
 * The anchors are never touched, so the part keeps its identity across the
 * update — which is what lets it be updated again.
 */
function setRange(address, text) {
  const r = index.get(address);
  if (!r?.start) return false;
  let n = r.start.nextSibling;
  while (n && n !== r.end) {
    const next = n.nextSibling;
    n.remove();
    n = next;
  }
  r.end.parentNode.insertBefore(document.createTextNode(text), r.end);
  return true;
}

/**
 * The nodes one loop instance owns, its anchors included.
 *
 * `null` when the instance is not in the document — which a patch targeting a
 * nonexistent instance must be refused for, rather than silently doing nothing.
 */
function instanceRange(scope, token) {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
  let start = null;
  let node;
  while ((node = walker.nextNode())) {
    if (node.data === `pw:s${scope}@${token}`) start = node;
    else if (node.data === `pw:e${scope}@${token}` && start) {
      return { start, end: node };
    }
  }
  return null;
}

/** Every node from an instance's start anchor to its end anchor, inclusive. */
function instanceNodes(range) {
  const nodes = [range.start];
  let n = range.start.nextSibling;
  while (n && n !== range.end) {
    nodes.push(n);
    n = n.nextSibling;
  }
  nodes.push(range.end);
  return nodes;
}

/**
 * Move nodes without recreating them, preserving as much state as the engine
 * allows.
 *
 * `Element.moveBefore()` is an ATOMIC move: the node is never removed from the
 * document, so focus, `:active`, CSS animations and transitions, iframe load
 * state, fullscreen, popover and modal state all survive. `insertBefore()` on
 * an attached node is specified as a remove followed by an insert, and every
 * one of those is lost.
 *
 * Verified against MDN (`Element/moveBefore`, read 2026-08-07): the method is
 * on the PARENT, the moved node must be an Element or CharacterData — comment
 * anchors qualify — and it is explicitly not Baseline. So the fallback is not
 * optional, and `docs/ASSUMPTIONS.md` records what it cannot preserve.
 *
 * The fallback restores focus and the caret by hand. That is a smaller claim
 * than the atomic move makes, and stating it is the point: an implementation
 * that quietly did less would pass the same focus test.
 */
const ATOMIC_MOVE = typeof Element.prototype.moveBefore === "function";

function moveNodes(parent, nodes, anchor) {
  if (ATOMIC_MOVE) {
    for (const node of nodes) parent.moveBefore(node, anchor);
    return "atomic";
  }
  const active = document.activeElement;
  const caret =
    active && "selectionStart" in active
      ? [active.selectionStart, active.selectionEnd]
      : null;
  const scroll = active?.scrollTop;
  for (const node of nodes) parent.insertBefore(node, anchor);
  if (active && active.isConnected && document.activeElement !== active) {
    active.focus({ preventScroll: true });
    if (caret) active.setSelectionRange(caret[0], caret[1]);
    if (scroll !== undefined) active.scrollTop = scroll;
  }
  return "restored";
}

/** Parse instance markup into nodes, anchors included. */
function parseInstance(html) {
  const template = document.createElement("template");
  template.innerHTML = html;
  return [...template.content.childNodes];
}

/**
 * Apply a structural operation to a keyed collection.
 *
 * Returns false when the operation names an instance the document does not
 * have. A patch that addressed nothing must be REFUSED rather than ignored:
 * the two are indistinguishable from the outside, and one of them means the
 * page and the server disagree about what the document contains.
 */
function applyList(key, scope, op) {
  switch (op.op) {
    case "insert_before":
    case "insert_after": {
      const loop = index.get(key);
      if (!loop) return false;
      let parent;
      let anchor;
      if (op.instance == null) {
        // No anchor instance: the head for `insert_before`, the tail for
        // `insert_after`. The only way into an EMPTY collection, which has no
        // instance to sit beside.
        parent = loop.start.parentNode;
        anchor = op.op === "insert_before" ? loop.start.nextSibling : loop.end;
      } else {
        const at = instanceRange(scope, op.instance);
        if (!at) return false;
        parent = at.start.parentNode;
        anchor = op.op === "insert_before" ? at.start : at.end.nextSibling;
      }
      for (const node of parseInstance(op.html)) parent.insertBefore(node, anchor);
      return true;
    }

    case "remove_instance": {
      const at = instanceRange(scope, op.instance);
      if (!at) return false;
      for (const node of instanceNodes(at)) node.remove();
      return true;
    }

    case "move_instance": {
      const at = instanceRange(scope, op.instance);
      if (!at) return false;
      // The NODES are moved, not recreated. That is the whole reason a list is
      // keyed: a move that deleted and re-rendered would lose focus, scroll,
      // form state and any identity a test marked — and would look identical
      // in a screenshot.
      const nodes = instanceNodes(at);
      let anchor;
      if (op.after) {
        const after = instanceRange(scope, op.after);
        if (!after) return false;
        anchor = after.end.nextSibling;
      } else {
        // To the front: just inside the LOOP's own range, whose address is
        // the patch's target. Reconstructing that address from the scope id
        // would assume the loop is at the top level, which a nested loop is
        // not.
        const loop = index.get(key);
        if (!loop) return false;
        anchor = loop.start.nextSibling;
      }
      // Already where it is asked to go: its own first node is the anchor,
      // and there is nothing to move. Moved anyway, `moveBefore` put the
      // instance's nodes before its own start, and the list's anchors no
      // longer nested (found 2026-10-03, ADR-0168).
      if (nodes.includes(anchor)) return true;
      window.__pw.moveKind = moveNodes(at.start.parentNode, nodes, anchor);
      return true;
    }

    default:
      return false;
  }
}

// --- the compatibility decision -----------------------------------------
//
// The same wasm E7V compiled, and the same rule: the gate FAILS CLOSED. A
// handler that attached because the decision had not loaded would be the bypass
// this design exists to prevent.

let decide = null;

async function bootDecision() {
  const response = await fetch("/pw-resume.wasm");
  const { instance } = await WebAssembly.instantiateStreaming(response, {});
  const { memory, alloc, decide_manifest, last_recovery, know } = instance.exports;

  const write = (s) => {
    const bytes = new TextEncoder().encode(s);
    const ptr = alloc(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  };
  // What this build compiled (ADR-0132), from the server that served this
  // runtime and never from the document: a document cached from another
  // build names handlers this one may not have. If the table cannot be read,
  // nothing is known and every handler is refused, which is the closed side.
  const table = await fetch("/pw-handlers").then((r) => (r.ok ? r.text() : ""));
  const known = know(...write(table));
  log.push(`knows ${known} handler(s)`);
  // `last_recovery` returns an INDEX into this list, not a pointer. A first
  // version read it as one and every decision threw `Start offset -1 is
  // outside the bounds of the buffer`, which the page reported as "boot
  // failed" — so no handler attached and the button was inert.
  //
  // Worth recording: fail-closed did its job. A bug in the code that READS the
  // decision produced a page that does nothing, not a page that attaches
  // anyway. That is the difference the design was for.
  //
  // In `pw-resume-wasm`'s order, from 0: `RefetchRegion` is 0 there. Until
  // 2026-10-03 this list began with a "none" the ABI does not have, so every
  // recovery was read one place off. A region to refetch read as "none", a
  // document to reload as "rerender-private-slot". Nothing acted on a
  // recovery then, so only the log was wrong (ADR-0155).
  const RECOVERY = [
    "refetch-region",
    "rerender-private-slot",
    "reload",
    "retry-interaction",
    "require-user-confirmation",
    "reject-irrecoverable",
  ];

  decide = (manifest) => {
    const [ptr, len] = write(manifest);
    // 0 resume, 1 migrate; anything else is a refusal.
    const code = decide_manifest(ptr, len);
    return {
      attach: code === 0 || code === 1,
      recovery: RECOVERY[last_recovery()] ?? "unknown",
      code,
    };
  };
}

// --- attaching -----------------------------------------------------------

/** What the page reports about itself, for the tests to read. */
const log = [];
window.__pw = {
  log,
  parts,
  /** The `PartAddress`es the last interaction updated. */
  updated: [],
  ready: false,
  address: (path, part) => addressOf(path, part),
  indexSize: () => index.size,
  /** Each streamed region that settled after boot, and who filled it:
   * `browser` where the platform applied its patch, `runtime` where this
   * did (ADR-0148). */
  settled: [],
};

// The manifest a handler presents, in the ABI `pw-resume-wasm` reads:
//
//   scheme|abi|build|handler|capture|document|scope|captures|construct
//
// Composed from what the SERVER put in the document, not from constants here.
// A test that wants an incompatible manifest changes the served data; it does
// not edit this file, which is what makes the refusal path testable rather
// than a branch nobody exercises.
const RESUME_FIELDS = [
  "scheme",
  "abi",
  "build",
  "handler",
  "capture",
  "document",
  "scope",
  "captures",
  "construct",
];

/**
 * The manifest for ONE handler.
 *
 * The document carries a base manifest and the part carries which handler it
 * is, so two handlers on one page present two manifests and `decide` answers
 * about the one being attached. A single page-wide manifest would authorise
 * every handler on the strength of whichever one it described.
 */
function manifestFor(part) {
  const r = parts.resume;
  if (!r) return null;
  // By the handler's IDENTITY (ADR-0132), which names its code; a name does
  // not, and a handler that calls nothing has none.
  const per = (r.handlers ?? {})[part?.value] ?? {};
  return RESUME_FIELDS.map((f) => per[f] ?? r[f] ?? "").join("|");
}

// --- E7-L: handler code, loaded on interaction --------------------------
//
// Architect ruling, 2026-08-07: handler bytes absent before interaction; the
// exact handler fetched on first interaction; E7V check → Authorised → attach
// → run; unrelated handler code still absent.
//
// So the document carries no behaviour. An element with a handler gets a
// listener whose whole job is to LOAD the handler, check it, and run it — and
// the second press finds it already loaded.

/** handler identity → the module, or the in-flight promise for it. */
const loaded = new Map();

/**
 * Fetch a handler by IDENTITY.
 *
 * By identity rather than by name, because a changed implementation is a
 * different identity and therefore a different URL. A name-keyed URL would let
 * a cache serve yesterday's behaviour into today's document, and the resume
 * check would pass — it compares identities, and the document's identity would
 * be the new one.
 *
 * In flight is remembered, not just resolved: two fast presses must not become
 * two fetches, and a `has()` check that only saw finished loads would let the
 * second press start a second one.
 */
const attempts = new Map();

function loadHandler(identity) {
  if (loaded.has(identity)) return loaded.get(identity);
  // A retry asks for a different specifier.
  //
  // The module map caches FAILURES as well as successes, so re-importing the
  // same specifier after a failed load returns the same rejected promise
  // forever — the network is never touched again and the button is
  // permanently dead. The identity still names the module; the counter only
  // makes the failed attempt distinguishable from the next one.
  const attempt = attempts.get(identity) ?? 0;
  const url = attempt === 0 ? `/handler/${identity}.mjs` : `/handler/${identity}.mjs?attempt=${attempt}`;
  const pending = import(url).then((module) => {
    loaded.set(identity, module);
    log.push(`loaded ${identity}`);
    return module;
  });
  loaded.set(identity, pending);
  window.__pw.fetched = (window.__pw.fetched ?? 0) + 1;
  return pending;
}

/**
 * A command, as a compiled handler calls one: the command's component id and
 * the arguments the handler computed.
 *
 * The server types the arguments by the command component's own parameters
 * and refuses anything else. A refusal is thrown, so the press fails visibly.
 * A command that ran and did not commit is not a refusal: the page learns what
 * happened from the resource, as it does when a command commits.
 */
async function command(component, args, interaction) {
  const response = await fetch(`/command/${encodeURIComponent(component)}`, {
    method: "POST",
    // The interaction this request belongs to (ADR-0121). One per press and
    // per command the press calls, made here and never by the handler, and
    // carried unchanged by any retry of this request: a command declared
    // `idempotent_by InteractionId` runs once for it however many times the
    // request is sent.
    headers: { "content-type": "application/json", "pw-interaction": interaction },
    body: JSON.stringify(args),
  });
  if (!response.ok) {
    throw new Error(`command ${component} refused: HTTP ${response.status}`);
  }
  return response.json();
}

// --- ADR-0122: optimistic transitions -----------------------------------
//
// ADR-0025: an optimistic clause targets a resource ENTRY, binds its current
// value, and transforms it with a pure function. The runtime holds the value
// it displayed, so a rejected command is undone by restoring that value, not
// by an inverse anyone wrote.
//
//   held        the authoritative value and version: the document's, then
//               each `entry_value` frame's
//   pending     speculations not yet reconciled, oldest first
//   displayed   fold(pending, held): what the speculated parts show
//
// A speculation is dropped when its command fails (restored) or when the
// entry reaches the version its commit produced (reconciled). The parts are
// then re-rendered from what remains, so a later press's speculation survives
// an earlier one's resolution, and nothing is restored over a newer value.

/** binding → { entry, version, raw, value, pending } */
const speculated = new Map();
for (const [binding, e] of Object.entries(parts.entries ?? {})) {
  speculated.set(binding, {
    entry: e.entry,
    version: e.version,
    raw: e.value,
    value: undefined,
    pending: [],
  });
}

let speculationModule = null;

/** The page's speculation module, loaded with the first press that needs it. */
function speculations() {
  if (!parts.speculation) return Promise.resolve(null);
  speculationModule ??= import(parts.speculation);
  return speculationModule;
}

function current(module, binding) {
  const state = speculated.get(binding);
  if (state && state.value === undefined && state.raw !== undefined) {
    state.value = module.decode[binding](state.raw);
    state.raw = undefined;
  }
  return state;
}

/** What a template writes has a text form (ADR-0074). */
function textOf(v) {
  return typeof v === "string" ? v : String(v);
}

function render(module, binding) {
  const state = current(module, binding);
  if (!state || state.value === undefined) return;
  let value = state.value;
  for (const p of state.pending) value = p.transition(value, p.args);
  for (const [part, read] of Object.entries(module.parts[binding] ?? {})) {
    setRange(addressOf([], Number(part)), textOf(read(value)));
  }
}

let nextSpeculation = 0;

/** Before a command's request: show each of its transitions this page can. */
async function speculate(component, args) {
  const module = await speculations();
  const ids = [];
  for (const s of module?.commands?.[component] ?? []) {
    const state = current(module, s.binding);
    if (!state || state.value === undefined) continue;
    const id = nextSpeculation++;
    state.pending.push({ id, transition: s.transition, args, until: undefined });
    ids.push([s.binding, id]);
    render(module, s.binding);
    log.push(`speculated ${component} on ${s.binding}`);
  }
  return ids;
}

/** The command answered: reconciled when its version arrives, or restored. */
async function resolveSpeculation(ids, committed, basis) {
  if (ids.length === 0) return;
  const module = await speculations();
  for (const [binding, id] of ids) {
    const state = speculated.get(binding);
    const i = state?.pending.findIndex((p) => p.id === id) ?? -1;
    if (i < 0) continue;
    const version = (basis ?? []).find((b) => b.entry === state.entry)?.version;
    if (!committed) {
      state.pending.splice(i, 1);
      log.push(`restored ${binding}`);
    } else if (version === undefined || version <= state.version) {
      // The value the page holds already includes this commit.
      state.pending.splice(i, 1);
      log.push(`reconciled ${binding} at version ${state.version}`);
    } else {
      state.pending[i].until = version;
    }
    render(module, binding);
  }
}

/** An `entry_value` frame: the authoritative value at a version. */
function entryValue(frame) {
  for (const [binding, state] of speculated) {
    if (state.entry !== frame.entry || frame.version <= state.version) continue;
    state.version = frame.version;
    state.raw = frame.value;
    state.value = undefined;
    const before = state.pending.length;
    state.pending = state.pending.filter((p) => p.until === undefined || p.until > frame.version);
    if (before !== state.pending.length) {
      log.push(`reconciled ${binding} at version ${frame.version}`);
    }
  }
}

/** After a batch: a patch writes the authoritative text, and a speculation
 * still pending is shown over it again. */
async function reapplySpeculations() {
  if (!speculationModule) return;
  const module = await speculationModule;
  for (const [binding, state] of speculated) {
    if (state.pending.length > 0) render(module, binding);
  }
}

// --- ADR-0130: signals ---------------------------------------------------
//
// A page's UI state. The document carries each signal's first value, each
// part a signal decides, and the template of each block one decides. A
// handler changes a signal through its context; this holds the value and
// renders again exactly the parts that read it: a text part by setting its
// range, a block by rendering its template with the browser's build of the
// server's renderer (`pw-render-wasm`), so the two agree by being one
// implementation. Nothing here asks the server for anything.

/** name → its value, as JSON carries it (`js_pure`'s wire form). */
const signals = new Map(Object.entries(parts.signals ?? {}));

/** Each signal's first value, as the document gave it: what an instance
 * starts again at when its block shows another arm (ADR-0144). */
const firstValues = new Map(Object.entries(parts.signals ?? {}));

/** What `{#if}` reads as true, as the renderer reads it (`Value::truthy`). */
function truthy(v) {
  if (typeof v === "boolean") return v;
  if (typeof v === "number") return v !== 0;
  if (typeof v === "string") return v !== "";
  if (Array.isArray(v)) return v.length > 0;
  return v !== null && v !== undefined;
}

/** The arm a block a signal decides shows: whether an `{#if}`'s value
 * holds, or a `{#match}`'s case. */
function armOf(live) {
  const v = signalAt(live.path);
  if (live.kind === "conditional") return truthy(v);
  return v !== null && typeof v === "object" && "$case" in v ? v.$case : JSON.stringify(v);
}

/** The arm each block that holds signals last showed (ADR-0144). */
const arms = new Map();
for (const live of parts.live ?? []) {
  if (live.owns?.length) arms.set(live.part, armOf(live));
}
let renderer = null;

/** The renderer, loaded with the first block a signal renders again. */
function renderModule() {
  renderer ??= fetch("/pw-render.wasm")
    .then((r) => WebAssembly.instantiateStreaming(r, {}))
    .then(({ instance }) => instance.exports);
  return renderer;
}

/** A path read from the signals' values: `greeting`, or `panel.item`. */
function signalAt(path) {
  const [root, ...fields] = path.split(".");
  let v = signals.get(root);
  for (const f of fields) v = v?.[f];
  return v;
}

/** The block numbered `id`, rendered at the signals' values now. */
async function renderBlock(id) {
  const x = await renderModule();
  const request = new TextEncoder().encode(
    JSON.stringify({ part: parts.blocks[id], values: Object.fromEntries(signals) }),
  );
  const ptr = x.alloc(request.length);
  new Uint8Array(x.memory.buffer, ptr, request.length).set(request);
  const code = x.render_part(ptr, request.length);
  const out = new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.out_ptr(), x.out_len()));
  if (code !== 0) throw new Error(`block ${id} did not render: ${out}`);
  return out;
}

/** The nodes from `start` to `end`, both included. */
function between(start, end) {
  const out = [];
  for (let n = start; n; n = n.nextSibling) {
    out.push(n);
    if (n === end) break;
  }
  return out;
}

/** Each `<dialog>` among `nodes` and inside them, open or shut as asked. */
function dialogsIn(nodes, open) {
  const out = [];
  for (const n of nodes) {
    if (n.nodeType !== Node.ELEMENT_NODE) continue;
    if (n.localName === "dialog" && n.open === open) out.push(n);
    for (const d of n.querySelectorAll("dialog")) if (d.open === open) out.push(d);
  }
  return out;
}

/**
 * **A `<dialog>` a signal's block renders is the browser's modal dialog**
 * (ADR-0141): shown with `showModal`, which focuses its `autofocus` element
 * and makes the page behind it inert. Escape closes it by itself, and the
 * dialog's `close` handler tells the signal. A `<dialog open>` is HTML's
 * dialog shown in place, and is left alone.
 */
function showDialogs(nodes) {
  for (const d of dialogsIn(nodes, false)) {
    if (!d.isConnected || d.hasAttribute("open")) continue;
    // What opened it: what has focus, or else the element whose handler ran
    // last. WebKit does not focus a button a pointer presses, and focus goes
    // back to what invoked a dialog (WAI-ARIA's dialog pattern).
    const focused = document.activeElement;
    openedFrom.set(d, focused && focused !== document.body ? focused : lastActed);
    d.addEventListener("close", () => giveFocusBack(d), { once: true });
    d.showModal();
  }
}

/** What had focus when each dialog was shown, or what invoked it. */
const openedFrom = new WeakMap();

/** The element whose handler ran last. */
let lastActed = null;

/**
 * **Focus goes back to what had it** when a modal dialog closes, as the HTML
 * standard says `close` does (ADR-0141). Some engines do not yet: WebKit
 * leaves focus in the closed dialog, or on the body. Done only where focus
 * was lost, so an engine that did it, or a handler that moved focus on
 * purpose, is left alone.
 */
function giveFocusBack(d) {
  const from = openedFrom.get(d);
  openedFrom.delete(d);
  const now = document.activeElement;
  const lost = !now || now === document.body || d.contains(now) || !now.isConnected;
  if (lost && from?.isConnected) from.focus();
}

// --- streamed regions (ADR-0148) ------------------------------------------
//
// A region whose query the page does not wait for is sent pending: its
// placeholder inside `<?start name="pw-N">` .. `<?end>`, and later in the
// same response its settled arm, as `<template for="pw-N">`. A browser with
// the platform's out-of-order streaming (Chrome 150+) applies it as it
// parses. One without reads the markers as comments and keeps the template,
// inert, at the end of the body; this applies it there. Either way, once a
// region has settled its parts are read again and their handlers bound.

const STREAM_NAME = /^pw-(\d+)$/;

/** The stream parts still showing their placeholder, as of boot and since. */
const pendingStreams = new Set();

/** Is the region of stream `id` still pending: does it hold its start
 * marker, a processing instruction or, where the parser has none, the
 * comment it reads one as? */
function streamPending(id) {
  const r = index.get(addressOf([], id));
  if (!r?.start) return false;
  for (let n = r.start.nextSibling; n && n !== r.end; n = n.nextSibling) {
    if (n.nodeType === Node.PROCESSING_INSTRUCTION_NODE && n.target === "start") return true;
    if (n.nodeType === Node.COMMENT_NODE && n.data.startsWith("?start ")) return true;
  }
  return false;
}

/** Apply each `<template for>` the browser left in the document: the
 * region's contents, between its anchors, become the template's. Returns the
 * parts it filled. */
function applyStreamPatches() {
  const applied = new Set();
  for (const t of document.querySelectorAll("template[for]")) {
    const m = STREAM_NAME.exec(t.getAttribute("for") ?? "");
    const r = m && index.get(addressOf([], m[1]));
    if (!r?.start) continue;
    for (let n = r.start.nextSibling; n && n !== r.end; ) {
      const next = n.nextSibling;
      n.remove();
      n = next;
    }
    r.end.parentNode.insertBefore(document.importNode(t.content, true), r.end);
    t.remove();
    applied.add(m[1]);
  }
  return applied;
}

/** Each region that settled since this last looked: read again, its
 * handlers bound, and reported. */
function settleStreams() {
  const filled = applyStreamPatches();
  if (filled.size > 0) buildIndex();
  const settled = [...pendingStreams].filter((id) => !streamPending(id));
  if (settled.length === 0) return;
  for (const id of settled) {
    pendingStreams.delete(id);
    window.__pw.settled.push({ part: Number(id), by: filled.has(id) ? "runtime" : "browser" });
  }
  buildIndex();
  bindEvents();
}

/** Watch the document for regions settling, while the response that carries
 * them is still arriving. */
function watchStreams() {
  for (const p of parts.parts ?? []) {
    if (p.kind === "stream" && streamPending(String(p.id))) pendingStreams.add(String(p.id));
  }
  if (pendingStreams.size === 0) return;
  settleStreams();
  if (document.readyState === "complete" || pendingStreams.size === 0) return;
  const observer = new MutationObserver(settleStreams);
  observer.observe(document.body, { childList: true, subtree: true });
  // The response has ended: nothing more arrives for these regions.
  window.addEventListener(
    "load",
    () => {
      settleStreams();
      observer.disconnect();
    },
    { once: true },
  );
}

/** Replace a block's range, its anchors included, with what was rendered,
 * and say what was put in its place. */
function replaceBlock(id, html) {
  return replaceRangeAt(addressOf([], id), html) ?? [];
}

/** The range at `key` replaced by `html`, or `null` where the document has
 * no such range (ADR-0146). */
function replaceRangeAt(key, html) {
  const r = index.get(key);
  if (!r?.start) return null;
  const parent = r.start.parentNode;
  const after = r.end.nextSibling;
  // A modal dialog the block showed is closed before it goes, so the browser
  // gives focus back to what had it before the dialog took it (ADR-0141).
  for (const d of dialogsIn(between(r.start, r.end), true)) {
    d.close();
    giveFocusBack(d);
  }
  for (let n = r.start; n; ) {
    const next = n.nextSibling;
    const last = n === r.end;
    n.remove();
    if (last) break;
    n = next;
  }
  const inserted = parseInstance(html);
  for (const node of inserted) parent.insertBefore(node, after);
  return inserted;
}

/**
 * **An attribute a signal decides, set in place** (ADR-0142), on the element
 * the part belongs to. A field's `value` is its property, and is not set from
 * the field's own typing: what it holds is what was typed, and a value its
 * handler set behind later keys would move the caret under the person typing
 * (ADR-0131's concern). Any other change, a handler clearing it, is shown. A
 * boolean attribute is its presence, and its property where the element has
 * one (`checked`, `disabled`).
 */
function setAttributePart(live, source) {
  const part = (parts.parts ?? []).find((p) => p.id === live.part);
  if (!part) return;
  const v = signalAt(live.path);
  for (const a of addressesFor(`e${part.owner}`)) {
    const el = index.get(a)?.element;
    if (!el) continue;
    if (live.kind === "boolean_attribute") {
      const on = v === true;
      el.toggleAttribute(live.attribute, on);
      if (typeof el[live.attribute] === "boolean") el[live.attribute] = on;
    } else if (live.attribute === "value" && "value" in el) {
      const typed = source?.el === el && source.event === "input";
      const text = textOf(v);
      if (!typed && el.value !== text) el.value = text;
    } else {
      el.setAttribute(live.attribute, textOf(v));
    }
  }
}

let dirty = new Set();
let flushing = null;

/** Where each signal's latest change came from: the element and the event
 * whose handler set it (ADR-0142). */
let sources = new Map();

/** A handler changed `name`: held now, and rendered once per press. A value
 * the signal already holds changes nothing, and renders nothing. */
function setSignal(name, value, source) {
  if (JSON.stringify(signals.get(name)) === JSON.stringify(value)) return;
  signals.set(name, value);
  if (source) sources.set(name, source);
  else sources.delete(name);
  dirty.add(name);
  flushing ??= Promise.resolve().then(flushSignals);
}

async function flushSignals() {
  const changed = dirty;
  const from = sources;
  dirty = new Set();
  sources = new Map();
  flushing = null;
  let rendered = false;
  const inserted = [];
  for (const live of parts.live ?? []) {
    if (live.kind === "text") {
      if (!changed.has(live.signal)) continue;
      setRange(addressOf([], live.part), textOf(signalAt(live.path)));
      log.push(`signal ${live.signal} -> part ${live.part}`);
    } else if (live.kind === "attribute" || live.kind === "boolean_attribute") {
      if (!changed.has(live.signal)) continue;
      setAttributePart(live, from.get(live.signal));
      log.push(`signal ${live.signal} -> attribute ${live.attribute} of part ${live.part}`);
    } else if ((live.reads ?? [live.signal]).some((s) => changed.has(s))) {
      // **A signal ends with its instance** (ADR-0130, R6; ADR-0144): when a
      // block shows another arm, what its arms held starts again, before the
      // block renders. Everything that reads them is inside it.
      if (live.owns?.length) {
        const arm = armOf(live);
        if (arm !== arms.get(live.part)) {
          for (const s of live.owns) signals.set(s, firstValues.get(s));
          arms.set(live.part, arm);
        }
      }
      inserted.push(...replaceBlock(live.part, await renderBlock(live.part)));
      rendered = true;
      log.push(`signal ${[...changed].join(",")} -> block ${live.part}`);
    }
  }
  if (rendered) {
    buildIndex();
    bindEvents();
    // Shown once what is in it can be pressed (ADR-0141).
    showDialogs(inserted);
  }
  readAgain(changed);
  window.__pw.signals = Object.fromEntries(signals);
}

// --- ADR-0152: a key a page changes ---------------------------------------
//
// A page's query given a signal is read again, for the new key, when the
// signal changes. The server answers in the session's frames, as one patch
// set, and applies a read only if it is the latest for this page. So the
// browser only says which key each binding asks for, and does what its stale
// work says:
//   cancel     the read in flight is aborted, and the new one sent;
//   supersede  the read in flight runs to its end, and the new one is sent;
//   keep       the new one is sent, and the server runs it after the one in
//              flight, dropping any it has not started that a newer replaced.
// In none is an old key's answer shown for the new key: the server applies
// only the latest read.

/** The document this page is: its first cursor, which each read names. */
const documentCursor = parts.cursor ?? 0;

/** Each binding a signal keys: its signals, its stale work, and its reads. */
const keyedReads = new Map(
  (parts.keyed ?? []).map((k) => [
    k.binding,
    { signals: k.signals, policy: k.on_key_change, seq: 0, current: null, inflight: null },
  ]),
);
for (const read of keyedReads.values()) read.current = keyOf(read);

/** A binding's key: each of its signals' values. */
function keyOf(read) {
  return JSON.stringify(Object.fromEntries(read.signals.map((s) => [s, signals.get(s)])));
}

/** The bindings whose keys `changed` changed, each asked for again. */
function readAgain(changed) {
  for (const [binding, read] of keyedReads) {
    if (!read.signals.some((s) => changed.has(s))) continue;
    const key = keyOf(read);
    // A key this page has, or is asking for, is not asked for again
    // (charter §15.6, test 6).
    if (key === read.current) continue;
    read.current = key;
    if (read.inflight && read.policy === "cancel") read.inflight.abort();
    ask(binding, read, key);
  }
}

/** One read of `binding` for `key`, numbered. */
function ask(binding, read, key) {
  const seq = ++read.seq;
  const controller = new AbortController();
  read.inflight = controller;
  busy(binding, true);
  window.__pw.reads = (window.__pw.reads ?? 0) + 1;
  const url =
    `/pw-read?binding=${encodeURIComponent(binding)}&seq=${seq}` +
    `&doc=${documentCursor}&key=${encodeURIComponent(key)}`;
  fetch(url, { signal: controller.signal })
    .then((r) => r.json())
    .then((answer) => log.push(`read ${binding} #${seq}: ${JSON.stringify(answer)}`))
    .catch((e) =>
      log.push(`read ${binding} #${seq} ${e?.name === "AbortError" ? "aborted" : "failed"}`),
    )
    .finally(() => {
      if (read.inflight !== controller) return;
      read.inflight = null;
      busy(binding, false);
    });
}

/** Mark what `binding` shows as waiting for a new key's answer. */
function busy(binding, waiting) {
  for (const p of parts.parts ?? []) {
    if (String(p.value ?? "").split(".")[0] !== binding) continue;
    const at = index.get(addressOf([], p.id))?.start?.parentElement;
    if (!at) continue;
    if (waiting) at.setAttribute("aria-busy", "true");
    else at.removeAttribute("aria-busy");
  }
}

// Leaving the page stops its reads (charter §15.6, test 8): the server, finding
// a read's request gone, lets go of a `cancel` read's flight.
addEventListener("pagehide", () => {
  for (const read of keyedReads.values()) read.inflight?.abort();
});

/** Each event part's decision, asked once (E7-L): before anything binds. */
const verdicts = new Map();

/** The recoveries a press performs by reading the document again: each is
 * the region, the slot or the interaction as the build serving the page
 * renders it now. A reload never replays the press: a mutation the user did
 * not ask for again is how a version mismatch becomes a double charge. */
const RELOADING = new Set(["reload", "refetch-region", "rerender-private-slot", "retry-interaction"]);

/**
 * **A refused handler recovers when pressed** (charter §15.6 test 16). The
 * resume decision refused it, and said how to recover. A document from another
 * build is read again, once, and the press is not replayed. Until 2026-10-03 a
 * press on it did nothing and said nothing.
 *
 * A reload asked for again within ten seconds, for the same address, is not
 * made. Otherwise a server still sending the stale document would reload the
 * page for ever. The button then stays inert, and says so.
 */
function recoverOnPress(part, owners, verdict) {
  const listens = DOM_EVENTS[part.event || "press"];
  if (!listens) return;
  for (const el of owners) {
    const parts = bound.get(el) ?? new Set();
    if (parts.has(part.id)) continue;
    parts.add(part.id);
    bound.set(el, parts);
    el.addEventListener(listens, (e) => {
      const asks = verdict.recovery === "require-user-confirmation";
      if (!RELOADING.has(verdict.recovery) && !asks) {
        el.dataset.pwHandlerError = verdict.recovery;
        log.push(`press on refused ${part.id}: ${verdict.recovery}, nothing to do`);
        return;
      }
      e.preventDefault();
      if (asks && !confirm("This page is out of date. Load it again? What you typed is not kept.")) {
        return;
      }
      const key = `pw-recovered:${location.pathname}`;
      let last = 0;
      try {
        last = Number(sessionStorage.getItem(key) ?? 0);
      } catch {}
      if (Date.now() - last < 10_000) {
        el.dataset.pwHandlerError = "reload-loop";
        log.push(`press on refused ${part.id}: read again already; not again`);
        return;
      }
      try {
        sessionStorage.setItem(key, String(Date.now()));
      } catch {}
      log.push(`press on refused ${part.id}: ${verdict.recovery}, reading the page again`);
      window.__pw.reloading = true;
      location.reload();
    });
  }
}
/** When the latest press's handler started (ADR-0152): the next press's
 * starts after it, so presses run in the order they were made. */
let lastPressStarted = Promise.resolve();
/** Each element's parts a listener is bound for: an element may handle two
 * events, `on:input` and `on:keydown`, each its own listener. */
const bound = new WeakMap();

/**
 * **What the browser calls each event the platform declares** (ADR-0138).
 * Until 2026-10-02 every handler listened for a click, whatever its event.
 */
const DOM_EVENTS = {
  press: "click",
  input: "input",
  change: "change",
  keydown: "keydown",
  submit: "submit",
  close: "close",
};

/**
 * **An event's record** (ADR-0131), the plain data its handler is given, as
 * `packages/pw-platform-web/events.pw` declares it. Read here, in the
 * listener: the DOM event is spent by the time a handler's module has loaded.
 */
function eventRecord(name, e) {
  switch (name) {
    case "press":
      return { x: Math.round(e.clientX ?? 0), y: Math.round(e.clientY ?? 0) };
    case "input":
    case "change":
      return { value: String(e.target?.value ?? "") };
    case "keydown":
      return { key: String(e.key ?? "") };
    case "submit":
      return { prevented: e.defaultPrevented === true };
    case "close":
      return { value: String(e.target?.returnValue ?? "") };
    default:
      return {};
  }
}
let booted = false;

/**
 * Bind each authorised handler to each of its elements not bound yet.
 *
 * At boot, every element the document has. After a block a signal decides
 * is rendered again (ADR-0130), what it made: its old elements are gone with
 * their listeners, and the elements still standing keep theirs.
 */
function bindEvents() {
  for (const part of parts.parts ?? []) {
    if (part.kind !== "event") continue;
    // Every INSTANCE of this template position. A template-scoped id names a
    // position in the template; the document has one instance per loop item,
    // and all three Add buttons carry `data-pw="0"`.
    const addresses = addressesFor(`e${part.owner}`);
    const owners = addresses.map((a) => index.get(a).element).filter(Boolean);
    if (owners.length === 0) {
      // A part in a block not rendered now (ADR-0130) is bound when it is.
      if (!booted) log.push(`no element ${part.owner} for part ${part.id}`);
      continue;
    }

    // The decision, before anything is bound. `decide` returning anything but
    // "attach" leaves the element without a listener — the button is inert
    // rather than wrong.
    //
    // Asked NOW rather than on interaction, deliberately. A refused handler
    // must never load its code: fetching first and checking after would make
    // the refusal a formality the network had already ignored.
    let verdict = verdicts.get(part.id);
    if (!verdict) {
      const manifest = manifestFor(part);
      verdict =
        decide && manifest ? decide(manifest) : { attach: false, recovery: "no-decision" };
      verdicts.set(part.id, verdict);
      if (!verdict.attach) {
        // The CODE as well as the recovery. "refused 5: none" says a handler
        // was refused and nothing about why; the code is the one field that
        // distinguishes an unknown handler from a widened privacy scope from
        // a stale document schema.
        log.push(`refused ${part.id}: code ${verdict.code} recovery ${verdict.recovery}`);
      }
    }
    if (!verdict.attach) {
      recoverOnPress(part, owners, verdict);
      continue;
    }

    // The event the part handles, and what to do before any code loads.
    const event = part.event || "press";
    const listens = DOM_EVENTS[event];
    if (!listens) {
      log.push(`no listener for event ${event} of part ${part.id}`);
      continue;
    }
    const modifiers = part.modifiers ?? [];

    let fresh = 0;
    for (const el of owners) {
      // Bound once: a block a signal renders again binds what it made, and
      // leaves what was bound before alone (ADR-0130).
      const parts = bound.get(el) ?? new Set();
      if (parts.has(part.id)) continue;
      parts.add(part.id);
      bound.set(el, parts);
      fresh++;
      el.addEventListener(listens, async (e) => {
        // Declared, so done now, synchronously (ADR-0131).
        if (modifiers.includes("prevent")) e.preventDefault();
        if (modifiers.includes("stop")) e.stopPropagation();
        lastActed = el;
        const record = eventRecord(event, e);
        // Which instance each signal the handler names is, for this use of
        // its view (ADR-0144). Read at the press: the element is the one the
        // block rendered last.
        const instances = JSON.parse(el.dataset.pwSignals ?? "{}");
        const instance = (name) => instances[name] ?? name;
        // **Presses run in the order they were made** (ADR-0152). Each
        // handler's module loads on its own, so a second press's could arrive
        // first; its handler waits for the press before it to start. Until
        // 2026-10-03 it did not wait: Hot then Cold, pressed quickly, could
        // leave Hot chosen.
        const myTurn = lastPressStarted;
        let started;
        lastPressStarted = new Promise((resolve) => (started = resolve));
        try {
          // Authorised above; loaded here. The order is the point of E7-L:
          // the bytes for this handler do not exist in this page until
          // somebody presses this button.
          let module;
          try {
            module = await loadHandler(part.value);
          } finally {
            // Its turn, loaded or not: a press whose code failed to load
            // must not hold up every press after it.
            await myTurn;
            started();
          }
          // The command COMMITS and returns nothing about the cart. The
          // browser learns the new value from the RESOURCE, because that is
          // what the program declares the page depends on:
          //
          //   command add_to_cart(..) invalidates Cart(current_session())
          //
          // A response carrying the value would make the UI change because an
          // endpoint said so, which is the thing E6 exists to replace.
          //
          // The module is the handler's compiled BODY (E10, 2026-09-25). It
          // reads what it captured from this element, where the renderer
          // serialized exactly the paths it reads, and calls its command with
          // the arguments it computed.
          // `getRandomValues`, not `randomUUID`: the second exists only in a
          // secure context, and a press must have an interaction everywhere.
          const press = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) =>
            b.toString(16).padStart(2, "0"),
          ).join("");
          let calls = 0;
          await module.run({
            captures: JSON.parse(el.dataset.pwCaptures ?? "{}"),
            // What the event was, read in the listener (ADR-0138).
            event: record,
            // The page's signals (ADR-0130), as JSON carries them, and where
            // a change came from (ADR-0142).
            // Through the instance this element's use of a view holds each
            // signal as, where the compiler wrote one (ADR-0144).
            get: (name) => signals.get(instance(name)),
            set: (name, value) => setSignal(instance(name), value, { el, event }),
            // Each command speculates before its request (ADR-0122), and its
            // speculation is resolved by the answer, whatever it is.
            command: async (component, args) => {
              const ids = await speculate(component, args);
              let answer;
              try {
                answer = await command(component, args, `${press}-${calls++}`);
              } catch (error) {
                await resolveSpeculation(ids, false);
                throw error;
              }
              await resolveSpeculation(ids, answer.committed === true, answer.basis);
              // What the command answered, as its handler's code reads it
              // (ADR-0157): `Ok`, or a declared `Err` the handler can show.
              // A command that trapped or was refused answered no value, and
              // the press fails visibly.
              if (!("result" in answer)) {
                throw new Error(`${component} answered no value`);
              }
              return answer.result;
            },
          });
        } catch (error) {
          // A load or a refused command is VISIBLE and leaves the button
          // usable. A silent failure here is the worst outcome available: the
          // press did nothing, the page looks fine, and the next press is the
          // user's only way to find out.
          loaded.delete(part.value);
          attempts.set(part.value, (attempts.get(part.value) ?? 0) + 1);
          el.dataset.pwHandlerError = "1";
          log.push(`handler ${part.value} failed: ${error.message ?? error}`);
          window.__pw.handlerErrors = (window.__pw.handlerErrors ?? 0) + 1;
        }
      });
    }
    if (fresh > 0) {
      log.push(
        `attached ${part.id} to ${owners.length} instance(s): ${addresses.join(" ")}`,
      );
    }
  }

}

async function attach() {
  // Marked, so activation cost is a MEASUREMENT rather than a wall-clock guess
  // taken from outside. `performance.measure` attributes the work to this
  // runtime; a stopwatch around navigation would also count the network, the
  // parser and the paint.
  performance.mark("pw:activate:start");

  // One traversal, before anything else. Every later lookup is a map hit.
  const indexed = buildIndex();
  log.push(`indexed ${indexed} address(es)`);

  await bootDecision();

  bindEvents();
  booted = true;
  // A dialog the server rendered at a signal's first value is shown as one
  // the browser renders is (ADR-0141).
  for (const live of parts.live ?? []) {
    if (live.kind !== "conditional" && live.kind !== "match") continue;
    const r = index.get(addressOf([], live.part));
    if (r?.start) showDialogs(between(r.start, r.end));
  }

  performance.mark("pw:activate:end");
  performance.measure("pw:activate", "pw:activate:start", "pw:activate:end");

  // The regions still pending, filled as the response goes on (ADR-0148).
  watchStreams();

  // A page whose values are its signals alone listens for nothing
  // (ADR-0130): no resource of its changes on the server.
  if (parts.listens !== false) subscribe();
  window.__pw.signals = Object.fromEntries(signals);
  window.__pw.ready = true;
  document.documentElement.dataset.pwReady = "1";
}

// --- the protocol ---------------------------------------------------------
//
// The browser knows five things: `ResourceEntryId`, `Version`, `PartAddress`,
// `StreamFrame` and `Patch`. It does not know `EntryKey`, SQLite, outbox rows,
// compiler graph nodes or anything else the server owns.
//
// Held versions are per ENTRY, not per page: `Store(47)` at 8 and
// `Cart(session_A)` at 14 are two independent facts, and one page version
// would make either one lie.

/**
 * ResourceEntryId → the version the DOCUMENT reflects.
 *
 * Advanced only when a patch is applied. Hearing that newer state exists is a
 * different fact and lives in `known`.
 */
const held = new Map();

/** ResourceEntryId → the newest version the server has told us about. */
const known = new Map();

/** The addresses the last frame batch updated, for the tests to read. */
window.__pw.updated = [];

/**
 * Would applying this basis move any dependency backward?
 *
 * Asked over EVERY entry, because a patch derived from a fresh cart and a
 * stale promotion would show a total computed from state the page has already
 * moved past. An empty basis has no causal claim and is refused: "no basis" is
 * not "any basis".
 */
function isNewer(basis) {
  const resources = basis?.resources ?? [];
  if (resources.length === 0) return false;
  let advances = false;
  for (const r of resources) {
    const current = held.get(r.entry);
    if (current === undefined) {
      advances = true;
    } else if (r.version < current) {
      return false;
    } else if (r.version > current) {
      advances = true;
    }
  }
  return advances;
}

/** The index key for a `PartAddress`, matching `pw_document::PartAddress::key`. */
function addressKey(address) {
  const path = (address.instances ?? []).map((f) => `${f.scope}@${f.instance}`);
  return `${address.template}/${path.join("/")}|${address.part}`;
}

/**
 * **An attribute a change reaches** (ADR-0168): the element the part
 * belongs to, by its owner in the parts manifest, in the instance the
 * address names. The value is as the document's HTML writes it, and the
 * platform's own parser reads it, so a patched attribute means what a
 * rendered one does. Until 2026-10-03 no patch set an attribute: an Add button
 * kept the name of an item renamed since.
 */
function setAttributeAt(address, op) {
  const part = (parts.parts ?? []).find((p) => p.id === address.part);
  if (part?.owner == null) return false;
  const path = (address.instances ?? []).map((f) => `${f.scope}@${f.instance}`);
  const element = index.get(`${address.template}/${path.join("/")}|e${part.owner}`)?.element;
  if (!element) return false;
  if (op.op === "remove_attribute") {
    element.removeAttribute(op.name);
    return true;
  }
  const parsed = document.createElement("template");
  parsed.innerHTML = `<i ${op.name}="${op.value}"></i>`;
  element.setAttribute(op.name, parsed.content.firstChild.getAttribute(op.name) ?? "");
  return true;
}

const isAttributeOp = (op) => op.op === "set_attribute" || op.op === "remove_attribute";

function applyFrame(frame) {
  if (frame.protocol !== undefined && frame.protocol !== 1) {
    // Incomparable, not different. A frame from another protocol version is
    // refused before anything it contains is interpreted.
    log.push(`refused frame: protocol ${frame.protocol}`);
    return;
  }
  switch (frame.frame) {
    case "resource_changed":
      // A NOTICE, not an application.
      //
      // `held` is what the DOCUMENT reflects, and hearing that newer state
      // exists does not make the document reflect it. A first version advanced
      // `held` here, and then the patch that realized the same version
      // advanced nothing and was refused as stale — the page stayed at 0 while
      // both frames arrived and both were "handled".
      //
      // What a notice is for: a subscriber with no patch coming may refetch,
      // and one that has already applied a patch learns nothing new.
      known.set(frame.entry, frame.version);
      log.push(`resource ${frame.entry.slice(0, 8)} at ${frame.version}`);
      return;

    case "patch": {
      if (!isNewer(frame.basis)) {
        window.__pw.ignored = (window.__pw.ignored ?? 0) + 1;
        log.push(`ignored a patch that advances nothing`);
        return;
      }
      for (const r of frame.basis.resources) held.set(r.entry, r.version);
      const key = addressKey(frame.target);
      const op = frame.operation;
      const at = frame.basis.resources.map((r) => r.version).join(",");

      if (op.op === "replace_text") {
        if (setRange(key, op.text)) {
          window.__pw.updated.push(key);
          log.push(`updated ${key} at version ${at}`);
        }
        return;
      }
      if (isAttributeOp(op)) {
        if (setAttributeAt(frame.target, op)) {
          window.__pw.updated.push(`${key}:${op.name}`);
          log.push(`${op.op} ${op.name} on ${key} at version ${at}`);
        }
        return;
      }

      // Structural operations on a keyed collection. The target names the LOOP;
      // `op.instance` names which of its instances.
      const scope = frame.target.part;
      if (applyList(key, scope, op)) {
        // The index maps addresses to nodes, and a structural change moves
        // nodes. Rebuilt rather than patched incrementally: an index that
        // tracked its own edits is a second model of the document, and the
        // document is right here to be read.
        buildIndex();
        window.__pw.updated.push(`${key}:${op.op}`);
        log.push(`${op.op} on ${key} at version ${at}`);
      } else {
        log.push(`refused ${op.op}: no instance ${op.instance}`);
        window.__pw.refused = (window.__pw.refused ?? 0) + 1;
      }
      return;
    }

    case "patch_set": {
      // **One change, every place it reaches** (ADR-0145): applied in order,
      // and the version held once all of them applied.
      if (!isNewer(frame.basis)) {
        window.__pw.ignored = (window.__pw.ignored ?? 0) + 1;
        log.push(`ignored a patch set that advances nothing`);
        return;
      }
      const at = frame.basis.resources.map((r) => r.version).join(",");
      for (const { target, operation: op } of frame.patches) {
        const key = addressKey(target);
        const applied =
          op.op === "replace_text"
            ? setRange(key, op.text)
            : op.op === "replace_range"
              ? replaceRangeAt(key, op.html) !== null
              : isAttributeOp(op)
                ? setAttributeAt(target, op)
                : applyList(key, target.part, op);
        if (!applied) {
          // The document is not what the server derived the change from, and
          // every later change would be derived from it too. Never "try
          // anyway": the version is not held, and a fresh document comes with
          // a fresh queue, as a `reload` recovery does.
          window.__pw.refused = (window.__pw.refused ?? 0) + 1;
          log.push(`refused ${op.op} on ${key}: the document disagrees; reloading`);
          if (!window.__pw.reloading) {
            window.__pw.reloading = true;
            location.reload();
          }
          return;
        }
        if (op.op === "replace_text") {
          window.__pw.updated.push(key);
          log.push(`updated ${key} at version ${at}`);
        } else if (isAttributeOp(op)) {
          window.__pw.updated.push(`${key}:${op.name}`);
          log.push(`${op.op} ${op.name} on ${key} at version ${at}`);
        } else {
          // A structural change moves nodes: the index is read again, so the
          // next patch in the set finds what this one put in place.
          buildIndex();
          window.__pw.updated.push(`${key}:${op.op}`);
          log.push(`${op.op} on ${key} at version ${at}`);
        }
      }
      for (const r of frame.basis.resources) held.set(r.entry, r.version);
      // What a change inserted may be pressed.
      bindEvents();
      return;
    }

    case "entry_value":
      entryValue(frame);
      return;

    case "recovery":
      // Never "try anyway".
      log.push(`recovery: ${JSON.stringify(frame.recovery)}`);
      // A reload is the one recovery the page performs by itself. The server
      // sends it when it can no longer say what this document missed: the
      // subscriber fell too far behind, or was forgotten while idle (E10 gate
      // item 3). Applying later frames to this document would be exactly
      // "try anyway"; a fresh document comes with a fresh queue. Once, so a
      // second reload frame in the same batch does not queue a second one.
      if (frame.recovery?.recovery === "reload" && !window.__pw.reloading) {
        window.__pw.reloading = true;
        location.reload();
      }
      return;

    default:
      log.push(`unknown frame`);
  }
}

/**
 * Where this page is in its subscriber's frame sequence.
 *
 * Carried by the document rather than starting at zero. A reloaded page's
 * document already contains every change committed before it was rendered, so
 * asking from zero would replay them — and replaying a structural patch is not
 * a no-op: an item would be inserted twice.
 */
let cursor = parts.cursor ?? 0;

/**
 * Apply one delivered batch.
 *
 * Every adapter ends here, and this function has no idea which one called it.
 * That is the whole design: `pw-protocol` names no transport, so a transport
 * cannot smuggle meaning into a frame — and a second adapter is how that stops
 * being a claim and becomes something a test can fail.
 */
function applyBatch(batch) {
  window.__pw.updated = [];
  for (const frame of batch.frames ?? []) applyFrame(frame);
  reapplySpeculations();
  // Advanced only AFTER applying. Advancing on receipt would acknowledge
  // frames a mid-batch failure never applied, and the server would drop them.
  cursor = batch.cursor ?? cursor;
  window.__pw.cursor = cursor;
}

/**
 * The long poll: one batch per request.
 *
 * The cursor is BOTH the request and the acknowledgement — asking for what
 * follows N tells the server that N arrived and was applied. A response that
 * never reaches this page costs nothing, because the frames are still there
 * for the next ask.
 */
async function pollOnce() {
  // Which document this page is (ADR-0161): each is its own subscriber.
  const response = await fetch(`/stream?doc=${documentCursor}&since=${cursor}`);
  applyBatch(await response.json());
}

/**
 * The streaming adapter: one connection, many batches.
 *
 * Newline-delimited JSON, read incrementally. A partial line is held rather
 * than parsed — a batch split across two network chunks is the normal case,
 * not an error, and parsing half of one would apply half a change.
 */
async function streamOnce() {
  const response = await fetch(`/stream?doc=${documentCursor}&since=${cursor}&mode=stream`);
  if (!response.body) throw new Error("no streaming body");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let held = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    held += decoder.decode(value, { stream: true });
    const lines = held.split("\n");
    held = lines.pop() ?? "";
    for (const line of lines) {
      if (line) applyBatch(JSON.parse(line));
    }
  }
}

/**
 * Which adapter to use.
 *
 * Streaming where the engine supports a readable response body, long poll
 * otherwise — and long poll again if streaming fails once, because a transport
 * that cannot connect is not a reason to stop subscribing. `?transport=` lets
 * a test pin one, so "both adapters deliver the same frames" is measured
 * rather than assumed.
 */
function chosenTransport() {
  const pinned = new URLSearchParams(location.search).get("transport");
  if (pinned === "poll") return "poll";
  if (pinned === "stream") return "stream";
  return typeof ReadableStream === "function" ? "stream" : "poll";
}

/** The page is going away: its requests fail because it is, and nothing is
 * asked again. */
let leaving = false;
addEventListener("pagehide", () => {
  leaving = true;
});

/**
 * **The subscription, kept** (charter §15.5's forced reconnect). A failed
 * request is a dropped connection, a server restarting or a network that
 * blinked, and it is asked again, from the cursor this page holds, after a
 * pause that grows to five seconds. Until 2026-10-03 a failure ended the
 * subscription for good: its fallback to the long poll waited on
 * `chosenTransport()` answering differently, which it never does, and the page
 * stopped hearing changes without a word.
 */
async function subscribe() {
  let transport = chosenTransport();
  const pinned = new URLSearchParams(location.search).has("transport");
  window.__pw.transport = transport;
  let failures = 0;
  for (;;) {
    try {
      if (transport === "stream") await streamOnce();
      else await pollOnce();
      failures = 0;
    } catch {
      if (leaving) return;
      failures += 1;
      // A stream that fails twice in a row falls back to the long poll, and
      // says so: a silent fallback would make the streaming adapter
      // untestable. A test that pins the adapter keeps it.
      if (transport === "stream" && failures >= 2 && !pinned) {
        transport = "poll";
        window.__pw.transport = "poll (fell back)";
        log.push("transport fell back to long poll");
      }
      const pause = Math.min(250 * 2 ** (failures - 1), 5000);
      window.__pw.reconnects = (window.__pw.reconnects ?? 0) + 1;
      log.push(`subscription failed; asking again in ${pause} ms`);
      await new Promise((resolve) => setTimeout(resolve, pause));
      if (leaving) return;
    }
  }
}

// Exposed so a test can deliver a frame out of order. A transport cannot be
// made to do that on demand, and the property is about the page's guard rather
// than about the transport.
window.__pwTestApply = applyFrame;
window.__pwHeld = () => Object.fromEntries(held);
window.__pwKnown = () => Object.fromEntries(known);

/** Update every part whose value the resource carries. */
function apply(values) {
  window.__pw.updated = [];
  for (const p of parts.parts ?? []) {
    if (p.kind !== "text") continue;
    if (!(p.value in values)) continue;
    for (const address of addressesFor(String(p.id))) {
      if (setRange(address, String(values[p.value]))) {
        window.__pw.updated.push(address);
      }
    }
  }
  if (window.__pw.updated.length) {
    log.push(`updated ${window.__pw.updated.join(",")} at version ${values.version}`);
  }
}

attach().catch((e) => {
  log.push(`boot failed: ${e}`);
  // Deliberately NOT re-thrown into a state where handlers attach anyway. A
  // runtime that failed to boot leaves an inert page, which is a page that
  // works for reading and does nothing surprising.
  document.documentElement.dataset.pwReady = "failed";
});
