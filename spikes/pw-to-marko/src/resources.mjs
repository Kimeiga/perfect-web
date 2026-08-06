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

export function add_to_cart(_item, quantity) {
  const cart = Cart(current_session());
  cart.line_count += quantity;
  return cart;
}
