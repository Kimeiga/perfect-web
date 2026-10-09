-- The store's first data, the in-memory layer's (`store.rs`, `State::seed`):
-- two stores, 47, whose menu E7-P changes, and 48 (ADR-0162, ADR-0192);
-- their menus, described and priced as the catalogue says (ADR-0166,
-- ADR-0169) and grouped as ADR-0181 lists them; the notice store 47 has
-- posted; how long its kitchen takes; nothing curated. A test holds the two
-- layers to answer alike.

INSERT INTO stores (id, position, name, description, opens_minute, closes_minute) VALUES
    ('47', 0, 'Blue Bottle',
     'Small-batch coffee, served at the bar or carried out. The espresso changes with the season, and the pastries come in every morning.',
     420, 1140),
    ('48', 1, 'Harbor Coffee',
     'A neighborhood cafe by the water, pouring drip coffee brewed to order and matcha whisked by hand.',
     420, 1140);

INSERT INTO categories (store, id, name) VALUES
    ('47', 'coffee', 'Coffee'),
    ('48', 'drinks', 'Drinks'),
    ('48', 'bakery', 'Bakery');

INSERT INTO menu_items (store, id, position, name, description, price, category, served, available) VALUES
    ('47', 'espresso', 0, 'Espresso', 'A double shot, pulled short.', 350, 'coffee', 'hot', true),
    ('47', 'cortado', 1, 'Cortado', 'Espresso cut with an equal part of warm milk.', 425, 'coffee', 'hot', true),
    ('47', 'cold-brew', 2, 'Cold Brew', 'Steeped for eighteen hours and served over ice.', 475, 'coffee', 'cold', true),
    ('48', 'drip', 0, 'Drip Coffee', 'Brewed to order, one cup at a time.', 300, 'drinks', 'hot', true),
    ('48', 'matcha', 1, 'Matcha Latte', 'Ceremonial matcha whisked with steamed milk.', 525, 'drinks', 'hot', true),
    ('48', 'scone', 2, 'Blueberry Scone', 'Baked each morning with wild blueberries.', 375, 'bakery', 'hot', true);

INSERT INTO notices (store, text) VALUES ('47', 'Open until 7 pm');

INSERT INTO kitchens (store, prep_minutes) VALUES ('47', 12);

INSERT INTO recommender (curated) VALUES (NULL);
