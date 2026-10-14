-- A run a command scheduled (the scheduled-commands track): written with the
-- scheduling command's writes, or not at all; one pending for each command
-- and its arguments (`key`), which a scheduling again replaces; removed once
-- the run is answered. `args` are the command's arguments as a press sends
-- them (a JSON array), `user_id` the user on whose behalf it runs, `due_ms`
-- when, on the host's clock, in milliseconds since 1970. A run that trapped
-- or was refused runs again, `attempts` counting them; `failed` says why one
-- was kept, failed for good, and never run again.

CREATE TABLE schedules (
    id       bigserial PRIMARY KEY,
    key      text NOT NULL UNIQUE,
    command  text NOT NULL,
    args     text NOT NULL,
    user_id  text NOT NULL,
    due_ms   bigint NOT NULL,
    attempts bigint NOT NULL DEFAULT 0,
    failed   text
);

CREATE INDEX schedules_due ON schedules (due_ms) WHERE failed IS NULL;
