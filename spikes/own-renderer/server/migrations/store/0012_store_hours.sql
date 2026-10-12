-- Track `store-hours` (W9): each store's opening hours, in its IANA time
-- zone. A weekly period's closing not after its opening is the next day's
-- (22:00 to 02:00); the same minute is open the whole day round. A special
-- date's row replaces its weekday's periods: closed where its minutes are
-- NULL. The last order is `last_order_minutes` before a closing. The
-- in-memory layer's rows are `hours.rs`'s `seed`, the same; a test holds the
-- two layers alike.

ALTER TABLE stores
    ADD COLUMN time_zone          text   NOT NULL DEFAULT 'America/Los_Angeles',
    ADD COLUMN last_order_minutes bigint NOT NULL DEFAULT 20
        CHECK (last_order_minutes >= 0 AND last_order_minutes < 1440);

CREATE TABLE store_hours (
    store         text   NOT NULL REFERENCES stores (id) ON DELETE CASCADE,
    weekday       bigint NOT NULL CHECK (weekday BETWEEN 1 AND 7),
    opens_minute  bigint NOT NULL CHECK (opens_minute BETWEEN 0 AND 1440),
    closes_minute bigint NOT NULL CHECK (closes_minute BETWEEN 0 AND 1440),
    PRIMARY KEY (store, weekday, opens_minute)
);

CREATE TABLE store_special_hours (
    store         text   NOT NULL REFERENCES stores (id) ON DELETE CASCADE,
    day           date   NOT NULL,
    opens_minute  bigint CHECK (opens_minute BETWEEN 0 AND 1440),
    closes_minute bigint CHECK (closes_minute BETWEEN 0 AND 1440),
    PRIMARY KEY (store, day),
    CHECK ((opens_minute IS NULL) = (closes_minute IS NULL))
);

-- Every store open the whole day round, every day.
INSERT INTO store_hours (store, weekday, opens_minute, closes_minute)
SELECT s.id, d, 0, 0 FROM stores s CROSS JOIN generate_series(1, 7) AS d;
