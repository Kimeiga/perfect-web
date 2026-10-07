# ADR-XXXX: accounts and sign-in, Pleris the relying party and the provider the deployment's

Status: proposed by track `identity` (W1, `track/identity`), under the
integrator's rulings of 2026-10-07 (docs/PARALLEL.md, "The integrator's
rulings"). Date: 2026-10-07. Milestone: E14, the owner's Twitter list, item
4 (NEXT 22's fourth).

## Context

- **`requires SignedIn` was a development model.** The server told every
  non-empty session it was signed in (ADR-0115, `Identity::requires`, pinned
  by `requires_starts_as_the_development_model`). No session had a user.
  Every guest posted as a user named for its session (ADR-0220).
- **The charter says Pleris is not an identity provider** (NEXT 22's item
  4: "OIDC behind a deployment's interface, and a local provider that is
  plainly not production"). Charter §17.5 asks tests of "CSRF for commands",
  "session fixation" and "cross-user cache leakage"; §15.6 test 12: "User
  A's cart can never be observed by User B".
- **What the server did with sessions, read before changing it:**
  - A fresh session's id was the process id plus a counter: the next
    visitor's id could be guessed.
  - The cookie was `pw-session=..; Path=/; SameSite=Lax`, readable by any
    script on the page.
  - A command was run for any request that carried a session's cookie, from
    any origin. `SameSite=Lax` held a cross-site POST back, and nothing else
    did.

## Research

- **OpenID Connect Core 1.0** (openid.net, read 2026-10-07):
  - §3.1.2.1: an authentication request carries `scope` with `openid`,
    `response_type=code`, `client_id` and `redirect_uri`; `state` and
    `nonce` are optional but recommended.
  - §3.2.2.5: "Clients MUST verify that the state value is equal to the
    value of state parameter in the Authorization Request."
  - §3.1.3.7, ID token validation: the issuer "MUST exactly match" `iss`;
    "the aud (audience) Claim contains its client_id"; "the current time
    MUST be before" `exp`; "If a nonce value was sent ... its value checked".
  - §2: `sub` is "locally unique and never reassigned ... within the
    Issuer".
- **RFC 7636, PKCE** (rfc-editor.org): §4.1, the verifier is 43 to 128
  unreserved characters; §7.1, "a 32-octet sequence"; §4.2, `S256` is
  `BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))`, and a client "capable of
  using S256 ... MUST use S256"; §4.6, a mismatch is `invalid_grant`.
  Appendix B's worked example is a test here.
- **OpenID Connect Prompt Create 1.0** (December 2022): `prompt=create`
  asks the provider to make an account and then answer as for a sign-in.
- **CSRF.** OWASP's Cross-Site Request Forgery Prevention Cheat Sheet: Fetch
  Metadata may be relied on for modern browsers, and "a fallback to standard
  origin verification headers is a mandatory requirement"; `SameSite` "does
  not replace a proper CSRF defense"; login forms are vulnerable too. Go
  1.25's `net/http.CrossOriginProtection` (pkg.go.dev) is that algorithm:
  GET, HEAD and OPTIONS pass; otherwise `Sec-Fetch-Site`, "available in all
  browsers since 2023", or "comparing the hostname of the Origin header with
  the Host header"; a request with neither is "assumed to be either
  same-origin or non-browser", and passes.
- **Sessions.** OWASP's Session Management Cheat Sheet: "at least 64 bits
  of entropy"; "The session ID must be renewed or regenerated ... after any
  privilege level change", authentication foremost; `HttpOnly`, `Secure`,
  and `SameSite=Strict` or `Lax`; on logout the server must invalidate the
  session.
- **Passwords.** OWASP's Password Storage Cheat Sheet: Argon2id, at least
  "m=19456 (19 MiB), t=2, p=1".

## Decision

### 1. What Pleris owns, and what the deployment owns

- **Pleris owns the relying party** (`identity.rs`): the authentication
  request (`state`, `nonce`, the PKCE verifier and its S256 challenge), the
  callback and its checks, the session it opens, the cookie that carries
  it, the same-origin check on every request that changes something, and
  what `requires` is told.
- **The deployment owns the provider**, behind `identity::Provider`: its
  issuer, this client's id, where a browser authenticates, and the token
  request, which exchanges a code, its verifier and the redirect URI for an
  ID token's claims. Verifying the token's signature against the provider's
  keys is the adapter's; Pleris checks the protocol's own values against
  what it sent (`check_claims`): the issuer exactly, this client in the
  audience, not expired, this sign-in's nonce, a subject.
- **The principal is the provider's `sub`**, the program's `UserId`, with
  the handle and name the provider gave (`preferred_username`, `name`).
  One issuer per deployment: two would need `(iss, sub)` (Not claimed).
- **The principal is resolved in the host, from the session**: no compiler
  change (the integrator's ruling, Q3). A data layer is handed the
  identity's principals once (`DataLayer::identified_by`, defaulted, the
  one change to the trait) and maps a session to its user.

### 2. A session, its principal, and its cookie

- **A sign-in opens a new session.** The callback gives the browser a new
  id and forgets the one it came with. A session id an attacker set
  (fixation) is never signed in, and a signed-in session that signs in again
  is signed out.
- **The sign-in is bound to the browser that started it**: its `state` is
  also in an `HttpOnly` cookie on `/sign-in`, and a callback whose state is
  not that cookie's is refused (login CSRF: an attacker's own code sent to
  someone else's browser). A state completes once, within ten minutes.
- **Signing out forgets the principal and rotates the session**: the old
  id, replayed, is no one.
- **The session cookie is `HttpOnly; SameSite=Lax`**, and `Secure` unless the
  deployment's origin (`PW_ORIGIN`) is plain HTTP on a loopback host. `Lax`,
  not `Strict`: a provider at its own origin sends the browser back by a
  cross-site top-level navigation, and a link to the feed from another site
  would read it signed out.
- **A fresh session's id is 128 bits from the operating system**
  (`getrandom`), `s-` and 32 hex digits. States, nonces, verifiers and codes
  are 256 bits, base64url.

### 3. Two development identities, and neither production

- **`guest`, the default**: every session is its own guest principal, signed
  in, named for its session, as the server held before. The store and every
  existing test run on it. A guest has no account to sign out of.
- **`dev-accounts`**: the development provider (`accounts.rs`), served by
  this server at `/dev-idp/authorize` as a deployment's provider would be at
  its own origin. Accounts in memory; a password kept only as its Argon2id
  hash (§7); a code bound to its client, redirect URI, challenge and nonce,
  exchanged once, within a minute, by its verifier; one answer for an
  unknown handle and a wrong password. Every page it serves says first that
  it is not production.
- **Both refuse to start unless the deployment says development and its
  origin is loopback** (`PW_DEPLOYMENT`, `PW_ORIGIN`), and the server's
  start says which runs (`pw dev server identity: ..`). A production
  deployment supplies its own provider. Flipping the feed's default to
  accounts is the integrator's ruling.

### 4. `requires`, evaluated

- **`SignedIn`**: the session has a principal.
- **`OwnsPost(post)`**: the session's principal wrote `post`, read through
  the command's own operations (`feed:data/posts#thread`), so within the
  command's transaction on PostgreSQL. A post not there is no one's.
- **A refusal is its own answer**: 403, `{"committed":false,
  "refused":"<predicate>"}`, never shaped like a declared error (the
  integrator's ruling, Q2). The predicate reaches the command's route on
  the thread that ran it (`identity::take_refusal`).
- **An unknown predicate is an error**, and the command does not run, as
  before.

### 5. A request that changes something comes from this origin

`Identity::answer`, asked before every route, refuses with 403 a request by
any method but GET, HEAD and OPTIONS whose `Sec-Fetch-Site` is not
`same-origin` or `none`, or, without one, whose `Origin` names another host
than `Host` (`null` included): Go 1.25's `CrossOriginProtection`. It holds
every command, sign-out, the provider's form, and the store's routes too, in
both identity models. No token: the runtime is unchanged.

### 6. The feed

- **`session query Me` reads a `Viewer`** (signed in, a guest or not, the
  provider's name) through `feed:data/users#viewer`, which the platform's
  `session.read` authorizes. The home page shows a signed-out reader "Sign
  in to post, reply or like." with Sign in and Sign up where the form's
  guard is, and a signed-in one who they are and Sign out.
- **A post, a reply and a like are the principal's user's**, in memory and
  on PostgreSQL, the author's handle and name written with what they write.
- **`command delete(post) requires SignedIn, OwnsPost(post)`**: the post and
  every reply under it (PostgreSQL: one statement, their likes with them).
  `Item.mine`, private as the timeline is, shows Delete on the author's rows
  alone. `Deleted(_)` drops every timeline and thread.
- **One module**: the integrator asked for a module of the track's own,
  `feed.accounts`. `delete` needs app.pw's `PostId` and `FeedError`, and the
  page needs `Me`: each way round is an import cycle (PW0025), and ten
  compiler tests read app.pw as the whole program. So the track's
  declarations are in app.pw, each marked.

### 7. Dependencies

- **argon2 0.6.0** (RustCrypto, 2026-08-27): the newest on crates.io, not
  yanked, MIT OR Apache-2.0, rust-version 1.85, checked 2026-10-07 on
  crates.io, on docs.rs for 0.6.0 and in its source. **The owner approved
  its download on 2026-10-07.** Argon2id v19 at the crate's defaults, which
  are OWASP's minimum (`Params::DEFAULT_M_COST = 19 * 1024`, `T = 2`,
  `P = 1`), PHC strings through password-hash 0.6
  (`PasswordHasher::hash_password`, `phc::PasswordHash::new`,
  `PasswordVerifier::verify_password`). It brings password-hash 0.6.1, phc
  0.6.1, blake2 0.11.0, base64ct 1.8.3, cpufeatures 0.3.0. Its own commit.
- **sha2 0.11.0, getrandom 0.4.3, base64 0.22.1**: already in `Cargo.lock`,
  taken at the lock's newest versions (the integrator's ruling); nothing
  downloaded. `just audit` is green.

## Alternatives

- **A synchronizer or double-submit CSRF token.** It needs the runtime to
  send it and every page to carry it; OWASP holds Fetch Metadata with the
  Origin fallback enough for modern browsers. Kept for a deployment that
  must serve browsers without either header.
- **An authentication cookie beside `pw-session`.** Two principals behind
  one `pw-session` would share every private entry keyed by the session.
  Rotating `pw-session` itself keeps one session, one principal.
- **`SameSite=Strict`**: see §2.
- **No passwords in the development provider**, an account picked by its
  handle: the integrator's fallback had the owner declined argon2.
- **The development provider as a separate process at its own origin**:
  closer to a deployment's, and a second server for every test. The
  interface is the same either way: the server reaches it only through
  `Provider` and `Provider::serve`.

## Consequences

- **What the program can say of a user is what the host resolves**: a
  `UserId` from a data layer op, keyed by the session. A typed principal in
  the language would let a program state `requires` over a principal and
  check it (Questions).
- **The runtime shows a refused command as its generic failure** where a
  page offered what its viewer cannot do (a tab signed out elsewhere); the
  feed's pages offer only what the viewer can.
- **Every unsafe request is held to its origin**, the store's included.

## Acceptance

Recorded by `just e14-identity` in `docs/evidence/E14/identity.txt`:

- **The server**, `identity.rs` (8), `accounts.rs` (6) and
  `tests/sign_in.rs` (15, two of them on PostgreSQL where
  `PW_FEED_DATABASE_URL` names one, passing doing nothing otherwise):
  - RFC 7636 Appendix B; cookies; loopback; Go's same-origin decision; the
    ID token's checks; the guest model refused outside development.
  - The development provider refused outside development; a request it
    cannot verify answered at the provider; a code exchanged once by its
    verifier; a password kept only as its Argon2id hash; one answer for an
    unknown handle and a wrong password; an account checked when made.
  - Over HTTP: a sign-in rotates the session and forgets the old one; a
    callback another browser started refused; forged tokens refused;
    sign-out forgets; a signed-out reader reads and is refused 403 with
    `SignedIn` for a post, a like and a reply; attribution; only its author
    deletes a post (`OwnsPost`); a command from another origin refused; a
    fresh cookie `HttpOnly` and unguessable; **two users' pages share
    nothing private**: each says who it is and not the other, and what the
    query runtime keeps for every reader and the materializer's public
    fragments name neither session nor reader, and equal what they are with
    no one signed in; the development provider end to end; the guest model.
- **The browser, in Chromium, Firefox and WebKit** (`e2e/identity.spec.mjs`,
  9 tests), and the feed's own suite unchanged (`e2e/feed.spec.mjs`).
- **`scripts/identity_mutations.py`: 16 mutants, 16 killed**: no CSRF
  check; the session kept at sign-in (fixation); the replaced session left
  signed in; sign-out leaving it signed in; the callback not bound to its
  browser; the nonce unchecked; every session `SignedIn`; the ownership
  predicate inverted; a refusal answered as a command that did not commit;
  the cookie readable by scripts; a guessable session id; the guest model
  and the development provider in production; a code exchanged without its
  verifier; a password stored as typed; a post its session's guest's.
- **The whole server's tests**, 189 (PostgreSQL's included, against 18.6
  locally), and `just ci`.

## Not claimed

- **A production provider adapter**: no OIDC discovery, no JWKS, no ID
  token signature verification here. `Provider` is the seam; the
  development provider and the tests' provider implement it in process.
- **A refusal shown by the runtime**: a stale tab's refused command is the
  runtime's generic failure (the integrator queues it in NEXT).
- **Sessions that outlive the process**, expire, or are listed and revoked;
  the principals are in memory. A session's server-side documents are not
  purged at sign-out (its id is no one after it).
- **More than one issuer** (`(iss, sub)` as the key), account linking,
  email, recovery, or rate limits on sign-in.
- **Browsers without `Sec-Fetch-Site` and `Origin`** on a cross-site POST:
  none of the three engines is one.
- **The feed's default flipped to accounts**, and the store on accounts.
- **`e14-identity` on CI's database job**: CI plans it in a browser shard,
  where the PostgreSQL tests pass doing nothing; they ran locally.

## Questions for the integrator

1. **A typed principal in the language** (Q3's later ruling). The case: a
   `requires` predicate is today a name the deployment interprets, and
   `OwnsPost` reads a post's author through an op it must know by name.
   With `context.current_principal() -> Option<UserId>` (a
   `pw:host/principal#read` op the host answers from the session) a program
   could write ownership as a predicate over its own data, and a data layer
   would not need `identified_by`. A language ADR, yours.
2. **Routes a deployment serves are not in the route table**: `/sign-in`,
   `/sign-up` and `/sign-out` are the host's, so the feed reaches them by
   forms (`method="get"` and `"post"`), which PW5009 does not check. Should
   the platform declare them, so a link to them is checked?
3. **`e14-identity` and the database job**: add it to `NEEDS_DATABASE` if the
   database job installs browsers, so its PostgreSQL tests run on CI.
4. **Two compiler tests** (`read_whole.rs`, `clauses_read_once.rs`) pin the
   thread's listeners as written in app.pw; they now count `Deleted`.

## Report

**Built.** `identity.rs` (the relying party, principals, cookies, ids, the
same-origin check, `requires`), `accounts.rs` (the development provider),
the feed's accounts in both data layers, `e2e/identity.spec.mjs`,
`scripts/identity_mutations.py`, `just/identity.just` (`e14-identity`).
`main.rs` is touched only at lines marked `TRACK SEAM (identity)`: the
`requires` closure, the three session cookies, a fresh session's id, the
server's identity built and handed to the layer, the refusal's answer, the
start's configuration and its line, and the tests' module.

**Tests and mutants.** `docs/evidence/E14/identity.txt` (`just
e14-identity` at b38d10d, PostgreSQL 18.6 named): the server's 29 track
tests pass, PostgreSQL's two among them; the browser suite 9 of 9 in three
engines; 16 of 16 mutants killed. The whole server, 189 tests, and `just
ci` and `just audit` green at b38d10d.

**Not claimed.** See Not claimed: no production provider adapter, no
refusal shown by the runtime, sessions in memory, one issuer, and the
feed's default left to the guest model.

**Merge notes.**
- Commits, in order: argon2 alone; the identity; the feed's accounts; a
  guest has no Sign out; the browser suite, the mutants and the recipe;
  this ADR and its evidence.
- Shared files touched: `Cargo.toml` and `Cargo.lock`, the server's
  `Cargo.toml`, `data.rs` (one defaulted method), `feed.rs`, `feed_pg.rs`,
  `examples/feed/app.pw`, `playwright.config.mjs` (`IDENTITY_PORTS`,
  PORT+70..72), two compiler tests, and `computed_rows_mutations.py`'s
  anchor (`Item.mine`).
- `PW_IDENTITY`, `PW_DEPLOYMENT` and `PW_ORIGIN` are new environment
  variables of the development server.
- No diagnostic code was needed: the track's checks are the host's.
