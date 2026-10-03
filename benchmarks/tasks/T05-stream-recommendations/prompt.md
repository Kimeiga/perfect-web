# Show the store's recommendations

The store's recommender suggests a few items for each store, and the store's
data layer can already ask it for them. The store page does not show them
yet.

Show them on the store page, in a section labelled "Recommendations", as a
list of the items' names.

The recommender is slow: it takes over a second to answer. The rest of the
store page, its menu and its Add buttons, must not wait for it.

- While the recommender is answering, the section says "Finding
  recommendations".
- If it fails, the section says "No recommendations right now".
- Its suggestions change: show what it says when the page is loaded, not
  what it said before.

A test sets how the recommender behaves with
`POST /bench/recommendations?delay=…&fail=…&items=…`; that hook is already
there.

Keep everything else working.
