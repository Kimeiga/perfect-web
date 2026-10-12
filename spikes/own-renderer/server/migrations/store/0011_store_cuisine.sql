-- ADR-0308, amended (the integrator's ruling of 2026-10-12): a store's
-- cuisine is the store's own field, text as written ("Café, Bakery"), shown
-- on the home page, the store's page and a search. 0010's table of search's
-- own is copied into the column and dropped. The in-memory layer's rows are
-- `search.rs`'s `cuisine_of`, the same; a test holds the two layers alike.

ALTER TABLE stores ADD COLUMN cuisine text NOT NULL DEFAULT '';

UPDATE stores SET cuisine = c.cuisine FROM store_cuisines c WHERE c.store = stores.id;

DROP TABLE store_cuisines;
