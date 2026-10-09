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

/** address → { element, template } or { start, end, template } */
const index = new Map();

/**
 * **Each template's parts, by its path** (ADR-0203): the page's, and each
 * one a view that contains itself renders an instance of. A part's id is
 * its template's, so an instance's parts are read in its own manifest, and
 * its button numbered as one of the page's is not taken for it.
 */
const PAGE = parts.template ?? "";
const manifests = new Map([
  [PAGE, parts.parts ?? []],
  ...Object.entries(parts.templates ?? {}).map(([path, t]) => [path, t.parts ?? []]),
]);
const entries = new Map(
  [...manifests].map(([path, list]) => [path, new Map(list.map((p) => [p.id, p]))]),
);

/** Each view's template, as JSON: what the browser's renderer renders an
 * instance in a block with (ADR-0203). Empty where the page has none. */
const TEMPLATES = Object.values(parts.templates ?? {}).map((t) => t.template);
const TEMPLATES_JSON = TEMPLATES.length ? JSON.stringify(TEMPLATES) : "";

/** A renderer's request, with each view's template where the page has one. */
function withTemplates(request) {
  return TEMPLATES_JSON ? `${request.slice(0, -1)},"templates":${TEMPLATES_JSON}}` : request;
}

function buildIndex() {
  index.clear();
  const path = [];
  // The template each frame's parts are in: a loop's row is its loop's, and
  // an instance is its view's (ADR-0203).
  const within = [PAGE];
  // A range being walked, by its path and its id: a view inside itself has
  // a part of the same id open around it (ADR-0203).
  const open = new Map();
  const at = () => path.map(([e, t]) => `${e}@${t}`).join("/");
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
        index.set(addressOf(path, `e${owner}`), { element: node, template: within.at(-1) });
      }
      continue;
    }
    const data = node.data ?? "";
    if (!data.startsWith("pw:")) continue;

    const m = /^pw:([se])(\d+)(?:@([A-Za-z0-9_-]+))?$/.exec(data);
    if (!m) continue;
    const [, side, id, token] = m;

    if (token) {
      // An INSTANCE boundary, a loop's row or a view's (ADR-0203): push a
      // frame, or pop it. A view's instance is a range too, of its part in
      // the template around it, which a change replaces whole.
      const template = within.at(-1);
      if (side === "s") {
        const part = entries.get(template)?.get(Number(id));
        const view = part?.kind === "instance";
        if (view) open.set(`${at()}|${id}@${token}`, { start: node, path: [...path], template });
        path.push([Number(id), token]);
        within.push(view ? part.value : template);
      } else {
        path.pop();
        within.pop();
        const key = `${at()}|${id}@${token}`;
        const started = open.get(key);
        if (started) {
          index.set(addressOf(started.path, id), {
            start: started.start,
            end: node,
            template: started.template,
          });
          open.delete(key);
        }
      }
      continue;
    }
    const key = `${at()}|${id}`;
    if (side === "s") {
      open.set(key, { start: node, path: [...path], template: within.at(-1) });
    } else {
      const started = open.get(key);
      if (started) {
        index.set(addressOf(started.path, id), {
          start: started.start,
          end: node,
          template: started.template,
        });
        open.delete(key);
      }
    }
  }
  // The page's title (ADR-0183): the document's, with no range in the body.
  for (const p of parts.parts ?? []) {
    if (p.kind === "title") index.set(addressOf([], p.id), { title: true });
  }
  return index.size;
}

/** Every address for a part id of `template`, the page's unless named,
 * across every instance of it. */
function addressesFor(part, template = PAGE) {
  const suffix = `|${part}`;
  return [...index]
    .filter(([k, v]) => k.endsWith(suffix) && (v.template ?? PAGE) === template)
    .map(([k]) => k);
}

/**
 * Replace a range's contents, leaving its anchors and everything else alone.
 *
 * The anchors are never touched, so the part keeps its identity across the
 * update — which is what lets it be updated again.
 *
 * A range that holds the text already is left as it is (ADR-0182). Text
 * written again, the same, is a change to a live region, and a screen reader
 * says the region again: one Add said "Items in cart: 1" up to four times,
 * written by its speculation, its answer and the cart's new value. Left as it
 * is, a selection in it stays too.
 */
function setRange(address, text) {
  const r = index.get(address);
  // The page's title is the document's (ADR-0183), set as it is shown.
  if (r?.title) {
    if (document.title !== text) document.title = text;
    return true;
  }
  if (!r?.start) return false;
  if (holdsText(r, text)) return true;
  let n = r.start.nextSibling;
  while (n && n !== r.end) {
    const next = n.nextSibling;
    n.remove();
    n = next;
  }
  r.end.parentNode.insertBefore(document.createTextNode(text), r.end);
  return true;
}

