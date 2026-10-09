//! **Who a request is, and what `requires` is told** (track `identity`,
//! `docs/PARALLEL.md`, ADR-0253, ADR-0258).
//!
//! The identity track owns this module: sign-up, sign-in and sign-out, a
//! session's principal, the session cookie, the check that a command comes
//! from the page's own origin, and the `requires` evaluator. `main.rs` reaches
//! it only at lines marked `TRACK SEAM (identity)`.
//!
//! **What Pleris owns, and what a deployment owns** (ADR-0258). Pleris is not
//! an identity provider (the charter). It owns the relying party's half of an
//! OpenID Connect authorization code flow with PKCE: the `state`, the
//! `nonce` and the code verifier, the callback and its checks, the session it
//! opens, the cookie that carries it, and what `requires` is told. A
//! deployment owns the provider, behind [`Provider`]: where a browser is sent
//! to sign in, and the exchange of a code for an ID token's verified claims.
//!
//! Two providers are here, and both are for development only:
//! - **[`Mode::Guest`]**, the model this server held before the track: every
//!   session is its own guest principal, so every session is `SignedIn`.
//!   The store and the feed's existing tests run on it.
//! - **The development provider** (`accounts.rs`), served by this server at
//!   `/dev-idp/…` as an external provider would be. It refuses to start
//!   outside a development deployment on a loopback origin.

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pw_host::engine::{HostFn, Val};

// ---------------------------------------------------------------------------
// The deployment
// ---------------------------------------------------------------------------

/// **What the deployment says it is**: development or not, and the origin
/// its pages are served from. A cookie is `Secure` unless that origin is a
/// loopback one, and a development provider runs only on one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployment {
    pub development: bool,
    /// `scheme://host[:port]`, no path: `http://127.0.0.1:3143`.
    pub origin: String,
}

impl Deployment {
    /// A development deployment on this machine, at `port`.
    #[cfg(test)]
    pub fn development(port: u16) -> Deployment {
        Deployment {
            development: true,
            origin: format!("http://127.0.0.1:{port}"),
        }
    }

    /// **The deployment the environment states.** `PW_DEPLOYMENT` is
    /// `development` (or unset: this binary is the development server) or
    /// `production`; `PW_ORIGIN` is the public origin, by default this
    /// machine's at `port`. Anything else is refused, not guessed.
    pub fn from_env(port: u16) -> Result<Deployment, String> {
        let development = match std::env::var("PW_DEPLOYMENT").as_deref() {
            Err(_) | Ok("") | Ok("development") => true,
            Ok("production") => false,
            Ok(other) => {
                return Err(format!(
                    "PW_DEPLOYMENT is `{other}`: it is `development` or `production`"
                ));
            }
        };
        let origin = match std::env::var("PW_ORIGIN") {
            Ok(o) if !o.is_empty() => o.trim_end_matches('/').to_string(),
            _ => format!("http://127.0.0.1:{port}"),
        };
        if !(origin.starts_with("http://") || origin.starts_with("https://")) {
            return Err(format!("PW_ORIGIN `{origin}` is not an http(s) origin"));
        }
        Ok(Deployment {
            development,
            origin,
        })
    }

    /// The origin's host, without its scheme or port.
    fn host(&self) -> &str {
        let rest = self
            .origin
            .split_once("://")
            .map(|(_, r)| r)
            .unwrap_or(&self.origin);
        let authority = rest.split('/').next().unwrap_or(rest);
        if let Some(v6) = authority.strip_prefix('[') {
            return v6.split(']').next().unwrap_or(v6);
        }
        authority.split(':').next().unwrap_or(authority)
    }

    /// **Is the origin this machine's?** `localhost`, a name under
    /// `.localhost`, `127.0.0.1` or `::1` (RFC 6761 §6.3 reserves
    /// `localhost` for the loopback).
    pub fn loopback(&self) -> bool {
        let host = self.host().to_ascii_lowercase();
        host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" || host == "::1"
    }

    /// **Does a cookie need `Secure`?** Everywhere but a plain-HTTP origin on
    /// this machine, where a browser could not be sent one back.
    pub fn secure_cookies(&self) -> bool {
        self.origin.starts_with("https://") || !self.loopback()
    }
}

/// Whether this process's cookies are `Secure`, set once by `main` from the
/// deployment ([`configure_cookies`]). A test's server never sets it, and is
/// a loopback one.
static SECURE_COOKIES: OnceLock<bool> = OnceLock::new();

/// **Set this process's cookie attributes from its deployment.** Once.
pub fn configure_cookies(deployment: &Deployment) {
    let _ = SECURE_COOKIES.set(deployment.secure_cookies());
}

// ---------------------------------------------------------------------------
// Cookies and ids
// ---------------------------------------------------------------------------

/// The session cookie's name.
pub const SESSION_COOKIE: &str = "pw-session";
/// The cookie binding a sign-in in flight to the browser that started it.
const SIGN_IN_COOKIE: &str = "pw-sign-in";

