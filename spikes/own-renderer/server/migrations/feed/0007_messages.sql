-- Direct messages (track `messages`, ADR-0279): a row each, written in its
-- command's transaction, and a read mark per reader and other user. A message
-- is not a post, and writes no notification. 0006 is the notifications'.
--
-- - No one messages themselves: a row never names its sender as its
--   recipient. Who may message whom is `requires MayMessage(to)`'s, read in
--   the command's own transaction before it writes.
-- - Its text is 1 to 10,000 characters, as the program's `MessageText` is:
--   `char_length` counts code points, as `String.length` does.
-- - A reader's mark is the last message they have read of their
--   conversation with the other: sending sets the sender's; "Mark read" sets
--   the reader's.
-- - A session's guest has no row in `users` (ADR-0220), so neither user is a
--   foreign key, as a post's author is not.

CREATE SEQUENCE message_seq;

CREATE TABLE messages (
    seq          bigint PRIMARY KEY DEFAULT nextval('message_seq'),
    id           text GENERATED ALWAYS AS ('m' || seq::text) STORED UNIQUE,
    sender       text NOT NULL,
    recipient    text NOT NULL,
    text         text NOT NULL CHECK (char_length(text) BETWEEN 1 AND 10000),
    sent_at      timestamptz NOT NULL DEFAULT now(),
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    CHECK (sender <> recipient)
);

-- A conversation, newest first, whichever of the two reads it; a reader's
-- list and count; and who has messaged whom, which `MayMessage` reads.
CREATE INDEX messages_pair
    ON messages (least(sender, recipient), greatest(sender, recipient), seq DESC);
CREATE INDEX messages_recipient ON messages (recipient, sender, seq);
CREATE INDEX messages_sender ON messages (sender, seq);

CREATE TABLE message_reads (
    reader       text NOT NULL,
    other        text NOT NULL,
    seq          bigint NOT NULL,
    committed_in xid8 NOT NULL DEFAULT pg_current_xact_id(),
    PRIMARY KEY (reader, other),
    CHECK (reader <> other)
);
