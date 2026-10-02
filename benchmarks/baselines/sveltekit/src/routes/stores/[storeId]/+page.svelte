<script lang="ts">
  import { enhance } from "$app/forms";
  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();
</script>

<main>
  <h1 id="store-name">{data.store.name}</h1>

  <section aria-label="Menu">
    <ul id="menu">
      {#each data.menu as item (item.id)}
        <li>
          <span>{item.name}</span>
          <form method="POST" action="?/add" use:enhance>
            <input type="hidden" name="item" value={item.id} />
            <button type="submit">Add</button>
          </form>
        </li>
      {/each}
    </ul>
  </section>

  <section aria-label="Cart">
    <p id="cart-count">{data.cartCount}</p>
    <form method="POST" action="?/clear" use:enhance>
      <button id="clear-cart" type="submit">Clear</button>
    </form>
  </section>
</main>
