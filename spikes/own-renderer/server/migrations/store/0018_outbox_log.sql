-- **The outbox is a log every host reads** (ADR-XXXX, track `second-host`):
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
