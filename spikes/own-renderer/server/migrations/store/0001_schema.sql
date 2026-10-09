-- The store's data in PostgreSQL (track `store-pg`): its stores and menus,
-- the carts, the orders and their lines, the notice, the kitchen, the
-- recommender's curation, each session's estimate, and the transactional
-- outbox. Applied once, in order, by `store_pg.rs`, in the store's own
-- schema, which records each in `pw_store_migrations`. Nothing here names
-- the feed's schema, and the store's connections search no other.
--
-- Every row a command writes records the transaction that wrote it
-- (`committed_in`), so a test can show a cart and its event committed in
-- one transaction, and not merely both committed.

-- The stores, in the order the home page lists them (ADR-0192).
CREATE TABLE stores (
    id            text PRIMARY KEY,
    position      integer NOT NULL UNIQUE,
    name          text NOT NULL,
    description   text NOT NULL,
    opens_minute  bigint NOT NULL,
    closes_minute bigint NOT NULL
);

-- A store's sections (ADR-0181): what its menu is grouped under.
CREATE TABLE categories (
    store text NOT NULL REFERENCES stores (id),
    id    text NOT NULL,
    name  text NOT NULL,
    PRIMARY KEY (store, id)
);

-- A store's menu, in its order. `served` is how an item is served, hot or
-- cold: what E14's T07 reads by category. `available` is whether it can be
-- ordered now (ADR-0178).
CREATE TABLE menu_items (
    store        text NOT NULL REFERENCES stores (id),
    id           text NOT NULL,
    position     integer NOT NULL,
    name         text NOT NULL,
    description  text NOT NULL,
    price        bigint NOT NULL CHECK (price >= 0),
    category     text NOT NULL,
    served       text NOT NULL CHECK (served IN ('hot', 'cold')),
    available    boolean NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (store, id),
    UNIQUE (store, position),
    FOREIGN KEY (store, category) REFERENCES categories (store, id)
);

CREATE INDEX menu_items_item ON menu_items (id);

-- A session's cart (ADR-0172): each line its item, as it was when the line
-- was made, and how many.
CREATE TABLE cart_lines (
    session      text NOT NULL,
    position     integer NOT NULL,
    item         text NOT NULL,
    name         text NOT NULL,
    quantity     bigint NOT NULL CHECK (quantity > 0),
    price        bigint NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (session, item),
    UNIQUE (session, position)
);

-- An order (ADR-0193): the cart's lines and a status. A session's order is
-- its latest.
CREATE TABLE orders (
    id           bigserial PRIMARY KEY,
    session      text NOT NULL,
    status       text NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id()
);

CREATE INDEX orders_session ON orders (session, id DESC);

CREATE TABLE order_lines (
    order_id     bigint NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    position     integer NOT NULL,
    item         text NOT NULL,
    name         text NOT NULL,
    quantity     bigint NOT NULL CHECK (quantity > 0),
    price        bigint NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (order_id, position)
);

-- What a store has posted (E14, T09).
CREATE TABLE notices (
    store text PRIMARY KEY REFERENCES stores (id),
    text  text NOT NULL
);

-- How long a store's kitchen takes now, in minutes (E14, T02).
CREATE TABLE kitchens (
    store        text PRIMARY KEY REFERENCES stores (id),
    prep_minutes bigint NOT NULL
);

-- What the recommender suggests for every store where it was curated, as
-- `[[id, name], ...]`; NULL draws from each store's menu (ADR-0165). One row.
CREATE TABLE recommender (
    one     boolean PRIMARY KEY DEFAULT true CHECK (one),
    curated jsonb CHECK (curated IS NULL OR jsonb_typeof(curated) = 'array')
);

-- A session's delivery estimate (E14, T10; ADR-0180), where one was made.
CREATE TABLE estimates (
    session     text PRIMARY KEY,
    min_minutes bigint NOT NULL,
    max_minutes bigint
);

-- The transactional outbox (ADR-0007, ADR-0208, ADR-0209), the feed's
-- shape (ADR-0246): what a command handed the platform, committed with its
-- writes or not at all, its values as the command computed them. A row is
-- deleted once delivered.
CREATE TABLE outbox (
    id           bigserial PRIMARY KEY,
    kind         text NOT NULL CHECK (kind IN ('event', 'invalidation')),
    name         text NOT NULL,
    args         jsonb NOT NULL CHECK (jsonb_typeof(args) = 'array'),
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id()
);