/// **A `Set-Cookie` line**: `HttpOnly`, so no script reads it; `SameSite=Lax`,
/// so a cross-site request carries it only on a top-level navigation by a
/// safe method (which a provider's redirect back is); and `Secure` outside
/// this machine. `max_age: Some(0)` removes it.
pub fn cookie_line(
    name: &str,
    value: &str,
    path: &str,
    max_age: Option<u64>,
    secure: bool,
) -> String {
    let mut line = format!("set-cookie: {name}={value}; Path={path}; HttpOnly; SameSite=Lax");
    if let Some(age) = max_age {
        line.push_str(&format!("; Max-Age={age}"));
    }
    if secure {
        line.push_str("; Secure");
    }
    line.push_str("\r\n");
    line
}

/// **The cookie that names `session`**, as every response that issues one
/// sends it (`main.rs`'s seams).
pub fn session_cookie(session: &str) -> String {
    cookie_line(
        SESSION_COOKIE,
        session,
        "/",
        None,
        *SECURE_COOKIES.get().unwrap_or(&false),
    )
}

/// `n` bytes from the operating system's random source.
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the operating system's random source");
    bytes
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// **A fresh session id**: 128 bits from the operating system's random
/// source (OWASP's Session Management Cheat Sheet asks at least 64 bits of
/// entropy). Until the identity track a fresh session was the process id
/// plus a counter, which the next visitor could guess.
pub fn new_session_id() -> String {
    format!("s-{}", hex(&random_bytes::<16>()))
}

/// **An unguessable token**: 256 bits, base64url without padding, 43
/// characters (RFC 7636 §4.1's code verifier is exactly this).
pub fn token() -> String {
    base64url(&random_bytes::<32>())
}

/// Base64url without padding (RFC 4648 §5), as RFC 7636 §3 defines it.
pub fn base64url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// **PKCE's S256** (RFC 7636 §4.2):
/// `BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))`.
pub fn s256(verifier: &str) -> String {
    use sha2::Digest;
    base64url(&sha2::Sha256::digest(verifier.as_bytes()))
}

/// Two secrets compared in time that does not depend on where they differ.
pub(crate) fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// RFC 7636 §4.1: 43 to 128 unreserved characters.
pub(crate) fn is_verifier(v: &str) -> bool {
    (43..=128).contains(&v.len())
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Principals
// ---------------------------------------------------------------------------

/// **Who a session is** (ADR-0258): the user a provider vouched for. Its
/// `user` is the program's `UserId`: the provider's `sub`, "locally unique
/// and never reassigned identifier within the Issuer" (OpenID Connect Core
/// §2), or a guest's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub user: String,
    pub handle: String,
    pub name: String,
    /// The provider's issuer, or `guest`.
    pub issuer: String,
}

impl Principal {
    /// **A session's own guest** (the guest model): named for its session,
    /// as the feed named one before accounts (ADR-0220).
    pub fn guest(session: &str) -> Principal {
        Principal {
            user: format!("u-{session}"),
            handle: format!("@{session}"),
            name: format!("Guest {session}"),
            issuer: "guest".to_string(),
        }
    }

    pub fn is_guest(&self) -> bool {
        self.issuer == "guest"
    }
}

#[derive(Debug, Default)]
struct PrincipalsInner {
    /// The accounts model: a session is signed in only where a sign-in put
    /// it in `sessions`. Otherwise every session is its own guest.
    accounts: AtomicBool,
    sessions: RwLock<HashMap<String, Principal>>,
}

/// **Each session's principal**, shared by the identity and the data layer
/// that maps a session to its user (`DataLayer::identified_by`).
#[derive(Debug, Clone, Default)]
pub struct Principals(Arc<PrincipalsInner>);

impl Principals {
    /// **The principal `session` is**, or none. A session no sign-in opened
    /// is its own guest in the guest model, and no one in the accounts one.
    pub fn of(&self, session: &str) -> Option<Principal> {
        if session.is_empty() {
            return None;
        }
        if let Some(p) = self.0.sessions.read().expect("principals").get(session) {
            return Some(p.clone());
        }
        (!self.0.accounts.load(Ordering::SeqCst)).then(|| Principal::guest(session))
    }

    /// The user `session` acts as: its principal's, or its guest's, who can
    /// write nothing where `requires SignedIn` is held.
    pub fn user_of(&self, session: &str) -> String {
        self.of(session)
            .map(|p| p.user)
            .unwrap_or_else(|| format!("u-{session}"))
    }

    fn open(&self, session: &str, principal: Principal) {
        self.0
            .sessions
            .write()
            .expect("principals")
            .insert(session.to_string(), principal);
    }

    fn close(&self, session: &str) {
        self.0.sessions.write().expect("principals").remove(session);
    }

    fn set_accounts(&self, on: bool) {
        self.0.accounts.store(on, Ordering::SeqCst);
    }

    /// How many sessions are signed in.
    #[cfg(test)]
    pub fn signed_in(&self) -> usize {
        self.0.sessions.read().expect("principals").len()
    }
}

// ---------------------------------------------------------------------------
// The provider interface a deployment supplies
// ---------------------------------------------------------------------------

/// **An ID token's claims, as the provider adapter verified them**
/// (OpenID Connect Core §2). The adapter checks the token's signature
/// against the provider's keys; Pleris checks the protocol's own values
/// against what it sent ([`Identity`]'s callback, Core §3.1.3.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    pub iss: String,
    pub sub: String,
    pub aud: Vec<String>,
    /// Seconds since the epoch.
    pub exp: u64,
    pub nonce: Option<String>,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
}

