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

/**
 * Every element carrying a given `ElementId`, in document order.
 *
 * A LIST, not one element. A template-scoped id names a position in the
 * TEMPLATE, and a position inside an `{#each}` has one document instance per
 * item — the store page's three Add buttons all carry `data-pw="0"`.
 *
 * A first version kept a `Map` from id to element and silently held the last
 * one, so the third button worked and the first two were inert. The page looked
 * correct and two thirds of it did nothing.
 *
 * For attaching behaviour this is the whole answer: every instance of that
 * template position gets the listener. For UPDATING one instance, the loop's
 * declared key is what distinguishes them, and that is the next piece of E7-R.
 */
function elementsWithId(id) {
  return [...document.querySelectorAll(`[data-pw="${id}"]`)];
}

/**
 * The nodes a range part owns, between its anchors.
 *
 * Walked once and kept: a range that currently holds nothing still has its two
 * comments, which is where new content goes. Finding it by "the text after the
 * label" would work until the label moved.
 */
function range(id) {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_COMMENT);
  let start = null;
  let end = null;
  let node;
  while ((node = walker.nextNode())) {
    if (node.data === `pw:s${id}`) start = node;
    else if (node.data === `pw:e${id}`) {
      end = node;
      break;
    }
  }
  return start && end ? { start, end } : null;
}

/** Replace a range's contents, leaving its anchors and everything else alone. */
function setRange(id, text) {
  const r = range(id);
  if (!r) return false;
  // Remove what is between the anchors, then insert. The anchors themselves are
  // never touched, so the part keeps its identity across the update — which is
  // what lets it be updated again.
  let n = r.start.nextSibling;
  while (n && n !== r.end) {
    const next = n.nextSibling;
    n.remove();
    n = next;
  }
  r.end.parentNode.insertBefore(document.createTextNode(text), r.end);
  return true;
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
  updated: [],
  ready: false,
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
  await bootDecision();

  for (const part of parts.parts ?? []) {
    if (part.kind !== "event") continue;
    const owners = elementsWithId(part.owner);
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

    for (const el of owners) {
      el.addEventListener("click", async (e) => {
        e.preventDefault();
        const response = await fetch("/command/add_to_cart", { method: "POST" });
        const next = await response.json();

        // Only the parts whose values changed. The manifest says which part
        // reads which path, so this updates by IDENTITY rather than by
        // re-rendering and comparing — there is nothing to compare against.
        window.__pw.updated = [];
        for (const p of parts.parts ?? []) {
          if (p.kind !== "text") continue;
          if (!(p.value in next)) continue;
          if (setRange(p.id, String(next[p.value]))) {
            window.__pw.updated.push(p.id);
          }
        }
        log.push(`updated ${window.__pw.updated.join(",")}`);
      });
    }
    log.push(`attached ${part.id} to ${owners.length} element(s) with id ${part.owner}`);
  }

  window.__pw.ready = true;
  document.documentElement.dataset.pwReady = "1";
}

attach().catch((e) => {
  log.push(`boot failed: ${e}`);
  // Deliberately NOT re-thrown into a state where handlers attach anyway. A
  // runtime that failed to boot leaves an inert page, which is a page that
  // works for reading and does nothing surprising.
  document.documentElement.dataset.pwReady = "failed";
});