/** Whether a range holds `text`, and nothing but text. */
function holdsText(r, text) {
  let shown = "";
  for (let n = r.start.nextSibling; n && n !== r.end; n = n.nextSibling) {
    if (n.nodeType !== Node.TEXT_NODE) return false;
    shown += n.data;
  }
  return shown === text;
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
  /** A page's address from its route and its parameters' values, as a
   * navigation goes to it (ADR-0280). */
  pageAddress: (route, args) => pageAddress(route, args),
  indexSize: () => index.size,
  /** The bytes of command bodies kept alive and not yet answered, and what
   * they may come to (ADR-0268). */
  keepalive: () => ({ inFlight: keptAlive, budget: keepaliveBudget }),
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
 *
 * **Sent again where no answer came** (ADR-0173), as the command's `retry`
 * clause says, which the compiled handler passes: the request may not have
 * arrived, or its answer may have been lost after the server committed, and
 * nothing here can tell which. Sent again with the same interaction, a
 * command the server ran answers with its first outcome (ADR-0121). An
 * answer of any kind, a refusal included, is an outcome, and never sent
 * again.
 *
 * **Kept alive past the page** (ADR-0268): a press's speculation is shown
 * before its request leaves (ADR-0122), and a link followed at once ended the
 * page and the request with it, a press shown taken and never made. A request
 * marked `keepalive` outlives its document (the Fetch standard), within 64 KiB
 * of such bodies in flight; one past what remains is sent as before.
 */
async function command(component, args, interaction, retry) {
  const sent = JSON.stringify(args);
  const bytes = new Blob([sent]).size;
  for (let attempt = 0; ; attempt += 1) {
    let response;
    let body;
    const keepalive = keptAlive + bytes <= keepaliveBudget;
    if (keepalive) keptAlive += bytes;
    try {
      response = await fetch(`/command/${encodeURIComponent(component)}`, {
        method: "POST",
        // The interaction this request belongs to (ADR-0121). One per press
        // and per command the press calls, made here and never by the
        // handler, and carried unchanged by any retry of this request: a
        // command declared `idempotent_by InteractionId` runs once for it
        // however many times the request is sent.
        headers: { "content-type": "application/json", "pw-interaction": interaction },
        body: sent,
        keepalive,
      });
      // An answer cut off in transit is no answer.
      if (response.ok) body = await response.text();
    } catch (error) {
      if (keepalive) keptAlive -= bytes;
      if (!retry || attempt >= retry.max) throw new Unreachable(component, error);
      log.push(`resent ${component}: no answer (${error.message ?? error})`);
      await new Promise((resolve) => setTimeout(resolve, resendDelay(retry, attempt)));
      continue;
    }
    if (keepalive) keptAlive -= bytes;
    if (!response.ok) {
      // **Refused by its `requires`** (ADR-XXXX): the predicate and the words
      // a reader is told, which the host that refused sends.
      if (response.status === 403) {
        const refusal = await response.json().catch(() => null);
        if (refusal?.refused) throw new Refused(component, refusal.refused, refusal.says ?? "");
      }
      throw new Error(`command ${component} refused: HTTP ${response.status}`);
    }
    return JSON.parse(body);
  }
}

/**
 * The bytes of command bodies kept alive and not yet answered (ADR-0268),
 * and what they may come to: the Fetch standard refuses a `keepalive` request
 * that would take the page's past 64 KiB. `?keepalive=` lowers it, so a test
 * reaches the bound with the store's few bytes, as `?transport=` pins an
 * adapter; it never raises it.
 */
let keptAlive = 0;
const keepaliveBudget = Math.min(
  64 * 1024,
  Number(new URLSearchParams(location.search).get("keepalive") ?? 64 * 1024) || 0,
);

/**
 * How long to wait before sending a command again (ADR-0173): a second,
 * doubling with each attempt, as TanStack Query waits between retries
 * (`Math.min(1000 * 2 ** n, 30000)`), and drawn at random below that where
 * the clause says `jitter`, so that pages that lost one connection do not all
 * send again at once. A delay that did not double, `fixed`, left the language
 * with ADR-0215.
 */
function resendDelay(retry, attempt) {
  const bound = Math.min(1000 * 2 ** attempt, 30000);
  return retry.jitter ? Math.random() * bound : bound;
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

/** The page's speculation module, loaded with the first press that needs it,
 * and the renderer with it where a speculation renders a region (ADR-0172):
 * a region is rendered between a batch's frames, so both are ready first. */
function speculations() {
  if (!parts.speculation) return Promise.resolve(null);
  speculationModule ??= Promise.all([
    import(parts.speculation),
    parts.regions ? renderModule() : null,
  ]).then(([module, renderer]) => {
    if (renderer) wasm = renderer;
    speculationLoaded = module;
    return module;
  });
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
  showRegions(module, binding, value, state.value);
}

let nextSpeculation = 0;

/** Before a command's request: show each of its transitions this page can,
 * each by the interaction its request carries (ADR-0172). */
async function speculate(component, args, interaction) {
  const module = await speculations();
  const ids = [];
  for (const s of module?.commands?.[component] ?? []) {
    const state = current(module, s.binding);
    if (!state || state.value === undefined) continue;
    const id = nextSpeculation++;
    state.pending.push({ id, transition: s.transition, args, until: undefined, interaction });
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
    // A speculation the value includes is dropped (ADR-0172): by the
    // interaction that committed it, which the value names whether or not
    // the command's answer has come; or by the version its answer said.
    const applied = new Set(frame.applied ?? []);
    state.pending = state.pending.filter(
      (p) => !applied.has(p.interaction) && (p.until === undefined || p.until > frame.version),
    );
    if (before !== state.pending.length) {
      log.push(`reconciled ${binding} at version ${frame.version}`);
    }
  }
}

/** After a batch: a patch writes the authoritative text, and a speculation
 * still pending is shown over it again. A region is shown as what the page
 * holds when none is (ADR-0172): its list was shown as held before the
 * batch, and the batch's patches made it what the page holds now. */
async function reapplySpeculations() {
  if (!speculationModule) return;
  const module = await speculationModule;
  for (const [binding, state] of speculated) {
    if (state.pending.length > 0 || module.regions?.[binding]?.length) render(module, binding);
  }
}

// --- ADR-0172: what a speculation renders again --------------------------
//
// A speculation reaches every part that reads its value. Text at the top of
// the page is set by the module's own functions (ADR-0122). An attribute, a
// block and a loop's rows are rendered by the browser's copy of the server's
// renderer, from the speculated value, so the two agree by being one.
//
// A row the server rendered keeps its nodes, and focus with them: its parts
// are set where they are, and one a speculation takes away is detached, not
// destroyed. A row a speculation adds is the browser's, at an address the
// browser's renderer derives, until the server's insert replaces it: the
// server's addresses are a keyed hash under the deployment's key, which a
// page does not hold.
//
// Before a batch of the server's patches applies, each list shows what the
// page held again, so each patch finds the document it was derived from;
// after, the speculation is shown again. A change's patches and its value
// come in one batch (the server queues them in one hold), so the list's rows
// are then the held value's, in its order.

/** The renderer, and the speculation module, once loaded. */
let wasm = null;
let speculationLoaded = null;

/** One call into the renderer: the request as JSON, the answer as text. */
function renderCall(name, request) {
  const bytes = new TextEncoder().encode(withTemplates(request));
  const ptr = wasm.alloc(bytes.length);
  new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
  const code = wasm[name](ptr, bytes.length);
  const out = new TextDecoder().decode(
    new Uint8Array(wasm.memory.buffer, wasm.out_ptr(), wasm.out_len()),
  );
  if (code !== 0) throw new Error(`${name} did not render: ${out}`);
  return out;
}

/** A value as the renderer reads it: JSON, each `Int` written exactly. A
 * `BigInt` has no JSON form of its own, and a number past 2^53 has no exact
 * one. */
function wire(v) {
  if (typeof v === "bigint") return v.toString();
  if (v === null || v === undefined) return "null";
  if (Array.isArray(v)) return `[${v.map(wire).join(",")}]`;
  if (typeof v === "object") {
    return `{${Object.entries(v)
      .map(([k, x]) => `${JSON.stringify(k)}:${wire(x)}`)
      .join(",")}}`;
  }
  return JSON.stringify(v);
}

/** The list at `collection`, `cart.lines`, in its binding's value. */
function listAt(value, collection) {
  let v = value;
  for (const field of collection.split(".").slice(1)) v = v?.[field];
  return Array.isArray(v) ? v : [];
}

/** A row's key, at its path from the row: `item_id`. */
function rowKey(item, path) {
  let v = item;
  for (const field of path.split(".").filter(Boolean)) v = v?.[field];
  return String(v);
}

/** A row as the renderer reads it: its item, and what the row reads of it
 * through a member, set whole by its path (ADR-0170). */
function rowOf(region, item) {
  const row = { ...item };
  for (const [path, read] of Object.entries(region.rows ?? {})) row[path] = read(item);
  return row;
}

/** What a region is rendered with: the value, what the module computes from
 * it, the page's parameters, and its signals. */
function regionValues(module, binding, value) {
  // And each value the region computes from it (ADR-0235): a block's
  // subject, and a value inside one, by the path the template reads.
  const computed = Object.entries(module.computed?.[binding] ?? {}).map(([path, f]) => [
    path,
    f(value),
  ]);
  // And the page's parameters (ADR-0236), which the document carries.
  const params = Object.entries(parts.params ?? {});
  return `{${[[binding, value], ...computed, ...params, ...signals]
    .map(([k, v]) => `${JSON.stringify(k)}:${wire(v)}`)
    .join(",")}}`;
}

/** The addresses of a loop's instances, in the document's order. */
function instanceTokens(part) {
  const loop = index.get(addressOf([], part));
  const open = new RegExp(`^pw:s${part}@([A-Za-z0-9_-]+)$`);
  const out = [];
  for (let n = loop?.start?.nextSibling; n && n !== loop.end; n = n.nextSibling) {
    const m = n.nodeType === Node.COMMENT_NODE && open.exec(n.data);
    if (m) out.push(m[1]);
  }
  return out;
}

/**
 * loop part → its rows as the document shows them: each one's key, its
 * address, the item it shows, whether the server rendered it, and its nodes
 * while a speculation takes it away. Read from the document the first time a
 * speculation shows the list, and again after each batch.
 */
const shownLists = new Map();

function listRows(region, held) {
  let state = shownLists.get(region.part);
  if (!state) {
    const tokens = instanceTokens(region.part);
    state = {
      rows: listAt(held, region.collection).map((item, i) => ({
        key: rowKey(item, region.key),
        token: tokens[i],
        shown: item,
        own: true,
        detached: null,
      })),
    };
    shownLists.set(region.part, state);
  }
  return state;
}

/** A row's nodes, its anchors included. */
function rowNodes(part, token) {
  const range = instanceRange(part, token);
  return range ? instanceNodes(range) : [];
}

/**
 * **Where focus goes when a row that has it goes** (ADR-0172): to the same
 * control in the row after it, else in the row before it, else to what
 * labels the region the list is in (WCAG 2.4.3). Until 2026-10-03 it went to
 * the document's body, and a keyboard user had to find the cart again.
 */
function passFocus(nodes, part) {
  const active = document.activeElement;
  if (!active || !nodes.some((n) => n === active || n.contains?.(active))) return;
  const owner = active.closest("[data-pw]")?.dataset.pw;
  const open = new RegExp(`^pw:s${part}@`);
  const isOpen = (n) => n?.nodeType === Node.COMMENT_NODE && open.test(n.data);
  const control = (start) => {
    const close = `pw:e${start.data.slice(`pw:s`.length)}`;
    for (let n = start.nextSibling; n && !(n.nodeType === Node.COMMENT_NODE && n.data === close); n = n.nextSibling) {
      if (n.nodeType !== Node.ELEMENT_NODE) continue;
      const found = n.dataset?.pw === owner ? n : n.querySelector?.(`[data-pw="${owner}"]`);
      if (found) return found;
    }
    return null;
  };
  let after = nodes[nodes.length - 1]?.nextSibling;
  while (after && !isOpen(after)) after = after.nextSibling;
  let before = nodes[0]?.previousSibling;
  while (before && !isOpen(before)) before = before.previousSibling;
  const labelled = nodes[0]?.parentElement?.closest("[aria-labelledby]");
  const label = labelled && document.getElementById(labelled.getAttribute("aria-labelledby"));
  const target = (after && control(after)) || (before && control(before)) || label;
  target?.focus();
}

/** What changed in a row the document keeps, set where it is; the row
 * rendered again, at its own address, where more than its text and its
 * attributes changed. */
function setRow(region, row, item, values) {
  const template = JSON.stringify(parts.regions[region.part]);
  const changes = JSON.parse(
    renderCall(
      "instance_changes",
      `{"part":${template},"was":${wire(rowOf(region, row.shown))},"now":${wire(rowOf(region, item))},"values":${values}}`,
    ),
  );
  if (changes === null) {
    const html = renderCall(
      "render_instance",
      `{"part":${template},"item":${wire(rowOf(region, item))},"values":${values}}`,
    );
    const fresh = /^<!--pw:s\d+@([A-Za-z0-9_-]+)-->/.exec(html)?.[1];
    const nodes = rowNodes(region.part, row.token);
    const replaced = parseInstance(fresh ? html.replaceAll(`@${fresh}-->`, `@${row.token}-->`) : html);
    nodes[0].before(...replaced);
    for (const n of nodes) n.remove();
    buildIndex();
    return;
  }
  for (const [part, change] of changes) {
    const at = { template: parts.schema, instances: [{ scope: region.part, instance: row.token }], part };
    if ("text" in change) {
      setRange(addressOf([[region.part, row.token]], part), change.text);
    } else {
      setAttributeAt(
        at,
        change.value === null
          ? { op: "remove_attribute", name: change.attribute }
          : { op: "set_attribute", name: change.attribute, value: change.value },
      );
    }
  }
}

/** **A loop's rows, as `items` makes them** (ADR-0172): each row the
 * document keeps set where it is, each it does not taken away, and each new
 * one rendered by the browser, in the items' order. Focus in a row taken
 * away passes on, unless the page is only showing what it holds before a
 * batch, after which focus goes back to its row (`refocus`). */
function showList(region, items, values, held, passing = true) {
  const loop = index.get(addressOf([], region.part));
  if (!loop?.start) return;
  const state = listRows(region, held);
  const wanted = items.map((item) => ({ key: rowKey(item, region.key), item }));
  const keys = new Set(wanted.map((w) => w.key));
  for (const row of state.rows) {
    if (keys.has(row.key) || row.detached) continue;
    const nodes = rowNodes(region.part, row.token);
    if (passing) passFocus(nodes, region.part);
    for (const n of nodes) n.remove();
    if (row.own) row.detached = nodes;
    else row.gone = true;
  }
  state.rows = state.rows.filter((row) => !row.gone);
  buildIndex();
  let after = loop.start;
  const shown = [];
  for (const w of wanted) {
    let row = state.rows.find((r) => r.key === w.key);
    if (!row) {
      const html = renderCall(
        "render_instance",
        `{"part":${JSON.stringify(parts.regions[region.part])},"item":${wire(rowOf(region, w.item))},"values":${values}}`,
      );
      const nodes = parseInstance(html);
      after.after(...nodes);
      const token = /^pw:s\d+@([A-Za-z0-9_-]+)$/.exec(nodes[0]?.data ?? "")?.[1];
      row = { key: w.key, token, shown: w.item, own: false, detached: null };
      state.rows.push(row);
      buildIndex();
    } else {
      if (row.detached) {
        after.after(...row.detached);
        row.detached = null;
        buildIndex();
      }
      const nodes = rowNodes(region.part, row.token);
      if (nodes[0] && nodes[0] !== after.nextSibling) {
        moveNodes(after.parentNode, nodes, after.nextSibling);
        buildIndex();
      }
      if (wire(rowOf(region, row.shown)) !== wire(rowOf(region, w.item))) {
        setRow(region, row, w.item, values);
      }
      row.shown = w.item;
    }
    shown.push(row);
    const nodes = rowNodes(region.part, row.token);
    after = nodes[nodes.length - 1] ?? after;
  }
  state.rows = [...shown, ...state.rows.filter((r) => r.detached)];
  bindEvents();
}

/** The last rendering of each block a speculation renders, so one rendered
 * as it is is not replaced, and its nodes kept. */
const shownBlocks = new Map();

/** **Every region that reads `binding`, at `value`** (ADR-0172). */
function showRegions(module, binding, value, held) {
  const regions = module.regions?.[binding] ?? [];
  if (regions.length === 0 || !wasm) return;
  const values = regionValues(module, binding, value);
  for (const region of regions) {
    const template = JSON.stringify(parts.regions?.[region.part]);
    if (region.kind === "list") {
      showList(region, listAt(value, region.collection), values, held);
    } else if (region.kind === "block") {
      const html = renderCall("render_part", `{"part":${template},"values":${values}}`);
      if (shownBlocks.get(region.part) !== html) {
        shownBlocks.set(region.part, html);
        replaceBlock(region.part, html);
        buildIndex();
        bindEvents();
      }
    } else if (region.kind === "captures") {
      // What a handler at the top of the page captures (ADR-0217): its
      // element's captures, as the speculated value makes them, so a press
      // meanwhile sends what the page shows.
      const written = JSON.parse(
        renderCall("captures_value", `{"parts":${template},"values":${values}}`),
      );
      setAttributeAt(
        { template: parts.schema, instances: [], part: region.part },
        written === null
          ? { op: "remove_attribute", name: "data-pw-captures" }
          : { op: "set_attribute", name: "data-pw-captures", value: written },
      );
    } else if (region.kind === "attribute") {
      const [name, written] = JSON.parse(
        renderCall("attribute_value", `{"part":${template},"values":${values}}`),
      );
      setAttributeAt(
        { template: parts.schema, instances: [], part: region.part },
        written === null
          ? { op: "remove_attribute", name }
          : { op: "set_attribute", name, value: written },
      );
    }
  }
}

/** The row of a speculated list that has focus, and the control in it:
 * what focus goes back to once a batch has been applied (ADR-0172). */
function focusedRow() {
  const active = document.activeElement;
  if (!active || active === document.body) return null;
  for (const [part, state] of shownLists) {
    for (const row of state.rows) {
      if (row.detached) continue;
      if (rowNodes(part, row.token).some((n) => n === active || n.contains?.(active))) {
        return { part, key: row.key, owner: active.closest("[data-pw]")?.dataset.pw };
      }
    }
  }
  return null;
}

/** Focus back on its row's control, where the batch took it away; on what
 * labels the list's region where the row is gone. */
function refocus(focused) {
  if (!focused) return;
  const active = document.activeElement;
  if (active && active !== document.body && active.isConnected) return;
  const row = shownLists.get(focused.part)?.rows.find((r) => r.key === focused.key && !r.detached);
  if (!row) {
    const loop = index.get(addressOf([], focused.part));
    const labelled = loop?.start?.parentElement?.closest("[aria-labelledby]");
    document.getElementById(labelled?.getAttribute("aria-labelledby") ?? "")?.focus();
    return;
  }
  for (const n of rowNodes(focused.part, row.token)) {
    if (n.nodeType !== Node.ELEMENT_NODE) continue;
    const control =
      n.dataset?.pw === focused.owner ? n : n.querySelector?.(`[data-pw="${focused.owner}"]`);
    if (control) {
      control.focus();
      return;
    }
  }
}

/** **Each list shown as the page holds it, before a batch's patches apply**
 * (ADR-0172), and read from the document again after them. */
function revertLists() {
  if (!speculationLoaded || !wasm) return;
  for (const [binding] of speculated) {
    const state = current(speculationLoaded, binding);
    if (!state || state.value === undefined) continue;
    const values = regionValues(speculationLoaded, binding, state.value);
    for (const region of speculationLoaded.regions?.[binding] ?? []) {
      if (region.kind === "list" && shownLists.has(region.part)) {
        showList(region, listAt(state.value, region.collection), values, state.value, false);
      }
    }
  }
  shownLists.clear();
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

/** **What the page computes from its signals now** (ADR-0229): each value,
 * by the path the compiler names it, for a block the browser renders again,
 * whose subject may be one. None where the page computes none. */
async function computedNow() {
  const computed = (parts.live ?? []).filter((l) => l.derived);
  if (computed.length === 0) return [];
  const module = await computedValues();
  return computed.map((l) => [
    `#${parts.template}~${l.part}`,
    module.parts[l.part](signalAt(l.path)),
  ]);
}

/** The block numbered `id`, rendered at the signals' values now, and what
 * the page computes from them. */
async function renderBlock(id) {
  const x = await renderModule();
  const values = [...signals, ...(await computedNow())]
    .map(([k, v]) => `${JSON.stringify(k)}:${wire(v)}`)
    .join(",");
  const request = new TextEncoder().encode(
    withTemplates(`{"part":${JSON.stringify(parts.blocks[id])},"values":{${values}}}`),
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

/** **The stream parts pending when the page was first indexed** (ADR-0272):
 * the runtime boots across the network, and a browser that streams a region
 * itself can fill one meanwhile. Until ADR-0272 such a region was indexed
 * pending, found settled after boot, and never read again: its buttons were
 * bound to nothing, and a press on one did nothing. */
const pendingAtIndex = new Set();

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

/** Apply each `<template for>` the browser left in the document, once it has
 * all arrived: the region's contents, between its anchors, become the
 * template's. Returns the parts it filled.
 *
 * A template is whole once the parser has put a node after it, the comment
 * the server writes after each one, or has read the whole document (ADR-0223).
 * Until 2026-10-05 one was applied as soon as it was seen, and a response that
 * arrived in parts gave the region the part of it parsed so far: the
 * recommendations showed one item of two, and the second was lost with the
 * template. The platform's own streaming applies one at its end tag. */
function applyStreamPatches() {
  const applied = new Set();
  for (const t of document.querySelectorAll("template[for]")) {
    if (!t.nextSibling && document.readyState === "loading") continue;
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
  // A region that settled while the runtime booted, which only the browser
  // fills: read again and bound, as one settling later is.
  const whileBooting = [...pendingAtIndex].filter((id) => !pendingStreams.has(id));
  if (whileBooting.length > 0) {
    for (const id of whileBooting) window.__pw.settled.push({ part: Number(id), by: "browser" });
    buildIndex();
    bindEvents();
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
function setAttributePart(live, source, v = signalAt(live.path)) {
  const part = (parts.parts ?? []).find((p) => p.id === live.part);
  if (!part) return;
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

/** The parts the runtime sets in place: a text part's range and an
 * attribute (ADR-0142). A block is rendered again whole. */
const SET_IN_PLACE = new Set(["text", "attribute", "boolean_attribute"]);

let computedModule = null;

/** **What the page computes from its signals** (ADR-0227): the module the
 * compiler wrote, its functions the same ones the host computed the page's
 * first values with. Loaded when one of those signals first changes. */
function computedValues() {
  computedModule ??= import(parts.computed).then((module) => {
    log.push(`loaded ${parts.computed}`);
    return module;
  });
  return computedModule;
}

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
    if (live.derived && SET_IN_PLACE.has(live.kind)) {
      // A value computed from the signal (ADR-0227), by its part's function.
      if (!changed.has(live.signal)) continue;
      const v = (await computedValues()).parts[live.part](signalAt(live.path));
      if (live.kind === "text") setRange(addressOf([], live.part), textOf(v));
      else setAttributePart(live, from.get(live.signal), v);
      log.push(`signal ${live.signal} -> computed part ${live.part}`);
    } else if (live.kind === "text") {
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

// --- what a failed press is told -----------------------------------------
//
// **A refusal is told where the press was** (ADR-XXXX). Until 2026-10-09 a
// press a command's `requires` refused failed silently: the speculation was
// taken back, `data-pw-handler-error` set, a line logged, and the reader told
// nothing, the dead button this project exists to remove (the owner's
// finding, the feed's composer signed out). The words go beside the control,
// which its `aria-describedby` names, and to the page's announcer, a
// `role="status"` the host serves empty from the page's first byte, so a
// screen reader says them without the focus moving (WCAG 2.2 SC 4.1.3,
// ARIA22). Focus, the control and what was typed stay as they are.

/** A command its `requires` refused: the predicate, and its words. */
class Refused extends Error {
  constructor(component, predicate, says) {
    super(`command ${component} refused by ${predicate}`);
    this.predicate = predicate;
    this.says = says;
  }
}

/** A command no answer came for, its retries spent. */
class Unreachable extends Error {
  constructor(component, cause) {
    super(`command ${component} could not be sent: ${cause?.message ?? cause}`);
  }
}

/** The platform's words, where no predicate's apply. */
const PLATFORM_SAYS = {
  unreachable: "This could not be sent. Check the connection and try again.",
  stale: "This page is out of date. Reload it to go on.",
  failed: "This did not work. Try again.",
};

let announcer = null;

/** The page's announcer: the one the host served (`pw_render::ANNOUNCER`),
 * the same node from the first byte, or, from a host that served none, one
 * added once, before anything is said. */
function announcerOf() {
  announcer ??= document.querySelector(".pw-announcer[role=status]");
  if (!announcer) {
    announcer = document.createElement("div");
    announcer.setAttribute("role", "status");
    announcer.className = "pw-announcer";
    // Seen by no one, said to whoever reads the page by ear.
    announcer.style.cssText =
      "position:absolute;width:1px;height:1px;margin:-1px;padding:0;border:0;" +
      "overflow:hidden;clip-path:inset(50%);white-space:nowrap";
    document.body.append(announcer);
  }
  return announcer;
}

/** Each control's message, beside it. */
const messages = new WeakMap();
let told = 0;
/** The announcer's words waiting to be written. */
let saying = 0;

/** Tell `words` where `el` was pressed, as `kind`, and say them. */
function tell(el, words, kind) {
  let message = messages.get(el);
  if (!message?.isConnected) {
    message = document.createElement("span");
    message.className = "pw-refusal";
    message.id = `pw-said-${++told}`;
    el.after(message);
    messages.set(el, message);
  }
  message.textContent = words;
  const described = (el.getAttribute("aria-describedby") ?? "").split(" ").filter(Boolean);
  if (!described.includes(message.id)) {
    el.setAttribute("aria-describedby", [...described, message.id].join(" "));
  }
  el.dataset.pwHandlerError = kind;
  // Emptied, then written: the same words told twice are said twice. Words
  // told before the last were written are not said at all: the last are.
  const status = announcerOf();
  status.textContent = "";
  clearTimeout(saying);
  saying = setTimeout(() => {
    status.textContent = words;
  }, 50);
  log.push(`told ${kind}: ${words}`);
}

/** What `el`'s last press was told, taken back: it is pressed again. */
function untell(el) {
  const message = messages.get(el);
  if (message) {
    message.textContent = "";
    const described = (el.getAttribute("aria-describedby") ?? "")
      .split(" ")
      .filter((id) => id && id !== message.id);
    if (described.length) el.setAttribute("aria-describedby", described.join(" "));
    else el.removeAttribute("aria-describedby");
  }
  delete el.dataset.pwHandlerError;
}

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
        tell(el, PLATFORM_SAYS.stale, verdict.recovery);
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
        tell(el, PLATFORM_SAYS.stale, "reload-loop");
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
/** Each press whose handler has not finished, as a promise of its finishing
 * (ADR-0280): a navigation waits for each made before it. */
const pressing = new Set();
/** Whether a handler has gone to another page (ADR-0280): from then the page
 * takes no press and no second navigation. A page the browser brings back
 * from its back-forward cache takes them again. */
let navigating = false;
addEventListener("pageshow", (e) => {
  if (e.persisted) navigating = false;
});
/**
 * **A handler goes to a page once its command commits** (ADR-0280). Its
 * module calls this last in the `Ok` arm of a command's answer, and an `Ok`
 * is a commit: the command callback refuses one that is not. From now the
 * page takes no press. Each press made before this one finishes first, its
 * command answered and its arms run, so the page it goes to is read after
 * every commit this page was answered. Then the browser loads it: a document
 * the server serves after the session's turn, never from a cache
 * (`no-store`), so no parameter busts one and nothing is reloaded. A second
 * navigation, made while the page leaves, is not taken: the first decides.
 */
async function navigate(route, args, mine) {
  if (navigating) {
    log.push(`navigation to ${route} not taken: the page is already leaving`);
    return;
  }
  const url = pageAddress(route, args);
  navigating = true;
  await Promise.allSettled([...pressing].filter((p) => p !== mine));
  log.push(`navigating to ${url}`);
  // A navigation stopped, by the user or `window.stop()`, leaves the page
  // where it was, and it takes presses again: where the browser says so.
  // The Navigation API's `navigate` event fires for this one too, and its
  // signal aborts when it is stopped. Where there is none, the page stays
  // leaving until it is loaded again.
  if (typeof navigation !== "undefined") {
    navigation.addEventListener(
      "navigate",
      (e) => e.signal.addEventListener("abort", () => (navigating = false), { once: true }),
      { once: true },
    );
  }
  location.assign(url);
}

/**
 * **A page's address** (ADR-0280): each `{name}` segment of its route given
 * its value, encoded as the renderer encodes a link's (`url_component`):
 * every UTF-8 byte but RFC 3986's unreserved characters, so a value is one
 * segment, whatever it holds. A value no segment can carry, empty, `.` or
 * `..`, which the URL parser drops or climbs by, is refused, and the press
 * fails visibly.
 */
function pageAddress(route, args) {
  return route.replace(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (_, name) => {
    const value = args[name];
    if (typeof value !== "string") {
      throw new Error(`no value for {${name}} in ${route}`);
    }
    if (value === "" || value === "." || value === "..") {
      throw new Error(`{${name}} in ${route} cannot be ${JSON.stringify(value)}: no segment carries it`);
    }
    let out = "";
    for (const b of new TextEncoder().encode(value)) {
      const unreserved =
        (b >= 0x41 && b <= 0x5a) || (b >= 0x61 && b <= 0x7a) || (b >= 0x30 && b <= 0x39) ||
        b === 0x2d || b === 0x2e || b === 0x5f || b === 0x7e;
      out += unreserved ? String.fromCharCode(b) : "%" + b.toString(16).toUpperCase().padStart(2, "0");
    }
    return out;
  });
}
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
  for (const [template, list] of manifests) {
    for (const part of list) bindEvent(template, part);
  }
}

/** One event part of `template`, bound to every element it has: the
 * page's, or a view's in each of its instances (ADR-0203). */
function bindEvent(template, part) {
  if (part.kind !== "event") return;
  // A part of a view's instance is named by its template, and the page's
  // as it always was.
  const named = template === PAGE ? `${part.id}` : `${template}:${part.id}`;
  // Every INSTANCE of this template position. A template-scoped id names a
  // position in the template; the document has one instance per loop item,
  // and all three Add buttons carry `data-pw="0"`.
  const addresses = addressesFor(`e${part.owner}`, template);
  const owners = addresses.map((a) => index.get(a).element).filter(Boolean);
  if (owners.length === 0) {
    // A part in a block not rendered now (ADR-0130) is bound when it is.
    if (!booted) log.push(`no element ${part.owner} for part ${named}`);
    return;
  }

  // The decision, before anything is bound. `decide` returning anything but
  // "attach" leaves the element without a listener — the button is inert
  // rather than wrong.
  //
  // Asked NOW rather than on interaction, deliberately. A refused handler
  // must never load its code: fetching first and checking after would make
  // the refusal a formality the network had already ignored.
  let verdict = verdicts.get(named);
  if (!verdict) {
    const manifest = manifestFor(part);
    verdict =
      decide && manifest ? decide(manifest) : { attach: false, recovery: "no-decision" };
    verdicts.set(named, verdict);
    if (!verdict.attach) {
      // The CODE as well as the recovery. "refused 5: none" says a handler
      // was refused and nothing about why; the code is the one field that
      // distinguishes an unknown handler from a widened privacy scope from
      // a stale document schema.
      log.push(`refused ${named}: code ${verdict.code} recovery ${verdict.recovery}`);
    }
  }
  if (!verdict.attach) {
    recoverOnPress(part, owners, verdict);
    return;
  }

  // The event the part handles, and what to do before any code loads.
  const event = part.event || "press";
  const listens = DOM_EVENTS[event];
  if (!listens) {
    log.push(`no listener for event ${event} of part ${named}`);
    return;
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
      // **A page that is leaving takes no press** (ADR-0280): a handler has
      // gone to another page once its command committed, and that page is
      // loading. A press now would send what no one here would read.
      if (navigating) {
        log.push(`press on ${part.value} not taken: the page is leaving`);
        return;
      }
      lastActed = el;
      // What its last press was told, taken back (ADR-XXXX).
      untell(el);
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
      // This press until its handler finishes: a navigation waits for each
      // press made before it (ADR-0280).
      let finished;
      const done = new Promise((resolve) => (finished = resolve));
      pressing.add(done);
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
          command: async (component, args, how = {}) => {
            const interaction = `${press}-${calls++}`;
            const ids = await speculate(component, args, interaction);
            let answer;
            try {
              answer = await command(component, args, interaction, how.retry);
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
            // **An answer that is not a declared error is a commit**
            // (ADR-0280): an `Ok` arm runs only after one, and a navigation
            // in it goes only after one. One that did not commit has failed
            // the press, whatever it says.
            if (answer.committed !== true && answer.result?.$case !== "err") {
              throw new Error(`${component} answered without committing`);
            }
            return answer.result;
          },
          // Goes to a page once the command it follows commits (ADR-0280).
          navigate: (route, args) => navigate(route, args, done),
        });
      } catch (error) {
        // A load or a refused command is VISIBLE and leaves the button
        // usable. A silent failure here is the worst outcome available: the
        // press did nothing, the page looks fine, and the next press is the
        // user's only way to find out.
        loaded.delete(part.value);
        attempts.set(part.value, (attempts.get(part.value) ?? 0) + 1);
        // Told where the press was (ADR-XXXX): a refusal in its predicate's
        // words, anything else in the platform's.
        if (error instanceof Refused) {
          tell(el, error.says || PLATFORM_SAYS.failed, `refused:${error.predicate}`);
        } else if (error instanceof Unreachable) {
          tell(el, PLATFORM_SAYS.unreachable, "unreachable");
        } else {
          tell(el, PLATFORM_SAYS.failed, "failed");
        }
        log.push(`handler ${part.value} failed: ${error.message ?? error}`);
        window.__pw.handlerErrors = (window.__pw.handlerErrors ?? 0) + 1;
      } finally {
        pressing.delete(done);
        finished();
      }
    });
  }
  if (fresh > 0) {
    log.push(
      `attached ${named} to ${owners.length} instance(s): ${addresses.join(" ")}`,
    );
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
  // The announcer a failed press is said by, there before anything is
  // (ADR-XXXX): a live region added with its words is not reliably said.
  announcerOf();
  log.push(`indexed ${indexed} address(es)`);
  for (const p of parts.parts ?? []) {
    if (p.kind === "stream" && streamPending(String(p.id))) pendingAtIndex.add(String(p.id));
  }

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
  const value = parsed.content.firstChild.getAttribute(op.name) ?? "";
  // A value set again, the same, is a change all the same, and a control's
  // name changed is said again where it has focus (ADR-0182).
  if (element.getAttribute(op.name) !== value) element.setAttribute(op.name, value);
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
        // A block a speculation renders too (ADR-0172): what the server
        // rendered is what it shows now.
        if (op.op === "replace_range" && !target.instances?.length) {
          shownBlocks.set(target.part, op.html);
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
  // The page's patches are derived from what it holds (ADR-0172), and focus
  // in a row stays with its row through them.
  const focused = focusedRow();
  revertLists();
  for (const frame of batch.frames ?? []) applyFrame(frame);
  reapplySpeculations().then(() => refocus(focused));
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
