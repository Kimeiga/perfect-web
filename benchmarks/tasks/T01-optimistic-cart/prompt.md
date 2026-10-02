# Make Add feel instant

On slow connections, pressing **Add** shows nothing until the server answers,
and customers press again. Make the cart count change as soon as Add is
pressed, before the server answers.

If the server then refuses the add, or the request fails, the count must go
back to what it was. When the server accepts it, the count must end up at the
server's value.

Keep everything else working as it does today.
