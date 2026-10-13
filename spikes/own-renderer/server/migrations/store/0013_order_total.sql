-- Track `order-total` (W9): what an order costs. Each store's fee schedule,
-- amounts in cents and rates in millionths (ADR-0327); the promo codes, each
-- one store's, and who has used each, once per reader by this table's key;
-- what each reader chose at checkout; and each order as charged, per store,
-- with the rate applied, so a later rate never rewrites what was charged.
-- The in-memory layer's rows are `checkout.rs`'s `seed_fees` and
-- `seed_promos`, the same; a test holds the two layers alike.

CREATE TABLE store_fees (
    store             text    PRIMARY KEY REFERENCES stores (id) ON DELETE CASCADE,
    delivery_fee      bigint  NOT NULL CHECK (delivery_fee >= 0),
    service_rate      bigint  NOT NULL CHECK (service_rate >= 0),
    service_min       bigint  NOT NULL CHECK (service_min >= 0),
    service_max       bigint  NOT NULL CHECK (service_max >= service_min),
    small_order_under bigint  NOT NULL CHECK (small_order_under >= 0),
    small_order_fee   bigint  NOT NULL CHECK (small_order_fee >= 0),
    tax_rate          bigint  NOT NULL CHECK (tax_rate >= 0),
    tax_delivery      boolean NOT NULL,
    tax_service       boolean NOT NULL
);

-- San Francisco's, the stores' city: delivery $2.99; service 15%, $3.00 to
-- $12.00; $2.50 under $10.00; sales tax 8.625% (CDTFA, effective
-- 2026-10-01); neither delivery nor service taxed (the ADR says why).
INSERT INTO store_fees
SELECT id, 299, 150000, 300, 1200, 1000, 250, 86250, false, false FROM stores;

CREATE TABLE promos (
    code         text   PRIMARY KEY,
    store        text   NOT NULL REFERENCES stores (id) ON DELETE CASCADE,
    percent_off  bigint CHECK (percent_off >= 0),
    amount_off   bigint CHECK (amount_off >= 0),
    min_subtotal bigint NOT NULL CHECK (min_subtotal >= 0),
    expires_at   bigint NOT NULL,
    funded_by    text   NOT NULL CHECK (funded_by IN ('store', 'platform')),
    CHECK ((percent_off IS NULL) <> (amount_off IS NULL))
);

-- WELCOME10: 10% off Blue Bottle's food over $15.00, the platform's, until
-- 2030-01-01.
INSERT INTO promos (code, store, percent_off, amount_off, min_subtotal, expires_at, funded_by)
SELECT 'WELCOME10', id, 100000, NULL, 1500, 1893456000000, 'platform'
FROM stores WHERE id = '47';

-- A code used, once per reader: the key is the rule.
CREATE TABLE promo_uses (
    code  text NOT NULL,
    owner text NOT NULL,
    PRIMARY KEY (code, owner)
);

-- What a reader chose at checkout: no tip unless they choose one.
CREATE TABLE checkout_choices (
    owner      text   PRIMARY KEY,
    tip_kind   text   NOT NULL CHECK (tip_kind IN ('none', 'fifteen', 'eighteen', 'twenty', 'amount')),
    tip_amount bigint NOT NULL DEFAULT 0 CHECK (tip_amount >= 0),
    promo      text
);

ALTER TABLE orders
    ADD COLUMN tip   bigint,
    ADD COLUMN total bigint,
    ADD COLUMN promo text;

CREATE TABLE order_parts (
    order_id    bigint  NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    position    integer NOT NULL,
    store       text    NOT NULL,
    subtotal    bigint  NOT NULL,
    discount    bigint  NOT NULL,
    funded_by   text    CHECK (funded_by IN ('by-store', 'by-platform')),
    delivery    bigint  NOT NULL,
    service     bigint  NOT NULL,
    small_order bigint  NOT NULL,
    taxable     bigint  NOT NULL,
    tax_rate    bigint  NOT NULL,
    tax         bigint  NOT NULL,
    PRIMARY KEY (order_id, position)
);
