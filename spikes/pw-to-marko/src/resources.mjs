/* Host implementations for the resources generated templates import.
 *
 * ADR-0018: the compiler emits a manifest saying HOW a resource behaves; the
 * host decides WHAT it does. This file is that decision for the spike, and it
 * is hand-written on purpose — nothing generates the fetch yet.
 *
 * The delay models charter §15.5's recommendation latency, so the streaming
 * measurement has something real to wait for.
 */
const RECOMMENDATION_DELAY_MS = 1200;

export function Recommendations(_id) {
  return new Promise((resolve) =>
    setTimeout(
      () => resolve([{ id: "a", name: "Affogato" }, { id: "g", name: "Gibraltar" }]),
      RECOMMENDATION_DELAY_MS,
    ),
  );
}

/* --- E4/E5: the store page -------------------------------------------------
 *
 * The split the demo exists to show. `Store` and `Menu` are the same for every
 * reader — a shared cache is correct for them. `Cart` belongs to one session,
 * and the `session` modifier on its declaration is what makes putting it in a
 * shared cache a compile error (PW5001) rather than a leak nobody notices.
 *
 * Hand-written, like the rest of this file: the compiler says HOW a resource
 * behaves via the manifest (ADR-0018); the host decides WHAT it does.
 */
const MENU = [
  { id: "espresso", name: "Espresso" },
  { id: "cortado", name: "Cortado" },
  { id: "affogato", name: "Affogato" },
];

// Per-session carts, so "private" is observable rather than asserted.
const CARTS = new Map();

export function Store(_id) {
  return { name: "Blue Bottle" };
}

export function Menu(_id) {
  return MENU;
}

export function current_session() {
  return "session-1";
}

export function PositiveInt(n) {
  return n;
}

export function Cart(session) {
  if (!CARTS.has(session)) CARTS.set(session, { line_count: 0 });
  return CARTS.get(session);
}

/* E7V in the actual path.
 *
 * The store page's Add button calls this. In the browser it first asks the
 * compatibility decision — `runtime/pw-resume`, compiled to wasm, the SAME code
 * the deployment matrix tests — whether the handler's manifest may attach. If
 * the decision refuses, the mutation does not happen and the recovery it chose
 * is put on the document where a test (or a user-facing shell) can see it.
 *
 * A page cannot bypass this by construction on the Rust side: the wasm module
 * takes the authorisation inside its own call, and the ABI exposes no way to
 * attach without a Resume or Migrate answer.
 *
 * The manifest is read from `document.body.dataset.resumeManifest` so a test
 * can inject an incompatible one into the real page. A deployment would put it
 * in the document the server rendered.
 */
const COMPATIBLE = "2|1|B1|add_to_cart|cart|cart-doc|public|x|region";

/* Boot the compatibility decision from here rather than from a layout.
 *
 * This module is imported by the generated page, so it is in the client bundle
 * already — no extra script tag, and nothing added to /static, whose gate item
 * is that it ships none.
 *
 * A layout was tried first and is instructive: a bare `<script>` in a Marko
 * template is the component's script BLOCK, not an HTML element, and it
 * swallowed the page body. `html-script` fixes that and then `renderBody`
 * rendered nothing. Two failures for one script tag; the module that is
 * already loaded is the simpler place.
 */
let resumeReady = false;
if (typeof document !== "undefined") {
  import("./resume-runtime.mjs").then(async (m) => {
    await m.load();
    globalThis.__pwResume = m;
    resumeReady = true;
    document.body.dataset.resumeReady = "1";
  });
}

export function add_to_cart(_item, quantity) {
  // Fail closed: no decision, no attachment. A handler that ran because the
  // gate had not finished loading would be the bypass this design exists to
  // make impossible.
  if (typeof document !== "undefined") {
    if (!resumeReady || !globalThis.__pwResume?.ready()) {
      document.body.dataset.resumeDecision = "refused:decision unavailable";
      return Cart(current_session());
    }
    const { decide } = globalThis.__pwResume;
    const manifest = document.body.dataset.resumeManifest || COMPATIBLE;
    const decision = decide(manifest);
    document.body.dataset.resumeDecision = decision.attached
      ? decision.kind
      : `refused:${decision.why}`;
    if (!decision.attached) {
      // Refused. The handler does not run, and the recovery is announced
      // rather than silently swallowed.
      document.body.dataset.resumeRecovery = decision.recovery;
      return Cart(current_session());
    }
  }
  const cart = Cart(current_session());
  cart.line_count += quantity;
  return cart;
}
