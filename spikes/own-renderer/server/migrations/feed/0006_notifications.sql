-- What others do that involves a user (track `notifications`, ADR-XXXX): a
-- row a like, a reply or a follow writes in its own transaction, beside what
-- it writes, as a post's image is the post's (ADR-0260). 0005 is the
-- uploads'.
--
-- - One's own act notifies no one: a row never names its actor as whom it
--   is for.
-- - A like's is about the post liked, a reply's about the reply; a follow's
--   about no post.
-- - A deleted post's notifications go with it: the post deleted, its rows
--   are, in the same statement.
-- - A session's guest has no row in `users` (ADR-0220), so neither user is a
--   foreign key, as a post's author is not.

CREATE SEQUENCE notification_seq;

CREATE TABLE notifications (
    seq          bigint PRIMARY KEY DEFAULT nextval('notification_seq'),
    id           text GENERATED ALWAYS AS ('n' || seq::text) STORED UNIQUE,
    recipient    text NOT NULL,
    actor        text NOT NULL,
    act          text NOT NULL CHECK (act IN ('liked', 'replied', 'followed')),
    post         text REFERENCES posts (id) ON DELETE CASCADE,
    read         boolean NOT NULL DEFAULT false,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    CHECK (recipient <> actor),
    CHECK ((act = 'followed') = (post IS NULL))
);

-- A user's notifications, newest first, and their unread ones counted.
CREATE INDEX notifications_recipient ON notifications (recipient, seq DESC);
CREATE INDEX notifications_unread ON notifications (recipient) WHERE NOT read;
-- What a post's deletion takes with it.
CREATE INDEX notifications_post ON notifications (post);
