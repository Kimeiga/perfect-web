-- The feed's first data, the in-memory layer's (`feed.rs`): two users, a
-- post, a reply to it and a reply to the reply, and three likes.

INSERT INTO users (id, handle, name) VALUES
    ('u-ada', '@ada', 'Ada'),
    ('u-grace', '@grace', 'Grace');

INSERT INTO posts (author, text, reply_to) VALUES
    ('u-ada', 'Hello, feed.', NULL);
INSERT INTO posts (author, text, reply_to) VALUES
    ('u-grace', 'Hello, Ada.', 'p1');
INSERT INTO posts (author, text, reply_to) VALUES
    ('u-ada', 'And a reply to the reply.', 'p2');

INSERT INTO likes (post, liker) VALUES
    ('p1', 'u-grace'),
    ('p1', 'u-ada'),
    ('p3', 'u-grace');