/// **An OpenID provider, as a deployment supplies it** (ADR-0258): the
/// authorization code flow with PKCE (OpenID Connect Core §3.1, RFC 7636).
pub trait Provider: Send + Sync {
    /// The issuer identifier, which an ID token's `iss` must equal exactly.
    fn issuer(&self) -> String;
    /// This relying party's client id at the provider.
    fn client_id(&self) -> String;
    /// Where a browser is sent to authenticate: an absolute URL, or a path
    /// on this origin.
    fn authorization_endpoint(&self) -> String;
    /// **The token request** (Core §3.1.3.1, RFC 7636 §4.5): a code, the
    /// verifier its challenge was made from, and the redirect URI it was
    /// issued for, exchanged for the ID token's verified claims.
    fn exchange(&self, code: &str, verifier: &str, redirect_uri: &str) -> Result<Claims, String>;

    /// **A request to the provider itself**, where this server serves it
    /// (the development provider's pages). A deployment's provider is at its
    /// own origin, and serves nothing here.
    fn serve(&self, _method: &str, _route: &str, _query: &str, _body: &[u8]) -> Option<Reply> {
        None
    }
}

/// `PW_IDENTITY`'s name for the development provider.
pub const DEV_ACCOUNTS: &str = "dev-accounts";

/// **The authentication request a browser is sent with** (Core
/// §3.1.2.1, RFC 7636 §4.3): `scope openid`, `response_type code`, the
/// client and its redirect URI, `state`, `nonce`, and the S256 challenge.
/// `prompt=create` asks for an account to be made (OpenID Connect Prompt
/// Create 1.0).
pub fn authorization_url(
    provider: &dyn Provider,
    redirect_uri: &str,
    state: &str,
    nonce: &str,
    challenge: &str,
    sign_up: bool,
) -> String {
    let mut url = format!(
        "{}?response_type=code&scope=openid%20profile&client_id={}&redirect_uri={}\
         &state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
        provider.authorization_endpoint(),
        encode(&provider.client_id()),
        encode(redirect_uri),
        encode(state),
        encode(nonce),
        encode(challenge),
    );
    if sign_up {
        url.push_str("&prompt=create");
    }
    url
}

// ---------------------------------------------------------------------------
// The identity: the relying party, and `requires`
// ---------------------------------------------------------------------------

/// Where a provider sends a browser back.
pub const CALLBACK: &str = "/sign-in/callback";

/// **A guest, signed in** (track `store-accounts`, ADR-XXXX; docs/PARALLEL.md,
/// Q2): the session the browser came with, which no sign-in had opened, the
/// session the sign-in opened in its place, and its principal. What a
/// deployment is told once the principal is opened and before the sign-in is
/// answered, so that what the guest held (a cart) is the user's before the
/// browser asks for a page. The identity knows nothing of what it is.
pub struct SignedIn<'a> {
    pub guest: &'a str,
    pub session: &'a str,
    pub principal: &'a Principal,
}
/// How long a sign-in may be in flight.
const SIGN_IN_LIFETIME: Duration = Duration::from_secs(600);

/// **Which principals a deployment has.**
pub enum Mode {
    /// Every session is its own guest principal: the development model this
    /// server held before the track (ADR-0115), for the store and the
    /// feed's existing tests.
    Guest,
    /// Sessions are signed in through a provider.
    Accounts(Accounts),
}

/// The accounts model: a provider, this client's redirect URI, and each
/// sign-in in flight.
pub struct Accounts {
    provider: Arc<dyn Provider>,
    redirect_uri: String,
    /// By `state`: the nonce and the code verifier, and when it started.
    pending: Mutex<HashMap<String, Pending>>,
}

struct Pending {
    nonce: String,
    verifier: String,
    started: Instant,
}

/// What the identity track keeps for one server.
pub struct Identity {
    principals: Principals,
    mode: RwLock<Mode>,
}

impl Default for Identity {
    fn default() -> Identity {
        Identity {
            principals: Principals::default(),
            mode: RwLock::new(Mode::Guest),
        }
    }
}

impl Identity {
    /// Each session's principal, which the data layer maps a session to
    /// its user by.
    pub fn principals(&self) -> Principals {
        self.principals.clone()
    }

    /// **The deployment's identity, as its configuration states it**:
    /// `PW_IDENTITY` is `guest` (or unset) for the guest model, or
    /// `dev-accounts` for the development provider. Neither runs outside a
    /// development deployment on this machine: a production deployment
    /// supplies a [`Provider`] of its own.
    pub fn configure(&self, deployment: &Deployment, choice: Option<&str>) -> Result<(), String> {
        match choice.unwrap_or("guest") {
            "" | "guest" => {
                if !deployment.development || !deployment.loopback() {
                    return Err(format!(
                        "the guest identity model signs every session in, and runs only in a \
                         development deployment on this machine, not at `{}`; a deployment \
                         supplies its own provider",
                        deployment.origin
                    ));
                }
                self.use_guests();
                Ok(())
            }
            DEV_ACCOUNTS => self.use_dev_accounts(deployment),
            other => Err(format!(
                "PW_IDENTITY is `{other}`: this server knows `guest` and `{DEV_ACCOUNTS}`"
            )),
        }
    }

