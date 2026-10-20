-- **Sessions and sign-ins kept where the hosts share them** (ADR-0369, track
-- `second-host`): a session a sign-in opened, by its id's SHA-256 (the id
-- itself, the cookie's value, is never written), and the principal it is; a
-- sign-in under way, by its state's SHA-256, with its nonce and its PKCE
-- verifier, which the token request needs as they are, taken once by the
-- callback on whichever host it reaches, within ten minutes.

CREATE TABLE sessions (
    hash    bytea PRIMARY KEY CHECK (octet_length(hash) = 32),
    user_id text NOT NULL,
    handle  text NOT NULL,
    name    text NOT NULL,
    issuer  text NOT NULL,
    opened  timestamptz NOT NULL DEFAULT clock_timestamp(),
    -- When it was last used, by which it expires idle (the integrator's
    -- ruling 2 of 2026-10-20 on this track); `opened` by its lifetime.
    seen    timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE sign_ins (
    hash     bytea PRIMARY KEY CHECK (octet_length(hash) = 32),
    nonce    text NOT NULL,
    verifier text NOT NULL,
    started  timestamptz NOT NULL DEFAULT clock_timestamp()
);
