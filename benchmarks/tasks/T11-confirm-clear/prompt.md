# Confirm before clearing the cart

Customers clear their cart by accident. Make **Clear** ask first.

Pressing **Clear** opens a dialog titled "Clear your cart?" with two buttons,
**Keep items** and **Clear cart**:

- **Clear cart** empties the cart and closes the dialog.
- **Keep items** closes the dialog and leaves the cart as it was.
- The Escape key closes it too, and leaves the cart as it was.

The dialog must work for keyboard and screen reader users. It is announced as
a modal dialog named by its title, and focus moves into it when it opens.
Nothing else on the page can be reached while it is open, and focus returns
to **Clear** when it closes.

Keep everything else working as it does today.