    /// **Which model runs**, as the server's start says it.
    pub fn describe(&self) -> String {
        match &*self.mode.read().expect("mode") {
            Mode::Guest => "guest (development: every session is its own guest)".to_string(),
            Mode::Accounts(a) => format!("accounts, signed in through {}", a.provider.issuer()),
        }
    }

    /// The guest model.
    pub fn use_guests(&self) {
        *self.mode.write().expect("mode") = Mode::Guest;
        self.principals.set_accounts(false);
    }

    /// **Sign-in through `provider`**, sent back to `redirect_uri`: the
    /// accounts model. Every session signed in before is signed out.
    pub fn use_provider(&self, provider: Arc<dyn Provider>, redirect_uri: &str) {
        *self.mode.write().expect("mode") = Mode::Accounts(Accounts {
            provider,
            redirect_uri: redirect_uri.to_string(),
            pending: Mutex::new(HashMap::new()),
        });
        self.principals.set_accounts(true);
    }

    /// **`session` signed in as `user`, without the flow** (track
    /// `notifications`): the accounts model, a test's principal, so a test
    /// holds one user in two sessions and another user beside them. The
    /// flow itself is `tests/sign_in.rs`'s to hold.
    #[cfg(test)]
    pub fn signed_in_for_test(&self, session: &str, user: &str, name: &str) {
        self.principals.set_accounts(true);
        self.principals.open(
            session,
            Principal {
                user: user.to_string(),
                handle: format!("@{user}"),
                name: name.to_string(),
                issuer: "https://id.example.test".to_string(),
            },
        );
    }

    /// **A request the identity track answers**, before any other route.
    /// `true` when it answered on `stream`; `false` leaves the request to
    /// the routes after it. `on_sign_in` is told of a guest that signed in
    /// ([`SignedIn`]), before the sign-in is answered.
    ///
    /// First, for every route, **a request that changes something is
    /// refused unless it comes from this origin** (charter §17.5's "CSRF for
    /// commands"): Fetch Metadata's `Sec-Fetch-Site`, and `Origin` where a
    /// browser sends no `Sec-Fetch-Site`, as Go 1.25's
    /// `http.CrossOriginProtection` decides it.
    #[allow(clippy::too_many_arguments)]
    pub fn answer(
        &self,
        method: &str,
        route: &str,
        headers: &str,
        session: &str,
        fresh: bool,
        body: &[u8],
        stream: &mut TcpStream,
        query: &str,
        on_sign_in: &dyn Fn(&SignedIn),
    ) -> bool {
        if let Err(why) = same_origin(method, headers) {
            Reply::text(403, &why).write(stream, session, fresh);
            return true;
        }
        let reply = match (method, route) {
            ("POST", "/sign-out") => Some(self.sign_out(session)),
            ("GET", "/sign-in") | ("GET", "/sign-up") => self.start(headers, route == "/sign-up"),
            ("GET", CALLBACK) => self.callback(headers, session, query, on_sign_in),
            _ => self.served(method, route, query, body),
        };
        match reply {
            Some(reply) => {
                reply.write(stream, session, fresh);
                true
            }
            None => false,
        }
    }

    /// A request to a provider this server also serves (the development
    /// one), answered by it.
    fn served(&self, method: &str, route: &str, query: &str, body: &[u8]) -> Option<Reply> {
        let provider = match &*self.mode.read().expect("mode") {
            Mode::Accounts(a) => a.provider.clone(),
            Mode::Guest => return None,
        };
        provider.serve(method, route, query, body)
    }

    /// **A sign-in started** (Core §3.1.2.1): a fresh `state`, `nonce` and
    /// code verifier, kept here by the state; the state also in a cookie of
    /// this browser's, so that the callback is this browser's sign-in and
    /// not one an attacker started and sent it to (login CSRF); and the
    /// browser sent to the provider. None in the guest model.
    fn start(&self, _headers: &str, sign_up: bool) -> Option<Reply> {
        let mode = self.mode.read().expect("mode");
        let Mode::Accounts(accounts) = &*mode else {
            return None;
        };
        let (state, nonce, verifier) = (token(), token(), token());
        let challenge = s256(&verifier);
        {
            let mut pending = accounts.pending.lock().expect("pending");
            pending.retain(|_, p| p.started.elapsed() < SIGN_IN_LIFETIME);
            pending.insert(
                state.clone(),
                Pending {
                    nonce: nonce.clone(),
                    verifier,
                    started: Instant::now(),
                },
            );
        }
        let url = authorization_url(
            &*accounts.provider,
            &accounts.redirect_uri,
            &state,
            &nonce,
            &challenge,
            sign_up,
        );
        Some(Reply::redirect(
            &url,
            vec![cookie_line(
                SIGN_IN_COOKIE,
                &state,
                "/sign-in",
                Some(SIGN_IN_LIFETIME.as_secs()),
                *SECURE_COOKIES.get().unwrap_or(&false),
            )],
        ))
    }

