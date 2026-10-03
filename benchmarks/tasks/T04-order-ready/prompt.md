# Tell customers when their order is ready

The store page has a "Your order" section that says where the customer's
order is: placed, being prepared, or delivered. The kitchen sets an order's
state with `POST /bench/order?status=…`, for the customer whose session the
request carries.

Orders now have one more state between being prepared and delivered: ready
for pickup. Add it.

- The kitchen sets it with `status=ready`.
- The page's order section then says "Your order is ready for pickup."

Keep every other state as it is today, and everything else working.
