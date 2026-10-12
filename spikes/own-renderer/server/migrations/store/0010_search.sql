-- Track `store-search` (W9): each store's cuisine, which a search matches
-- beside its name and its description. The in-memory layer's rows are
-- `search.rs`'s `cuisine_of`, the same; a test holds the two layers to
-- answer alike. A table of its own, so the stores' own rows stay as they are.

CREATE TABLE store_cuisines (
    store   text PRIMARY KEY REFERENCES stores (id),
    cuisine text NOT NULL
);

INSERT INTO store_cuisines (store, cuisine) VALUES
    ('47', 'Coffee'),
    ('48', 'Café, Bakery');
