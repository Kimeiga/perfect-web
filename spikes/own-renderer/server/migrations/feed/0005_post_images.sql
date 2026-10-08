-- A post's image (track `uploads`, ADR-0260): kept with the post, written in
-- the post's own transaction. Its bytes are the deployment's blob storage's,
-- by their SHA-256; what is kept here is that key, the kind sniffed from the
-- bytes, the width and height a browser shows them at, and the author's
-- words for them. A post has all five or none. 0004 is the follows'.

ALTER TABLE posts
    ADD COLUMN image_key    text CHECK (image_key ~ '^[0-9a-f]{64}$'),
    ADD COLUMN image_kind   text CHECK (image_kind IN ('png', 'jpeg', 'webp', 'gif')),
    ADD COLUMN image_width  integer CHECK (image_width > 0),
    ADD COLUMN image_height integer CHECK (image_height > 0),
    ADD COLUMN image_alt    text CHECK (char_length(image_alt) BETWEEN 1 AND 1000),
    ADD CONSTRAINT posts_image_whole CHECK (
        num_nonnulls(image_key, image_kind, image_width, image_height, image_alt) IN (0, 5)
    );
