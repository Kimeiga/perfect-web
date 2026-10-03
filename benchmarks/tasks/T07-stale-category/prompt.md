# Stop reading a category the customer moved past

The store page lets a customer browse the menu by category: All, Hot and
Cold. Choosing a category reads its items, and reading a category can be
slow: when the kitchen is busy, Hot can take a second and a half.

Customers who choose Hot and then, before it answers, choose Cold wait for
Hot's items before they see Cold's, or see Cold's replaced by Hot's. And
the store keeps reading Hot for nobody.

Change it so that:
- the items shown are always the chosen category's, as soon as it has
  answered, whatever a category chosen before it does;
- a category the customer has moved past is not read any longer: its
  request is stopped.

A test makes a category slow with `POST /bench/category?slow=hot&delay=1500`;
that hook is already there.

Keep everything else working.
