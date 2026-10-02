import { notFound } from "next/navigation";
import { getCart, getMenu, getStore, lineCount } from "@/lib/store";
import { currentSession } from "@/lib/session";
import { addToCartAction, clearCartAction } from "./actions";

export default async function StorePage({
  params,
}: {
  params: Promise<{ storeId: string }>;
}) {
  const { storeId } = await params;
  const store = getStore(storeId);
  const menu = getMenu(storeId);
  if (!store || !menu) notFound();
  const cart = getCart(await currentSession());

  return (
    <main>
      <h1 id="store-name">{store.name}</h1>

      <section aria-label="Menu">
        <ul id="menu">
          {menu.map((item) => (
            <li key={item.id}>
              <span>{item.name}</span>
              <form action={addToCartAction}>
                <input type="hidden" name="store" value={store.id} />
                <input type="hidden" name="item" value={item.id} />
                <button type="submit">Add</button>
              </form>
            </li>
          ))}
        </ul>
      </section>

      <section aria-label="Cart">
        <p id="cart-count">{lineCount(cart)}</p>
        <form action={clearCartAction}>
          <input type="hidden" name="store" value={store.id} />
          <button id="clear-cart" type="submit">
            Clear
          </button>
        </form>
      </section>
    </main>
  );
}
