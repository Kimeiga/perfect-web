# Prevent duplicate submissions

Customers on flaky connections report that pressing **Add** once sometimes adds
the item twice. A retrying proxy, or the browser itself, can send the same
request again after the first one reached the server.

Make pressing Add once add exactly one item, even when that press's request is
delivered to the server more than once.

Keep everything else working: two separate presses still add two items, Clear
still empties the cart, each session keeps its own cart, and the page still
works the way it does today.
