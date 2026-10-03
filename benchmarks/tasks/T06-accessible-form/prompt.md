# Let customers request an item

Some customers want items the store does not sell. Add a form below the menu
where they can ask for one. It is answered on the page, and nothing is sent
to the server.

- A text field labelled "Item to request", and a **Request** button.
- Submitting with the field empty shows the error "Enter an item to
  request." next to the field.
- Submitting a name shows "Thanks! We'll ask the store about NAME." with
  the name typed, removes any error, and empties the field.

The form must be accessible:
- the field has a visible label tied to it;
- the form submits with Enter;
- while the error shows, the field is marked invalid and the error is its
  description;
- the error and the thanks are each announced to screen reader users.

Keep everything else working as it does today.
