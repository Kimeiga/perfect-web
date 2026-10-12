-- An interaction's answer, committed with its command's writes (ADR-0325,
-- PW0348): a retry of the interaction, by this host after it restarted or by
-- another host, is answered with it, and the command runs once. Its key is the
-- session, the command and the interaction; `sent` the arguments it was first
-- sent with, which a retry must repeat; `answer` what it answered, as ADR-0121
-- keeps it. Kept a day: each commit forgets those older (`at_ms`, the host's
-- clock, in milliseconds since 1970).

CREATE TABLE interactions (
    key    text PRIMARY KEY,
    sent   text NOT NULL,
    answer text NOT NULL,
    at_ms  bigint NOT NULL
);

CREATE INDEX interactions_at ON interactions (at_ms);