    /// **The provider's answer** (Core §3.1.2.5, §3.1.3.7): the state this
    /// browser started, once; the code exchanged with its verifier; the ID
    /// token's issuer, audience, expiry and nonce checked; and then a NEW
    /// session for the principal, the one the browser came with forgotten
    /// (OWASP: "renew the session ID after any privilege level change", the
    /// defence against session fixation).
    fn callback(
        &self,
        headers: &str,
        session: &str,
        query: &str,
        on_sign_in: &dyn Fn(&SignedIn),
    ) -> Option<Reply> {
        let mode = self.mode.read().expect("mode");
        let Mode::Accounts(accounts) = &*mode else {
            return None;
        };
        let fields = parse_form(query);
        let refused = |why: &str| {
            Reply::page(
                400,
                "Sign-in not completed",
                &format!(
                    "<p role=\"alert\">Sign-in was not completed: {}.</p>\
                     <p><a href=\"/sign-in\">Try again</a></p>",
                    escape(why)
                ),
            )
        };
        if let Some(error) = fields.get("error") {
            return Some(refused(&format!("the provider answered `{error}`")));
        }
        let state = fields.get("state").cloned().unwrap_or_default();
        let bound = cookie(headers, SIGN_IN_COOKIE).unwrap_or_default();
        if state.is_empty() || !same(&state, &bound) {
            return Some(refused("it was not started by this browser"));
        }
        let Some(pending) = accounts.pending.lock().expect("pending").remove(&state) else {
            return Some(refused("it was already completed or is unknown"));
        };
        if pending.started.elapsed() > SIGN_IN_LIFETIME {
            return Some(refused("it took too long"));
        }
        let code = fields.get("code").cloned().unwrap_or_default();
        let claims =
            match accounts
                .provider
                .exchange(&code, &pending.verifier, &accounts.redirect_uri)
            {
                Ok(c) => c,
                Err(why) => return Some(refused(&why)),
            };
        if let Err(why) = check_claims(&claims, &*accounts.provider, &pending.nonce) {
            return Some(refused(&why));
        }
        let principal = Principal {
            handle: claims
                .preferred_username
                .clone()
                .map(|h| format!("@{h}"))
                .unwrap_or_else(|| format!("@{}", claims.sub)),
            name: claims.name.clone().unwrap_or_else(|| claims.sub.clone()),
            user: claims.sub,
            issuer: claims.iss,
        };
        let rotated = new_session_id();
        // A guest's, where no sign-in opened the session it came with: a
        // signed-in session that signs in again is signed out, and what it
        // held is its user's, never the next one's.
        let was_guest = self.principals.of(session).is_none_or(|p| p.is_guest());
        self.principals.close(session);
        self.principals.open(&rotated, principal.clone());
        // TRACK SEAM (store-accounts): the deployment told, before the
        // browser is answered.
        if was_guest && !session.is_empty() {
            on_sign_in(&SignedIn {
                guest: session,
                session: &rotated,
                principal: &principal,
            });
        }
        let secure = *SECURE_COOKIES.get().unwrap_or(&false);
        Some(Reply::redirect(
            "/",
            vec![
                cookie_line(SESSION_COOKIE, &rotated, "/", None, secure),
                cookie_line(SIGN_IN_COOKIE, "", "/sign-in", Some(0), secure),
            ],
        ))
    }

    /// **Signed out**: the session's principal forgotten, so its id, if
    /// replayed, is no one; and a new session, so the browser is not left
    /// holding the old one.
    fn sign_out(&self, session: &str) -> Reply {
        self.principals.close(session);
        let rotated = new_session_id();
        Reply::redirect(
            "/",
            vec![cookie_line(
                SESSION_COOKIE,
                &rotated,
                "/",
                None,
                *SECURE_COOKIES.get().unwrap_or(&false),
            )],
        )
    }

    /// **What `requires predicate` is told for `session`** (ADR-0115):
    /// `Ok(true)` where the session's principal holds it, `Ok(false)` where
    /// it does not, and an error for a predicate this deployment does not
    /// know, so that an unknown predicate cannot run.
    ///
    /// - `SignedIn`: the session has a principal.
    /// - `OwnsPost(post)`: the session's principal wrote `post`, read through
    ///   the command's own operations (`feed:data/posts#thread`), so within
    ///   the command's transaction: the post it deletes is the post it was
    ///   shown to own.
    /// - TRACK SEAM (messages): `MayMessage(to)`: the session's principal may
    ///   message `to` (`messages.rs`), read the same way.
    pub fn requires(
        &self,
        predicate: &str,
        bound: &[&Val],
        session: &str,
        host: &std::collections::BTreeMap<String, HostFn>,
    ) -> Result<bool, String> {
        let held = self.holds(predicate, bound, session, host);
        if held == Ok(false) {
            REFUSED.with(|r| {
                r.borrow_mut().get_or_insert_with(|| predicate.to_string());
            });
        }
        held
    }

