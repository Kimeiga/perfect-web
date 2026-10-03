# Show what is in the cart

The store page shows how many items the cart holds, and not which. In the
cart section, below the count, list each line of the cart: the item's name
and how many, as "Espresso × 2", in the order the items were first added.

- The list is labelled "In your cart", with one list item per line. When
  the cart is empty, it lists nothing.
- Adding an item, and clearing the cart, update the list on the page with
  no reload.
- A cart is its customer's alone: no other customer may ever see its lines,
  whatever the page keeps between requests.

Keep everything else working as it does today.
