-- Track `store-accounts` (ADR-XXXX), milestone 2: where each store is and
-- how far it delivers, and each reader's saved addresses. The places an
-- address may name are the repository's fixed table (`places.rs`), not a
-- geocoder's, and are not rows: an address keeps a place's id.
--
-- Store 47 is at Union Square and delivers four kilometres around it, store
-- 48 at the Ferry Building and two (`store.rs`, `zone_of`); a test holds the
-- two layers to answer alike. Coordinates in millionths of a degree.

ALTER TABLE stores
    ADD COLUMN lat_e6   bigint NOT NULL DEFAULT 0,
    ADD COLUMN lon_e6   bigint NOT NULL DEFAULT 0,
    ADD COLUMN radius_m bigint NOT NULL DEFAULT 0 CHECK (radius_m >= 0);

UPDATE stores SET lat_e6 = 37787990, lon_e6 = -122407440, radius_m = 4000 WHERE id = '47';
UPDATE stores SET lat_e6 = 37795500, lon_e6 = -122393700, radius_m = 2000 WHERE id = '48';

-- A reader's saved addresses, in the order they were saved: its owner (a
-- user's id, or a session's guest's), the label the reader gave it, the
-- place it names, and whether it is the one chosen, at most one per owner.
CREATE TABLE addresses (
    owner        text NOT NULL,
    id           text NOT NULL,
    position     integer NOT NULL,
    label        text NOT NULL,
    place        text NOT NULL,
    chosen       boolean NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (owner, id),
    UNIQUE (owner, position)
);

CREATE UNIQUE INDEX addresses_one_chosen ON addresses (owner) WHERE chosen;