    fn holds(
        &self,
        predicate: &str,
        bound: &[&Val],
        session: &str,
        host: &std::collections::BTreeMap<String, HostFn>,
    ) -> Result<bool, String> {
        match predicate {
            "SignedIn" => Ok(self.principals.of(session).is_some()),
            "OwnsPost" => {
                let Some(principal) = self.principals.of(session) else {
                    return Ok(false);
                };
                let [Val::String(post)] = bound else {
                    return Err(format!(
                        "OwnsPost takes one post id, and was given {bound:?}"
                    ));
                };
                let thread = host
                    .get("feed:data/posts#thread")
                    .ok_or("OwnsPost reads a post's author through feed:data/posts#thread")?;
                match thread(&[Val::String(post.clone())])?.as_slice() {
                    [Val::Result(Ok(Some(found)))] => Ok(author_of(found) == Some(&principal.user)),
                    [Val::Result(Err(_))] => Ok(false),
                    other => Err(format!("posts#thread answered {other:?}")),
                }
            }
            // TRACK SEAM (messages): `MayMessage(to)`, the session's principal
            // may message `to`, read through the command's own operations, so
            // within its transaction, as `OwnsPost(post)` is.
            "MayMessage" => crate::messages::may_message(self.principals.of(session), bound, host),
            other => Err(format!(
                "the development deployment has no authorization predicate `{other}`"
            )),
        }
    }
}

