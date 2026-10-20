-- **The outbox is a log every host reads** (ADR-0369, track `second-host`):
-- a commit's rows are kept once delivered, not deleted, so that each host on
-- the database tells its own pages of every other host's commits. `id` is in
-- commit order: a commit takes the outbox's lock before its rows take their
-- ids, and holds it to its end. `host` is the committing host's id, whose own
-- reader passes its rows by (it told its pages at once); `created` is the
-- database's clock at the insert, by which rows are trimmed. `outbox_trimmed`
-- holds the highest id trimmed: a host whose cursor is below it missed rows it
-- can no longer read, and tells its pages to reload.

ALTER TABLE outbox ADD COLUMN host text NOT NULL DEFAULT '';
ALTER TABLE outbox ADD COLUMN created timestamptz NOT NULL DEFAULT clock_timestamp();
CREATE INDEX outbox_created ON outbox (created);

CREATE TABLE outbox_trimmed (
    one     boolean PRIMARY KEY DEFAULT true CHECK (one),
    through bigint NOT NULL
);
INSERT INTO outbox_trimmed (through) VALUES (0);

-- **Each host's instance** (the integrator's ruling 1 of 2026-10-20 on this
-- track): a number each host takes as it opens the layer, put in the high bits
-- of every page number it gives, so that no two hosts on the database number
-- a page alike and none takes another's page for its own.
CREATE SEQUENCE host_instances;
