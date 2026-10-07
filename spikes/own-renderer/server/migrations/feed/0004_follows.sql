-- Who follows whom (ADR-0257): a row a follow writes and an unfollow
-- deletes, the follower first. A session's guest has no row in `users`
-- (ADR-0220), so no foreign key, as a post's author has none. 0003 is the
-- identity track's (docs/PARALLEL.md).

CREATE TABLE follows (
    follower     text NOT NULL,
    followee     text NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (follower, followee),
    CHECK (follower <> followee)
);

-- A user's followers, counted by the one followed.
CREATE INDEX follows_followee ON follows (followee);

-- A user's posts that reply to none, newest first: their page, and the
-- following timelines that read them.
CREATE INDEX posts_author ON posts (author, seq DESC) WHERE reply_to IS NULL;
