-- The feed's data in PostgreSQL (ADR-0246): users, posts with their replies,
-- likes, and the transactional outbox. Applied once, in order, by
-- `feed_pg.rs`, which records each in `pw_migrations`.
--
-- Every row a command writes records the transaction that wrote it
-- (`committed_in`), so a test can show a post and its event committed in
-- one transaction, and not merely both committed.

CREATE TABLE users (
    id     text PRIMARY KEY,
    handle text NOT NULL,
    name   text NOT NULL
);

-- A post's id is `p` and its place in the order posts were written: the
-- in-memory layer's ids, from a sequence, so two commands never compute one.
CREATE SEQUENCE post_seq;

CREATE TABLE posts (
    seq          bigint PRIMARY KEY DEFAULT nextval('post_seq'),
    id           text GENERATED ALWAYS AS ('p' || seq::text) STORED UNIQUE,
    -- A session's guest has no row in `users` (ADR-0220), so no foreign key.
    author       text NOT NULL,
    text         text NOT NULL CHECK (char_length(text) BETWEEN 1 AND 280),
    reply_to     text REFERENCES posts (id),
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id()
);

CREATE INDEX posts_timeline ON posts (seq DESC) WHERE reply_to IS NULL;
CREATE INDEX posts_replies ON posts (reply_to);

-- Each like a row: two likes of one post insert two rows, and neither
-- updates a count the other read.
CREATE TABLE likes (
    seq          bigserial PRIMARY KEY,
    post         text NOT NULL REFERENCES posts (id),
    liker        text NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id()
);

CREATE INDEX likes_post ON likes (post);

-- The transactional outbox (ADR-0007, ADR-0208, ADR-0209): what a command
-- handed the platform, committed with its writes or not at all. `args` keeps
-- each value as the command computed it, a JSON string, number or boolean,
-- so an `Int` key is read back an `Int`. A row is deleted once delivered.
CREATE TABLE outbox (
    id           bigserial PRIMARY KEY,
    kind         text NOT NULL CHECK (kind IN ('event', 'invalidation')),
    name         text NOT NULL,
    args         jsonb NOT NULL CHECK (jsonb_typeof(args) = 'array'),
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id()
);
