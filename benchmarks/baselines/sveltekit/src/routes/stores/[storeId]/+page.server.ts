import { error, fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { addToCart, clearCart, getCart, getMenu, getStore, isMenuItem, lineCount } from "$lib/server/store";
import { currentSession, ensureSession } from "$lib/server/session";

export const load: PageServerLoad = ({ params, cookies }) => {
  const store = getStore(params.storeId);
  const menu = getMenu(params.storeId);
  if (!store || !menu) error(404, "no such store");
  return {
    store,
    menu,
    cartCount: lineCount(getCart(currentSession(cookies))),
  };
};

export const actions: Actions = {
  add: async ({ params, request, cookies }) => {
    const item = String((await request.formData()).get("item") ?? "");
    if (!isMenuItem(params.storeId, item)) {
      return fail(400, { error: `no menu item ${item}` });
    }
    addToCart(ensureSession(cookies), item, 1);
  },
  clear: async ({ cookies }) => {
    clearCart(ensureSession(cookies));
  },
};
