-- Track `reviews` (ADR-0341): a customer's rating of a store, through an
-- order of theirs that was delivered. One per order and store, by this
-- table's key; its stars 1 to 5, and its comment, empty where none, at most
-- 500 characters. No user: the order names its owner. Not a foreign key to
-- `orders`: a rating stands where the kitchen's route removes its order
-- (`orders#set-status`), as the in-memory layer's does. `id` orders a store's
-- reviews, newest first. The in-memory layer's rows are `State`'s
-- `ratings`, the same; a test holds the two layers alike.

CREATE TABLE order_ratings (
    id           bigserial PRIMARY KEY,
    order_id     bigint NOT NULL,
    store        text   NOT NULL REFERENCES stores (id) ON DELETE CASCADE,
    stars        bigint NOT NULL CHECK (stars BETWEEN 1 AND 5),
    comment      text   NOT NULL DEFAULT '' CHECK (char_length(comment) <= 500),
    committed_in xid8   NOT NULL DEFAULT pg_current_xact_id(),
    UNIQUE (order_id, store)
);

-- A store's ratings counted, and its newest reviews, by store.
CREATE INDEX order_ratings_store ON order_ratings (store, id DESC);
