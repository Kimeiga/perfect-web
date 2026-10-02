// The canonical store (charter §15), held in memory by this one server
// process, as the Pleris development server holds it. The store and menu are
// the same for every reader; a cart belongs to one session.

export type StoreId = string;
export type MenuItemId = string;

export type Store = { id: StoreId; name: string };
export type MenuItem = { id: MenuItemId; name: string };
export type CartLine = { itemId: MenuItemId; quantity: number };
export type Cart = { lines: CartLine[] };

const STORES: Record<StoreId, Store> = {
  "blue-bottle": { id: "blue-bottle", name: "Blue Bottle" },
};

const MENUS: Record<StoreId, MenuItem[]> = {
  "blue-bottle": [
    { id: "espresso", name: "Espresso" },
    { id: "cortado", name: "Cortado" },
    { id: "cold-brew", name: "Cold Brew" },
  ],
};

// One map for the process. `globalThis` so that every module instance the
// bundler makes shares it.
const carts: Map<string, Map<MenuItemId, number>> =
  ((globalThis as Record<string, unknown>).__pwBenchCarts as Map<
    string,
    Map<MenuItemId, number>
  >) ?? new Map();
(globalThis as Record<string, unknown>).__pwBenchCarts = carts;

export function getStore(id: StoreId): Store | undefined {
  return STORES[id];
}

export function getMenu(id: StoreId): MenuItem[] | undefined {
  return MENUS[id];
}

export function getCart(session: string | undefined): Cart {
  const lines = session ? carts.get(session) : undefined;
  return {
    lines: [...(lines ?? new Map())].map(([itemId, quantity]) => ({ itemId, quantity })),
  };
}

/** How many items the cart holds: what the page shows. */
export function lineCount(cart: Cart): number {
  return cart.lines.reduce((n, line) => n + line.quantity, 0);
}

export function isMenuItem(store: StoreId, item: string): boolean {
  return (MENUS[store] ?? []).some((m) => m.id === item);
}

export function addToCart(session: string, item: MenuItemId, quantity: number): Cart {
  const lines = carts.get(session) ?? new Map<MenuItemId, number>();
  lines.set(item, (lines.get(item) ?? 0) + quantity);
  carts.set(session, lines);
  return getCart(session);
}

export function clearCart(session: string): Cart {
  carts.delete(session);
  return getCart(session);
}
