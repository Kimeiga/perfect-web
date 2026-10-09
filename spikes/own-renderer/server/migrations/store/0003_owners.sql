-- Track `store-accounts` (ADR-XXXX): a cart and an order are their owner's,
-- not their session's. The owner is opaque to the store: a user's id where
-- the program reads a cart by its reader's handle (`store:data/user-carts`,
-- the canonical store), or a session's id where it reads one by the session
-- (`store:data/carts`, the benchmark's copy of the store). A guest's user is
-- its session's guest (ADR-0270), `u-` and the session's id.
--
-- The rows written before this migration keep their values: each was its
-- session's, which the benchmark's copy still reads it by. The canonical
-- store's guests begin with an empty cart, as after a deployment.

ALTER TABLE cart_lines RENAME COLUMN session TO owner;
ALTER TABLE orders RENAME COLUMN session TO owner;
ALTER INDEX orders_session RENAME TO orders_owner;
