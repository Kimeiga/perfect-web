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
  const { memory, alloc, decide_manifest, last_recovery } = instance.exports;

  const write = (s) => {
    const bytes = new TextEncoder().encode(s);
    const ptr = alloc(bytes.length);
    new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
  };
  // `last_recovery` returns an INDEX into this list, not a pointer. A first
  // version read it as one and every decision threw `Start offset -1 is
  // outside the bounds of the buffer`, which the page reported as "boot
  // failed" — so no handler attached and the button was inert.
  //
  // Worth recording: fail-closed did its job. A bug in the code that READS the
  // decision produced a page that does nothing, not a page that attaches
  // anyway. That is the difference the design was for.
  const RECOVERY = [
    "none",
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

function manifestFor() {
  const r = parts.resume;
  if (!r) return null;
  return RESUME_FIELDS.map((f) => r[f] ?? "").join("|");
}

async function attach() {
  // One traversal, before anything else. Every later lookup is a map hit.
  const indexed = buildIndex();
  log.push(`indexed ${indexed} address(es)`);

  await bootDecision();

  for (const part of parts.parts ?? []) {
    if (part.kind !== "event") continue;
    // Every INSTANCE of this template position. A template-scoped id names a
    // position in the template; the document has one instance per loop item,
    // and all three Add buttons carry `data-pw="0"`.
    const addresses = addressesFor(`e${part.owner}`);
    const owners = addresses.map((a) => index.get(a).element).filter(Boolean);
    if (owners.length === 0) {
      log.push(`no element ${part.owner} for part ${part.id}`);
      continue;
    }

    // The decision, before anything is bound. `decide` returning anything but
    // "attach" leaves the element without a listener — the button is inert
    // rather than wrong.
    const manifest = manifestFor();
    const verdict =
      decide && manifest ? decide(manifest) : { attach: false, recovery: "no-decision" };
    if (!verdict.attach) {
      log.push(`refused ${part.id}: ${verdict.recovery}`);
      continue;
    }

    for (const [i, el] of owners.entries()) {
      const at = addresses[i];
      el.addEventListener("click", async (e) => {
        e.preventDefault();
        // The command COMMITS and returns nothing about the cart. The browser
        // learns the new value from the RESOURCE, because that is what the
        // program declares the page depends on:
        //
        //   command add_to_cart(..) invalidates Cart(current_session())
        //
        // A response carrying the value would make the UI change because an
        // endpoint said so, which is the thing E6 exists to replace.
        await fetch("/command/add_to_cart", { method: "POST" });

        void at;
      });
    }
    log.push(
      `attached ${part.id} to ${owners.length} instance(s): ${addresses.join(" ")}`,
    );
  }

  subscribe();
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

    case "recovery":
      // Never "try anyway".
      log.push(`recovery: ${JSON.stringify(frame.recovery)}`);
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
  const response = await fetch(`/stream?since=${cursor}`);
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
  const response = await fetch(`/stream?since=${cursor}&mode=stream`);
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

async function subscribe() {
  let transport = chosenTransport();
  window.__pw.transport = transport;
  for (;;) {
    try {
      if (transport === "stream") await streamOnce();
      else await pollOnce();
    } catch {
      if (transport === "stream" && chosenTransport() !== "stream") {
        // Fall back once, and say so. A silent fallback would make the
        // streaming adapter untestable: every run would look like whichever
        // one happened to work.
        transport = "poll";
        window.__pw.transport = "poll (fell back)";
        log.push("transport fell back to long poll");
        continue;
      }
      return; // the page is going away
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