thread_local! {
    /// **The predicate `requires` refused**, on the thread that ran the
    /// command: the command's route answers it as its own case (403), and
    /// takes it before the command runs so that none is left from another.
    static REFUSED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// **The predicate the last command on this thread was refused by**, taken.
pub fn take_refusal() -> Option<String> {
    REFUSED.with(|r| r.borrow_mut().take())
}

/// A post record's `author.id`.
fn author_of(post: &Val) -> Option<&String> {
    let Val::Record(fields) = post else {
        return None;
    };
    let (_, Val::Record(author)) = fields.iter().find(|(k, _)| k == "author")? else {
        return None;
    };
    match author.iter().find(|(k, _)| k == "id")? {
        (_, Val::String(id)) => Some(id),
        _ => None,
    }
}

/// **An ID token's claims, checked against what was sent** (Core
/// §3.1.3.7): the issuer exactly, this client in the audience, not expired,
/// and the nonce this sign-in sent.
pub fn check_claims(claims: &Claims, provider: &dyn Provider, nonce: &str) -> Result<(), String> {
    if claims.iss != provider.issuer() {
        return Err(format!("the token's issuer is `{}`", claims.iss));
    }
    if !claims.aud.iter().any(|a| *a == provider.client_id()) {
        return Err("the token is not for this client".to_string());
    }
    if claims.exp <= now_secs() {
        return Err("the token expired".to_string());
    }
    match &claims.nonce {
        Some(n) if same(n, nonce) => {}
        _ => return Err("the token's nonce is not this sign-in's".to_string()),
    }
    if claims.sub.is_empty() {
        return Err("the token names no subject".to_string());
    }
    Ok(())
}

/// **Does an unsafe request come from this origin?** (OWASP's CSRF
/// Prevention Cheat Sheet, "Fetch Metadata headers", with its "mandatory"
/// fallback to `Origin`; the decision Go 1.25's `CrossOriginProtection`
/// makes.) GET, HEAD and OPTIONS change nothing and pass. Otherwise:
/// `Sec-Fetch-Site` must be `same-origin` or `none` where a browser sends it;
/// where it sends none, `Origin`'s host must be the request's `Host`; and a
/// request with neither came from no browser, which carries no one's cookie
/// unasked, and passes.
pub fn same_origin(method: &str, headers: &str) -> Result<(), String> {
    if matches!(method, "GET" | "HEAD" | "OPTIONS") {
        return Ok(());
    }
    if let Some(site) = header(headers, "sec-fetch-site") {
        return match site.to_ascii_lowercase().as_str() {
            "same-origin" | "none" => Ok(()),
            other => Err(format!(
                "a {other} request is refused: it changes something"
            )),
        };
    }
    if let Some(origin) = header(headers, "origin") {
        let host = header(headers, "host").unwrap_or_default();
        let from = origin.split_once("://").map(|(_, h)| h).unwrap_or("");
        if from.is_empty() || !from.eq_ignore_ascii_case(host) {
            return Err(format!(
                "a request from `{origin}` to `{host}` is refused: it changes something"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP, as the track's routes need it
// ---------------------------------------------------------------------------

/// A header's value, by its name in any case.
pub fn header<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// A cookie's value, by its name.
pub fn cookie(headers: &str, name: &str) -> Option<String> {
    headers
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("cookie").then_some(v)
        })
        .flat_map(|v| v.split(';'))
        .find_map(|pair| {
            let (k, v) = pair.trim().split_once('=')?;
            (k == name).then(|| v.to_string())
        })
}

/// `application/x-www-form-urlencoded`, or a query string, as pairs.
pub fn parse_form(text: &str) -> HashMap<String, String> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let digit = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => match (digit(bytes[i + 1]), digit(bytes[i + 2])) {
                (Some(hi), Some(lo)) => {
                    out.push(hi * 16 + lo);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encoding of everything but RFC 3986's unreserved characters.
pub fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Text safe in HTML's text and in a quoted attribute.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// **An answer of the track's**: a page, a redirect or text, and the
/// cookies it sets. Every one is kept by no cache (ADR-0184).
pub struct Reply {
    status: u16,
    location: Option<String>,
    cookies: Vec<String>,
    mime: &'static str,
    body: String,
}

impl Reply {
    /// A page of this server's own: a sign-in that was not completed.
    pub(crate) fn page(status: u16, title: &str, main: &str) -> Reply {
        Reply::html(status, title, "", main)
    }

    /// **A page of the development provider's**, which says first that it
    /// is not production.
    pub(crate) fn provider_page(status: u16, title: &str, main: &str) -> Reply {
        Reply::html(
            status,
            title,
            "<p id=\"not-production\" role=\"note\">Development identity provider: not \
             for production. It runs only in a development deployment on this machine, and \
             keeps its accounts in memory.</p>",
            main,
        )
    }

    fn html(status: u16, title: &str, banner: &str, main: &str) -> Reply {
        Reply {
            status,
            location: None,
            cookies: Vec::new(),
            mime: "text/html; charset=utf-8",
            body: format!(
                "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
                 <title>{0}</title></head><body>{banner}\
                 <main><h1>{0}</h1>{main}</main></body></html>",
                escape(title)
            ),
        }
    }

    pub(crate) fn redirect(location: &str, cookies: Vec<String>) -> Reply {
        Reply {
            status: 303,
            location: Some(location.to_string()),
            cookies,
            mime: "text/plain; charset=utf-8",
            body: String::new(),
        }
    }

    fn text(status: u16, body: &str) -> Reply {
        Reply {
            status,
            location: None,
            cookies: Vec::new(),
            mime: "text/plain; charset=utf-8",
            body: body.to_string(),
        }
    }

    /// The response as bytes. A fresh session's cookie goes with it unless
    /// the reply sets the session itself.
    pub(crate) fn bytes(&self, session: &str, fresh: bool) -> Vec<u8> {
        let reason = match self.status {
            200 => "OK",
            303 => "See Other",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            405 => "Method Not Allowed",
            409 => "Conflict",
            _ => "",
        };
        let mut head = format!(
            "HTTP/1.1 {} {reason}\r\ncontent-type: {}\r\ncontent-length: {}\r\n{}",
            self.status,
            self.mime,
            self.body.len(),
            super::PRIVATE
        );
        if let Some(location) = &self.location {
            head.push_str(&format!("location: {location}\r\n"));
        }
        let sets_session = self
            .cookies
            .iter()
            .any(|c| c.starts_with(&format!("set-cookie: {SESSION_COOKIE}=")));
        if fresh && !sets_session {
            head.push_str(&session_cookie(session));
        }
        for c in &self.cookies {
            head.push_str(c);
        }
        head.push_str("connection: close\r\n\r\n");
        let mut out = head.into_bytes();
        out.extend_from_slice(self.body.as_bytes());
        out
    }

    fn write(&self, stream: &mut TcpStream, session: &str, fresh: bool) {
        let _ = stream.write_all(&self.bytes(session, fresh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The development model the track started from, as `Server::run` held
    /// it inline before ADR-0253, is the guest model: a session is signed in,
    /// no session is not, and a predicate the deployment does not know is an
    /// error.
    #[test]
    fn requires_starts_as_the_development_model() {
        let identity = Identity::default();
        let host = BTreeMap::new();
        assert_eq!(identity.requires("SignedIn", &[], "s-1", &host), Ok(true));
        assert_eq!(identity.requires("SignedIn", &[], "", &host), Ok(false));
        assert!(identity.requires("Admin", &[], "s-1", &host).is_err());
    }

    /// **PKCE's S256 is RFC 7636's** (Appendix B): its worked example's
    /// verifier gives its challenge.
    #[test]
    fn s256_is_rfc_7636s_worked_example() {
        assert_eq!(
            s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let verifier = token();
        assert!(is_verifier(&verifier), "{verifier}");
        assert_eq!(verifier.len(), 43);
        assert_ne!(verifier, token());
    }

    /// **A session's cookie**: HttpOnly and SameSite=Lax always, Secure
    /// where it is asked; a deployment asks it everywhere but plain HTTP on
    /// this machine.
    #[test]
    fn a_cookie_is_http_only_and_secure_outside_this_machine() {
        let line = cookie_line("pw-session", "s-1", "/", None, false);
        assert_eq!(
            line,
            "set-cookie: pw-session=s-1; Path=/; HttpOnly; SameSite=Lax\r\n"
        );
        let line = cookie_line("pw-session", "", "/", Some(0), true);
        assert!(line.ends_with("; Max-Age=0; Secure\r\n"), "{line}");
        let at = |origin: &str| Deployment {
            development: true,
            origin: origin.to_string(),
        };
        for local in [
            "http://127.0.0.1:3143",
            "http://localhost:8080",
            "http://feed.localhost",
            "http://[::1]:3000",
        ] {
            assert!(at(local).loopback(), "{local}");
            assert!(!at(local).secure_cookies(), "{local}");
        }
        assert!(at("https://localhost").secure_cookies());
        for public in [
            "https://feed.example.com",
            "http://192.168.1.5:3143",
            "http://localhost.example.com",
        ] {
            assert!(!at(public).loopback(), "{public}");
            assert!(at(public).secure_cookies(), "{public}");
        }
    }

    /// **A development identity runs only in a development deployment on
    /// this machine**: the guest model is refused in production and at a
    /// public origin, and an identity the server does not know is refused,
    /// not guessed.
    #[test]
    fn the_guest_model_runs_only_in_development_on_this_machine() {
        let identity = Identity::default();
        let production = Deployment {
            development: false,
            origin: "http://127.0.0.1:3143".to_string(),
        };
        assert!(identity.configure(&production, None).is_err());
        let public = Deployment {
            development: true,
            origin: "https://feed.example.com".to_string(),
        };
        assert!(identity.configure(&public, Some("guest")).is_err());
        assert!(
            identity
                .configure(&Deployment::development(3143), Some("ldap"))
                .is_err()
        );
        assert!(
            identity
                .configure(&Deployment::development(3143), None)
                .is_ok()
        );
        assert!(identity.describe().starts_with("guest"));
    }

    /// **A request that changes something comes from this origin**, as Go
    /// 1.25's `CrossOriginProtection` decides: Sec-Fetch-Site where a
    /// browser sends it, `Origin` against `Host` where it does not, and a
    /// request with neither is no browser's.
    #[test]
    fn an_unsafe_request_from_another_origin_is_refused() {
        let ok = |method: &str, headers: &str| same_origin(method, headers).is_ok();
        assert!(ok("GET", "Sec-Fetch-Site: cross-site\r\n"));
        assert!(ok("POST", "Sec-Fetch-Site: same-origin\r\nHost: t\r\n"));
        assert!(ok("POST", "Sec-Fetch-Site: none\r\n"));
        assert!(!ok("POST", "Sec-Fetch-Site: cross-site\r\n"));
        assert!(!ok("POST", "Sec-Fetch-Site: same-site\r\n"));
        // Sec-Fetch-Site decides where it is sent, whatever Origin says.
        assert!(!ok(
            "POST",
            "Sec-Fetch-Site: cross-site\r\nOrigin: http://t\r\nHost: t\r\n"
        ));
        assert!(ok("POST", "Origin: http://t:3143\r\nHost: t:3143\r\n"));
        assert!(!ok("POST", "Origin: http://evil.test\r\nHost: t:3143\r\n"));
        assert!(!ok(
            "POST",
            "Origin: http://t:3143.evil.test\r\nHost: t:3143\r\n"
        ));
        assert!(!ok("POST", "Origin: null\r\nHost: t\r\n"));
        assert!(!ok("DELETE", "origin: http://evil.test\r\nhost: t\r\n"));
        assert!(ok("POST", "Host: t\r\n"));
    }

    /// **An ID token is held to the sign-in that asked for it** (OpenID
    /// Connect Core §3.1.3.7).
    #[test]
    fn a_token_is_held_to_its_issuer_audience_expiry_and_nonce() {
        struct P;
        impl Provider for P {
            fn issuer(&self) -> String {
                "https://id.test".into()
            }
            fn client_id(&self) -> String {
                "c".into()
            }
            fn authorization_endpoint(&self) -> String {
                "https://id.test/authorize".into()
            }
            fn exchange(&self, _: &str, _: &str, _: &str) -> Result<Claims, String> {
                Err("unused".into())
            }
        }
        let good = Claims {
            iss: "https://id.test".into(),
            sub: "u1".into(),
            aud: vec!["other".into(), "c".into()],
            exp: now_secs() + 60,
            nonce: Some("n".into()),
            preferred_username: None,
            name: None,
        };
        assert_eq!(check_claims(&good, &P, "n"), Ok(()));
        let wrong: [fn(&mut Claims); 6] = [
            |c| c.iss = "https://id.test/".into(),
            |c| c.aud = vec!["other".into()],
            |c| c.exp = now_secs() - 1,
            |c| c.nonce = None,
            |c| c.nonce = Some("m".into()),
            |c| c.sub = String::new(),
        ];
        for (n, change) in wrong.iter().enumerate() {
            let mut claims = good.clone();
            change(&mut claims);
            assert!(check_claims(&claims, &P, "n").is_err(), "change {n}");
        }
    }

    /// **A fresh session's id is 128 random bits**, and a form's fields are
    /// read as a browser encodes them.
    #[test]
    fn ids_are_random_and_forms_are_decoded() {
        let (a, b) = (new_session_id(), new_session_id());
        assert_ne!(a, b);
        assert!(a.starts_with("s-") && a.len() == 34, "{a}");
        assert!(a[2..].bytes().all(|c| c.is_ascii_hexdigit()), "{a}");
        let form = parse_form("handle=ada&name=Ada+L%C3%B6velace&bad=%zz&cut=%4");
        assert_eq!(form["handle"], "ada");
        assert_eq!(form["name"], "Ada Lövelace");
        assert_eq!(form["bad"], "%zz");
        assert_eq!(form["cut"], "%4");
        assert_eq!(encode("a b/c~"), "a%20b%2Fc~");
        assert_eq!(
            escape("<a href=\"x\">'&"),
            "&lt;a href=&quot;x&quot;&gt;&#39;&amp;"
        );
    }

    /// **The refusal a command's route answers** is the first predicate
    /// `requires` refused on this thread, taken once.
    #[test]
    fn a_refusal_is_taken_once() {
        let identity = Identity::default();
        identity.principals.set_accounts(true);
        let host = BTreeMap::new();
        let _ = take_refusal();
        assert_eq!(identity.requires("SignedIn", &[], "s-1", &host), Ok(false));
        assert_eq!(identity.requires("OwnsPost", &[], "s-1", &host), Ok(false));
        assert_eq!(take_refusal().as_deref(), Some("SignedIn"));
        assert_eq!(take_refusal(), None);
    }
}
