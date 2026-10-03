# Don't let the delivery estimate hold up or break the store page

The store page shows the customer's delivery estimate, "Delivery in 25 min",
in a section labelled "Delivery". The estimate comes from an estimator that
is slow, taking up to a few seconds, and is sometimes down.

Today the whole store page waits for the estimate, and when the estimator is
down the store page does not load at all. Change that, so the menu, the cart
and the Add buttons never wait for the estimate or fail with it:

- while the estimate is being worked out, the section says "Estimating
  delivery";
- if the estimator fails, the section says "Delivery estimate unavailable";
- otherwise it says "Delivery in N min", as it does now.

A test sets how the estimator behaves for the customer's session with
`POST /bench/estimate?delay=…&fail=…&minutes=…`; that hook is already
there.

Keep everything else working.
