"use server";

import { revalidatePath } from "next/cache";
import { addToCart, clearCart, isMenuItem } from "@/lib/store";
import { ensureSession } from "@/lib/session";

export async function addToCartAction(formData: FormData): Promise<void> {
  const store = String(formData.get("store") ?? "");
  const item = String(formData.get("item") ?? "");
  if (!isMenuItem(store, item)) {
    throw new Error(`no menu item ${item} in store ${store}`);
  }
  const session = await ensureSession();
  addToCart(session, item, 1);
  revalidatePath(`/stores/${store}`);
}

export async function clearCartAction(formData: FormData): Promise<void> {
  const store = String(formData.get("store") ?? "");
  const session = await ensureSession();
  clearCart(session);
  revalidatePath(`/stores/${store}`);
}
